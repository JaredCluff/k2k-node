use crate::server::NodeState;
use axum::http::HeaderMap;
use std::sync::Arc;

/// Authenticate a request by verifying the JWT bearer token.
///
/// Returns the validated claims or an error string.
pub async fn authenticate(
    state: &Arc<NodeState>,
    headers: &HeaderMap,
) -> Result<k2k_common::K2KClaims, String> {
    // 1. Extract bearer token
    let auth_header = headers
        .get("authorization")
        .ok_or("Missing Authorization header")?
        .to_str()
        .map_err(|_| "Invalid Authorization header")?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or("Authorization must be Bearer token")?;

    // 2. Decode JWT to get client_id (unverified)
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err("Invalid JWT format".to_string());
    }

    let payload_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|_| "Invalid JWT payload encoding")?;
    let payload: serde_json::Value = serde_json::from_slice(&payload_bytes)
        .map_err(|_| "Invalid JWT payload JSON")?;

    let client_id = payload
        .get("client_id")
        .and_then(|v| v.as_str())
        .ok_or("JWT missing client_id claim")?;

    // 3. Look up client
    let client = state.db.get_client(client_id)
        .map_err(|e| format!("Database error: {}", e))?
        .ok_or_else(|| format!("Unknown client: {}", client_id))?;

    let (_name, public_key_pem, status) = client;

    // 4. Check client status
    if status != "approved" {
        return Err(format!("Client '{}' is not approved (status: {})", client_id, status));
    }

    // 5. Verify JWT signature
    let claims = state.key_manager.lock().await.verify_jwt(token, &public_key_pem)
        .map_err(|e| format!("JWT verification failed: {}", e))?;

    // 6. Check expiration
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    if claims.exp < now {
        return Err("JWT expired".to_string());
    }

    // 7. Check iat not too far in future
    if claims.iat > now + 60 {
        return Err("JWT issued-at is too far in the future".to_string());
    }

    // 8. Check allowed clients list
    if !state.config.allowed_clients.is_empty()
        && !state.config.allowed_clients.contains(&client_id.to_string())
    {
        return Err(format!("Client '{}' not in allowed_clients list", client_id));
    }

    Ok(claims)
}

use base64::Engine;
