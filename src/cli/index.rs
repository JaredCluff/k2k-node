use crate::config::K2KNodeConfig;
use crate::db::Database;
use crate::embeddings::EmbeddingModel;
use std::sync::Arc;

pub async fn run(config: K2KNodeConfig, path: &str) -> anyhow::Result<()> {
    std::fs::create_dir_all(&config.data_dir)?;

    let db = Arc::new(Database::open(&config.db_path())?);
    let mut model = EmbeddingModel::load(&config.models_dir())?;

    println!("Indexing {}...", path);
    let count = crate::indexer::index_directory(path, &db, &mut model).await?;
    println!("Indexed {} chunks", count);

    let total = db.chunk_count()?;
    println!("Total chunks in database: {}", total);

    Ok(())
}
