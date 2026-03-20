use crate::config::K2KNodeConfig;
use crate::db::Database;
use std::collections::HashMap;
use std::sync::Arc;

/// Start mDNS advertisement and browsing.
pub async fn start_mdns(config: &K2KNodeConfig, db: Arc<Database>) -> anyhow::Result<()> {
    if !config.mdns_enabled {
        tracing::info!("mDNS discovery disabled");
        return Ok(());
    }

    let node_id = config.node_id.clone().unwrap_or_default();
    let port = config.port;
    let node_name = config.node_name.clone();
    let trusted_ids = config.trusted_node_ids.clone();

    // Register our service
    let mdns = mdns_sd::ServiceDaemon::new()
        .map_err(|e| anyhow::anyhow!("Failed to create mDNS daemon: {}", e))?;

    let instance_name = node_name
        .to_lowercase()
        .replace(' ', "-")
        .chars()
        .take(63)
        .collect::<String>();

    let service_type = "_k2k._tcp.local.";
    let mut properties = HashMap::new();
    properties.insert("node_id".to_string(), node_id.clone());
    properties.insert("version".to_string(), env!("CARGO_PKG_VERSION").to_string());

    let service_info = mdns_sd::ServiceInfo::new(
        service_type,
        &instance_name,
        &format!("{}.local.", instance_name),
        "",
        port,
        properties,
    ).map_err(|e| anyhow::anyhow!("Failed to create service info: {}", e))?;

    mdns.register(service_info)
        .map_err(|e| anyhow::anyhow!("Failed to register mDNS service: {}", e))?;
    tracing::info!("mDNS: advertising as '{}' on port {}", instance_name, port);

    // Browse for peers
    let receiver = mdns.browse(service_type)
        .map_err(|e| anyhow::anyhow!("Failed to browse mDNS: {}", e))?;

    let db_clone = db.clone();
    let own_node_id = node_id.clone();

    tokio::spawn(async move {
        while let Ok(event) = receiver.recv_async().await {
            if let mdns_sd::ServiceEvent::ServiceResolved(info) = event {
                    let props = info.get_properties();
                    let peer_id = props.get_property_val_str("node_id").unwrap_or("");

                    // Skip self
                    if peer_id == own_node_id || peer_id.is_empty() {
                        continue;
                    }

                    // Check trusted list (empty = trust none)
                    if !trusted_ids.is_empty() && !trusted_ids.contains(&peer_id.to_string()) {
                        tracing::debug!("mDNS: ignoring untrusted node {}", peer_id);
                        continue;
                    }

                    let addresses = info.get_addresses();
                    if let Some(addr) = addresses.iter().next() {
                        let port = info.get_port();
                        let endpoint = format!("http://{}:{}/k2k/v1", addr, port);

                        tracing::info!("mDNS: discovered peer {} at {}", peer_id, endpoint);
                        if let Err(e) = db_clone.upsert_node(peer_id, &addr.to_string(), port, &endpoint, "[]") {
                            tracing::warn!("Failed to register discovered node: {}", e);
                        }
                    }
            }
        }
    });

    Ok(())
}
