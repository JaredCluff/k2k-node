use crate::middleware;
use crate::server::NodeState;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use subtle::ConstantTimeEq;

// ============================================================================
// Public endpoints
// ============================================================================

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub node_id: String,
    pub node_type: String,
    pub capabilities: Vec<String>,
    pub indexed_files: usize,
    pub uptime_seconds: u64,
    pub protocol_version: String,
}

pub async fn handle_health(State(state): State<Arc<NodeState>>) -> Json<HealthResponse> {
    let indexed = state.vectordb.indexed_count().unwrap_or(0);
    let uptime = state.start_time.elapsed().as_secs();
    let caps = state.capability_registry.list_ids();
    let node_id = state.config.node_id.clone().unwrap_or_default();

    Json(HealthResponse {
        status: "healthy".to_string(),
        node_id,
        node_type: "k2k-node".to_string(),
        capabilities: caps,
        indexed_files: indexed,
        uptime_seconds: uptime,
        protocol_version: k2k_common::PROTOCOL_VERSION.to_string(),
    })
}

#[derive(Serialize)]
pub struct InfoResponse {
    pub node_id: String,
    pub node_name: String,
    pub node_type: String,
    pub version: String,
    pub public_key: String,
    pub federation_endpoint: String,
    pub protocol_version: String,
}

pub async fn handle_info(State(state): State<Arc<NodeState>>) -> Json<InfoResponse> {
    let node_id = state.config.node_id.clone().unwrap_or_default();
    let public_key = state.key_manager.lock().await.public_key_pem().to_string();

    Json(InfoResponse {
        node_id,
        node_name: state.config.node_name.clone(),
        node_type: "k2k-node".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        public_key,
        federation_endpoint: format!("http://{}:{}/k2k/v1", state.config.host, state.config.port),
        protocol_version: k2k_common::PROTOCOL_VERSION.to_string(),
    })
}

// ============================================================================
// Client registration
// ============================================================================

#[derive(Deserialize)]
pub struct RegisterClientRequest {
    pub client_id: String,
    pub client_name: String,
    pub public_key_pem: String,
    #[serde(default)]
    pub registration_secret: Option<String>,
}

#[derive(Serialize)]
pub struct RegisterClientResponse {
    pub status: String,
    pub client_id: String,
    pub message: String,
}

pub async fn handle_register_client(
    State(state): State<Arc<NodeState>>,
    Json(req): Json<RegisterClientRequest>,
) -> Result<Json<RegisterClientResponse>, (StatusCode, String)> {
    // Determine initial status
    let status = if state.config.auto_approve {
        "approved"
    } else if let Some(ref secret) = state.config.registration_secret {
        if let Some(ref provided) = req.registration_secret {
            let a = provided.as_bytes();
            let b = secret.as_bytes();
            if a.len() == b.len() && bool::from(a.ct_eq(b)) {
                "approved"
            } else {
                "pending"
            }
        } else {
            "pending"
        }
    } else {
        "pending"
    };

    state.db.register_client(&req.client_id, &req.client_name, &req.public_key_pem, status)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tracing::info!("Client '{}' registered with status '{}'", req.client_id, status);

    Ok(Json(RegisterClientResponse {
        status: status.to_string(),
        client_id: req.client_id,
        message: if status == "approved" {
            "Client approved".to_string()
        } else {
            "Client registered, pending admin approval".to_string()
        },
    }))
}

// ============================================================================
// Capabilities
// ============================================================================

pub async fn handle_capabilities(
    State(state): State<Arc<NodeState>>,
) -> Json<k2k_common::CapabilitiesResponse> {
    let caps = state.capability_registry.list();
    let node_id = state.config.node_id.clone().unwrap_or_default();

    Json(k2k_common::CapabilitiesResponse {
        node_id,
        capabilities: caps,
        protocol_version: k2k_common::PROTOCOL_VERSION.to_string(),
    })
}

#[derive(Serialize)]
pub struct ManifestResponse {
    pub service_name: String,
    pub version: String,
    pub protocol_version: String,
    pub capabilities: Vec<k2k_common::AgentCapability>,
}

pub async fn handle_manifest(
    State(state): State<Arc<NodeState>>,
) -> Json<ManifestResponse> {
    let caps = state.capability_registry.list();
    let node_id = state.config.node_id.clone().unwrap_or_default();

    Json(ManifestResponse {
        service_name: format!("k2k-node-{}", node_id),
        version: env!("CARGO_PKG_VERSION").to_string(),
        protocol_version: k2k_common::PROTOCOL_VERSION.to_string(),
        capabilities: caps,
    })
}

// ============================================================================
// Admin endpoints
// ============================================================================

pub async fn handle_pending_clients(
    State(state): State<Arc<NodeState>>,
) -> Result<Json<Vec<serde_json::Value>>, (StatusCode, String)> {
    let clients = state.db.list_pending_clients()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let result: Vec<serde_json::Value> = clients.into_iter().map(|(id, name, registered_at)| {
        serde_json::json!({
            "client_id": id,
            "client_name": name,
            "registered_at": registered_at,
        })
    }).collect();

    Ok(Json(result))
}

#[derive(Deserialize)]
pub struct ClientAction {
    pub client_id: String,
}

pub async fn handle_approve_client(
    State(state): State<Arc<NodeState>>,
    Json(req): Json<ClientAction>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let approved = state.db.approve_client(&req.client_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if approved {
        tracing::info!("Client '{}' approved", req.client_id);
        Ok(Json(serde_json::json!({"status": "approved", "client_id": req.client_id})))
    } else {
        Err((StatusCode::NOT_FOUND, "Client not found or not pending".to_string()))
    }
}

pub async fn handle_reject_client(
    State(state): State<Arc<NodeState>>,
    Json(req): Json<ClientAction>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let rejected = state.db.reject_client(&req.client_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if rejected {
        tracing::info!("Client '{}' rejected", req.client_id);
        Ok(Json(serde_json::json!({"status": "rejected", "client_id": req.client_id})))
    } else {
        Err((StatusCode::NOT_FOUND, "Client not found".to_string()))
    }
}

// ============================================================================
// Protected: Query
// ============================================================================

pub async fn handle_query(
    State(state): State<Arc<NodeState>>,
    headers: HeaderMap,
    Json(req): Json<k2k_common::K2KQueryRequest>,
) -> Result<Json<k2k_common::K2KQueryResponse>, (StatusCode, String)> {
    // Authenticate
    let _claims = middleware::authenticate(&state, &headers).await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e))?;

    let start = std::time::Instant::now();

    // Generate query embedding
    let query_embedding = {
        let model = &mut *state.embedding_model.lock().await;
        model.embed_text(&req.query)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Embedding failed: {}", e)))?
    };

    // Search
    let top_k = req.top_k.min(100);
    let results = state.vectordb.search(&query_embedding, top_k)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Search failed: {}", e)))?;

    let query_time_ms = start.elapsed().as_millis() as u64;
    let node_id = state.config.node_id.clone().unwrap_or_default();

    let k2k_results: Vec<k2k_common::K2KResult> = results.into_iter().map(|r| {
        k2k_common::K2KResult {
            article_id: r.chunk_id.clone(),
            store_id: node_id.clone(),
            title: r.title,
            summary: r.content.chars().take(200).collect(),
            content: r.content,
            confidence: r.score,
            source_type: "local_file".to_string(),
            tags: vec![],
            metadata: serde_json::json!({"path": r.path}),
            provenance: Some(k2k_common::ResultProvenance {
                store_id: node_id.clone(),
                store_type: "personal".to_string(),
                original_rank: 0,
                rrf_score: r.score,
            }),
        }
    }).collect();

    Ok(Json(k2k_common::K2KQueryResponse {
        query_id: uuid::Uuid::new_v4().to_string(),
        total_results: k2k_results.len(),
        results: k2k_results,
        stores_queried: vec![node_id],
        query_time_ms,
        routing_decision: None,
        trace_id: req.trace_id,
    }))
}

// ============================================================================
// Protected: Tasks
// ============================================================================

pub async fn handle_submit_task(
    State(state): State<Arc<NodeState>>,
    headers: HeaderMap,
    Json(mut req): Json<k2k_common::TaskRequest>,
) -> Result<(StatusCode, Json<k2k_common::TaskSubmitResponse>), (StatusCode, String)> {
    let claims = middleware::authenticate(&state, &headers).await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e))?;

    req.client_id = claims.client_id;

    let task_id = uuid::Uuid::new_v4().to_string();

    // Persist task to SQLite (spec improvement #6)
    let input_json = serde_json::to_string(&req.input).unwrap_or_default();
    state.db.insert_task(&task_id, &req.capability_id, &req.client_id, &input_json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // If capability is semantic_search, execute inline
    if req.capability_id == "semantic_search" {
        let query = req.input.get("query").and_then(|v| v.as_str()).unwrap_or("");
        let top_k = req.input.get("top_k").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        let query_embedding = {
            let model = &mut *state.embedding_model.lock().await;
            model.embed_text(query)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Embedding failed: {}", e)))?
        };

        let results = state.vectordb.search(&query_embedding, top_k)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Search failed: {}", e)))?;

        let result_json = serde_json::to_string(&serde_json::json!({
            "results": results,
            "total": results.len(),
        })).unwrap_or_default();

        state.db.update_task_status(&task_id, "completed", Some(&result_json), None, 100)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(k2k_common::TaskSubmitResponse {
            task_id,
            status: k2k_common::TaskStatus::Queued,
        }),
    ))
}

pub async fn handle_get_task(
    State(state): State<Arc<NodeState>>,
    headers: HeaderMap,
    Path(task_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let claims = middleware::authenticate(&state, &headers).await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e))?;

    let task = state.db.get_task(&task_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Task not found".to_string()))?;

    // IDOR prevention: only the task owner can see it
    if task.client_id != claims.client_id {
        return Err((StatusCode::NOT_FOUND, "Task not found".to_string()));
    }

    Ok(Json(serde_json::json!({
        "task_id": task.task_id,
        "capability_id": task.capability_id,
        "status": task.status,
        "result": task.result.as_deref().and_then(|r| serde_json::from_str::<serde_json::Value>(r).ok()),
        "error": task.error,
        "progress": task.progress,
        "created_at": task.created_at,
        "updated_at": task.updated_at,
    })))
}

pub async fn handle_cancel_task(
    State(state): State<Arc<NodeState>>,
    headers: HeaderMap,
    Path(task_id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let claims = middleware::authenticate(&state, &headers).await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e))?;

    let task = state.db.get_task(&task_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Task not found".to_string()))?;

    if task.client_id != claims.client_id {
        return Err((StatusCode::NOT_FOUND, "Task not found".to_string()));
    }

    if task.status != "queued" && task.status != "running" {
        return Err((StatusCode::CONFLICT, "Task is not in a cancellable state".to_string()));
    }

    state.db.update_task_status(&task_id, "cancelled", None, None, task.progress)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}
