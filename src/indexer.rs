use crate::db::Database;
use crate::embeddings::EmbeddingModel;
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

const CHUNK_SIZE: usize = 512; // characters per chunk
const CHUNK_OVERLAP: usize = 64;

/// Index all supported files under a directory path.
pub async fn index_directory(
    path: &str,
    db: &Arc<Database>,
    model: &mut EmbeddingModel,
) -> Result<usize> {
    let mut count = 0;

    for entry in walkdir::WalkDir::new(path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let file_path = entry.path();
        if !file_path.is_file() {
            continue;
        }

        // Skip hidden files and common non-text files
        let name = file_path.file_name().unwrap_or_default().to_string_lossy();
        if name.starts_with('.') {
            continue;
        }

        let ext = file_path.extension().unwrap_or_default().to_string_lossy().to_lowercase();
        if !is_supported_extension(&ext) {
            continue;
        }

        match index_file(file_path, db, model) {
            Ok(chunks) => {
                count += chunks;
            }
            Err(e) => {
                tracing::warn!("Failed to index {}: {}", file_path.display(), e);
            }
        }
    }

    tracing::info!("Indexed {} chunks from {}", count, path);
    Ok(count)
}

fn index_file(path: &Path, db: &Arc<Database>, model: &mut EmbeddingModel) -> Result<usize> {
    let content = std::fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(0);
    }

    let title = path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let path_str = path.to_string_lossy().to_string();
    let metadata = std::fs::metadata(path)?;
    let file_size = metadata.len() as i64;
    let modified_at = metadata.modified().ok().map(|t| {
        chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339()
    });
    let content_type = mime_from_extension(path);

    let chunks = chunk_text(&content, CHUNK_SIZE, CHUNK_OVERLAP);
    let mut count = 0;

    for (idx, chunk) in chunks.iter().enumerate() {
        let embedding = model.embed_text(chunk)?;
        let embedding_bytes = EmbeddingModel::embedding_to_bytes(&embedding);

        let chunk_id = format!("{}:{}", path_str, idx);

        db.insert_chunk(
            &chunk_id,
            &path_str,
            &title,
            chunk,
            idx as i32,
            &embedding_bytes,
            content_type.as_deref(),
            Some(file_size),
            modified_at.as_deref(),
        )?;

        count += 1;
    }

    if count > 0 {
        tracing::debug!("Indexed {} chunks from {}", count, path_str);
    }
    Ok(count)
}

fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= chunk_size {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut start = 0;

    while start < chars.len() {
        let end = (start + chunk_size).min(chars.len());
        let chunk: String = chars[start..end].iter().collect();
        if !chunk.trim().is_empty() {
            chunks.push(chunk);
        }
        if end >= chars.len() {
            break;
        }
        start += chunk_size - overlap;
    }

    chunks
}

fn is_supported_extension(ext: &str) -> bool {
    matches!(
        ext,
        "txt" | "md" | "markdown" | "rst" | "org"
            | "rs" | "py" | "js" | "ts" | "go" | "java" | "c" | "cpp" | "h" | "hpp"
            | "rb" | "php" | "swift" | "kt" | "scala"
            | "json" | "yaml" | "yml" | "toml" | "xml" | "csv"
            | "html" | "htm" | "css" | "scss"
            | "sh" | "bash" | "zsh" | "fish"
            | "sql" | "r" | "lua" | "perl" | "pl"
            | "tex" | "bib"
            | "dockerfile" | "makefile"
    )
}

fn mime_from_extension(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    Some(match ext.as_str() {
        "md" | "markdown" => "text/markdown",
        "txt" => "text/plain",
        "html" | "htm" => "text/html",
        "json" => "application/json",
        "yaml" | "yml" => "text/yaml",
        "rs" => "text/x-rust",
        "py" => "text/x-python",
        "js" => "text/javascript",
        "ts" => "text/typescript",
        _ => "text/plain",
    }.to_string())
}
