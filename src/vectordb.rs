use crate::db::Database;
use crate::embeddings::EmbeddingModel;
use anyhow::Result;
use std::sync::Arc;

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub chunk_id: String,
    pub title: String,
    pub content: String,
    pub path: String,
    pub score: f32,
    pub chunk_index: Option<i32>,
}

pub struct VectorDB {
    db: Arc<Database>,
}

impl VectorDB {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Search for chunks most similar to the query embedding.
    pub fn search(&self, query_embedding: &[f32], top_k: usize) -> Result<Vec<SearchResult>> {
        let all_chunks = self.db.get_all_embeddings()?;

        let mut scored: Vec<(f32, String, String, String, String)> = all_chunks
            .into_iter()
            .map(|(id, embedding_bytes, title, content, path)| {
                let embedding = EmbeddingModel::bytes_to_embedding(&embedding_bytes);
                let score = cosine_similarity(query_embedding, &embedding);
                (score, id, title, content, path)
            })
            .collect();

        // Sort descending by score
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);

        Ok(scored
            .into_iter()
            .map(|(score, chunk_id, title, content, path)| SearchResult {
                chunk_id,
                title,
                content,
                path,
                score,
                chunk_index: None,
            })
            .collect())
    }

    pub fn indexed_count(&self) -> Result<usize> {
        self.db.chunk_count()
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}
