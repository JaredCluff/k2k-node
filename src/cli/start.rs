use crate::capabilities::CapabilityRegistry;
use crate::config::K2KNodeConfig;
use crate::db::Database;
use crate::embeddings::EmbeddingModel;
use crate::keys::KeyManager;
use crate::server::{NodeState, start_server};
use crate::tasks::TaskQueue;
use crate::vectordb::VectorDB;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

pub async fn run(mut config: K2KNodeConfig) -> anyhow::Result<()> {
    // Ensure node_id exists
    if config.node_id.is_none() {
        config.node_id = Some(uuid::Uuid::new_v4().to_string());
        tracing::info!("Generated node_id: {}", config.node_id.as_ref().unwrap());
    }

    // Ensure data directory exists
    std::fs::create_dir_all(&config.data_dir)?;

    // Initialize components
    let db = Arc::new(Database::open(&config.db_path())?);
    let key_manager = Arc::new(Mutex::new(KeyManager::load_or_generate(&config.keys_dir())?));

    tracing::info!("Loading embedding model...");
    let embedding_model = Arc::new(Mutex::new(EmbeddingModel::load(&config.models_dir())?));
    tracing::info!("Embedding model loaded");

    let vectordb = Arc::new(VectorDB::new(db.clone()));
    let capability_registry = Arc::new(CapabilityRegistry::new());
    let task_queue = Arc::new(TaskQueue::new(config.max_task_workers));

    let state = Arc::new(NodeState {
        config: config.clone(),
        db: db.clone(),
        key_manager,
        embedding_model,
        vectordb,
        capability_registry,
        task_queue,
        start_time: Instant::now(),
    });

    // Start mDNS discovery in background
    let db_for_mdns = db.clone();
    let config_for_mdns = config.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::discovery::start_mdns(&config_for_mdns, db_for_mdns).await {
            tracing::warn!("mDNS discovery failed to start: {}", e);
        }
    });

    // Index configured paths on startup
    if !config.index_paths.is_empty() {
        let paths = config.index_paths.clone();
        let db_for_index = db.clone();
        let model = state.embedding_model.clone();
        tokio::spawn(async move {
            for path in &paths {
                let mut model_guard = model.lock().await;
                match crate::indexer::index_directory(path, &db_for_index, &mut model_guard).await {
                    Ok(count) => tracing::info!("Startup index: {} chunks from {}", count, path),
                    Err(e) => tracing::warn!("Startup index failed for {}: {}", path, e),
                }
            }
        });
    }

    // Start HTTP server (blocks)
    start_server(state).await
}
