use crate::capabilities::CapabilityRegistry;
use crate::config::K2KNodeConfig;
use crate::db::Database;
use crate::embeddings::EmbeddingModel;
use crate::keys::KeyManager;
use crate::tasks::TaskQueue;
use crate::vectordb::VectorDB;
use axum::{
    routing::{get, post, delete},
    Router,
};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

pub struct NodeState {
    pub config: K2KNodeConfig,
    pub db: Arc<Database>,
    pub key_manager: Arc<Mutex<KeyManager>>,
    pub embedding_model: Arc<Mutex<EmbeddingModel>>,
    pub vectordb: Arc<VectorDB>,
    pub capability_registry: Arc<CapabilityRegistry>,
    pub task_queue: Arc<TaskQueue>,
    pub start_time: Instant,
}

pub async fn start_server(state: Arc<NodeState>) -> anyhow::Result<()> {
    let host = state.config.host.clone();
    let port = state.config.port;

    let app = Router::new()
        // Public endpoints (no auth)
        .route("/k2k/v1/health", get(crate::handlers::handle_health))
        .route("/k2k/v1/info", get(crate::handlers::handle_info))
        .route("/k2k/v1/register-client", post(crate::handlers::handle_register_client))
        .route("/k2k/v1/capabilities", get(crate::handlers::handle_capabilities))
        .route("/.well-known/k2k-manifest", get(crate::handlers::handle_manifest))
        // Admin endpoints (localhost only — enforced by binding to 127.0.0.1)
        .route("/k2k/v1/admin/pending-clients", get(crate::handlers::handle_pending_clients))
        .route("/k2k/v1/admin/approve-client", post(crate::handlers::handle_approve_client))
        .route("/k2k/v1/admin/reject-client", post(crate::handlers::handle_reject_client))
        // Protected endpoints (JWT auth)
        .route("/k2k/v1/query", post(crate::handlers::handle_query))
        .route("/k2k/v1/tasks", post(crate::handlers::handle_submit_task))
        .route("/k2k/v1/tasks/:task_id", get(crate::handlers::handle_get_task))
        .route("/k2k/v1/tasks/:task_id", delete(crate::handlers::handle_cancel_task))
        .with_state(state);

    let addr = format!("{}:{}", host, port);
    tracing::info!("K2K node listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
