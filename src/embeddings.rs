use anyhow::{Context, Result};
use ndarray::Array2;
use ort::session::Session;
use ort::value::Tensor;
use std::path::Path;

pub struct EmbeddingModel {
    session: Session,
    tokenizer: tokenizers::Tokenizer,
    dimension: usize,
}

impl EmbeddingModel {
    /// Load or download the all-MiniLM-L6-v2 model.
    pub fn load(models_dir: &str) -> Result<Self> {
        std::fs::create_dir_all(models_dir)?;

        let model_path = format!("{}/all-MiniLM-L6-v2/model.onnx", models_dir);
        let tokenizer_path = format!("{}/all-MiniLM-L6-v2/tokenizer.json", models_dir);

        if !Path::new(&model_path).exists() || !Path::new(&tokenizer_path).exists() {
            Self::download_model(models_dir)?;
        }

        let session = Session::builder()?
            .with_intra_threads(1)?
            .commit_from_file(&model_path)
            .context("Failed to load ONNX model")?;

        let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        Ok(Self {
            session,
            tokenizer,
            dimension: 384, // MiniLM-L6-v2 output dimension
        })
    }

    fn download_model(models_dir: &str) -> Result<()> {
        let model_dir = format!("{}/all-MiniLM-L6-v2", models_dir);
        std::fs::create_dir_all(&model_dir)?;

        tracing::info!("Downloading all-MiniLM-L6-v2 model...");

        // Download model.onnx
        let model_url = "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/onnx/model.onnx";
        let model_bytes = reqwest::blocking::get(model_url)
            .context("Failed to download model")?
            .bytes()
            .context("Failed to read model bytes")?;
        std::fs::write(format!("{}/model.onnx", model_dir), &model_bytes)?;

        // Download tokenizer.json
        let tokenizer_url = "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/tokenizer.json";
        let tokenizer_bytes = reqwest::blocking::get(tokenizer_url)
            .context("Failed to download tokenizer")?
            .bytes()
            .context("Failed to read tokenizer bytes")?;
        std::fs::write(format!("{}/tokenizer.json", model_dir), &tokenizer_bytes)?;

        tracing::info!("Model downloaded successfully");
        Ok(())
    }

    /// Generate an embedding vector for the given text.
    pub fn embed_text(&mut self, text: &str) -> Result<Vec<f32>> {
        let encoding = self.tokenizer.encode(text, true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;

        let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        let attention_mask: Vec<i64> = encoding.get_attention_mask().iter().map(|&m| m as i64).collect();
        let token_type_ids: Vec<i64> = encoding.get_type_ids().iter().map(|&t| t as i64).collect();

        let seq_len = input_ids.len();

        let input_ids_array = Array2::from_shape_vec((1, seq_len), input_ids)?;
        let attention_mask_array = Array2::from_shape_vec((1, seq_len), attention_mask)?;
        let token_type_ids_array = Array2::from_shape_vec((1, seq_len), token_type_ids)?;

        // Convert to ort Tensors
        let input_ids_tensor = Tensor::from_array(input_ids_array)?;
        let attention_mask_tensor = Tensor::from_array(attention_mask_array)?;
        let token_type_ids_tensor = Tensor::from_array(token_type_ids_array)?;

        let outputs = self.session.run(ort::inputs![
            "input_ids" => input_ids_tensor,
            "attention_mask" => attention_mask_tensor,
            "token_type_ids" => token_type_ids_tensor,
        ])?;

        // Extract the token embeddings (shape: [1, seq_len, 384])
        let embeddings = outputs["last_hidden_state"].try_extract_array::<f32>()?;
        let embeddings = embeddings.view();

        // Mean pooling over token dimension
        let mut pooled = vec![0.0f32; self.dimension];
        let mask: Vec<f32> = encoding.get_attention_mask().iter().map(|&m| m as f32).collect();
        let mask_sum: f32 = mask.iter().sum();

        for (token_idx, &m) in mask.iter().enumerate() {
            if m > 0.0 {
                for dim in 0..self.dimension {
                    pooled[dim] += embeddings[[0, token_idx, dim]];
                }
            }
        }

        for dim in 0..self.dimension {
            pooled[dim] /= mask_sum.max(1.0);
        }

        // L2 normalize
        let norm: f32 = pooled.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut pooled {
                *x /= norm;
            }
        }

        Ok(pooled)
    }

    /// Convert a float vector to bytes for SQLite BLOB storage.
    pub fn embedding_to_bytes(embedding: &[f32]) -> Vec<u8> {
        embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
    }

    /// Convert bytes back to a float vector.
    pub fn bytes_to_embedding(bytes: &[u8]) -> Vec<f32> {
        bytes.chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect()
    }

    pub fn dimension(&self) -> usize {
        self.dimension
    }
}
