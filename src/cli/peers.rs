use crate::config::K2KNodeConfig;
use crate::db::Database;
use std::sync::Arc;

pub async fn run(config: K2KNodeConfig) -> anyhow::Result<()> {
    let db = Arc::new(Database::open(&config.db_path())?);
    let nodes = db.list_healthy_nodes()?;

    if nodes.is_empty() {
        println!("No discovered peers");
    } else {
        println!("Discovered peers:");
        println!("{}", "-".repeat(60));
        for (node_id, host, port, endpoint) in &nodes {
            println!("  {} ({}:{}) -> {}", node_id, host, port, endpoint);
        }
    }

    Ok(())
}
