use crate::config::K2KNodeConfig;
use crate::db::Database;
use std::sync::Arc;

pub async fn run(config: K2KNodeConfig, client_id: &str) -> anyhow::Result<()> {
    let db = Arc::new(Database::open(&config.db_path())?);

    if db.approve_client(client_id)? {
        println!("Client '{}' approved", client_id);
    } else {
        println!("Client '{}' not found or not pending", client_id);
    }

    Ok(())
}
