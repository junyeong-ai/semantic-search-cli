use std::path::Path;
use std::sync::{Arc, Mutex};

use ort::session::{Session, builder::GraphOptimizationLevel};
use ort::value::Tensor;
use tokenizers::Tokenizer;
use tokenizers::{PaddingParams, PaddingStrategy, TruncationParams, TruncationStrategy};

use crate::error::ModelError;
use crate::models::EmbeddingConfig;

const QUERY_INSTRUCTION: &str =
    "Instruct: Given a search query, retrieve relevant passages\nQuery: ";

pub struct EmbeddingModel {
    session: Mutex<Session>,
    tokenizer: Tokenizer,
    dimension: usize,
}

impl EmbeddingModel {
    pub fn load(config: &EmbeddingConfig, model_dir: &Path) -> Result<Self, ModelError> {
        let model_path = model_dir.join("model.onnx");
        let tokenizer_path = model_dir.join("tokenizer.json");
        let max_tokens = config.max_tokens as usize;

        if !model_path.exists() {
            return Err(ModelError::NotFound(format!(
                "model not found: {}",
                model_path.display()
            )));
        }

        let session = Session::builder()
            .map_err(|e: ort::Error| ModelError::LoadError(e.to_string()))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e: ort::Error| ModelError::LoadError(e.to_string()))?
            .with_intra_threads(num_cpus())
            .map_err(|e: ort::Error| ModelError::LoadError(e.to_string()))?
            .commit_from_file(&model_path)
            .map_err(|e: ort::Error| ModelError::LoadError(e.to_string()))?;

        let mut tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| ModelError::TokenizerError(e.to_string()))?;

        // Configure truncation to prevent OOM with long texts
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: max_tokens,
                strategy: TruncationStrategy::LongestFirst,
                ..Default::default()
            }))
            .map_err(|e| ModelError::TokenizerError(e.to_string()))?;

        // Configure padding for efficient batch processing
        tokenizer.with_padding(Some(PaddingParams {
            strategy: PaddingStrategy::BatchLongest,
            ..Default::default()
        }));

        Ok(Self {
            session: Mutex::new(session),
            tokenizer,
            dimension: config.dimension as usize,
        })
    }

    pub fn embed(&self, texts: &[String], is_query: bool) -> Result<Vec<Vec<f32>>, ModelError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let processed: Vec<String> = if is_query {
            texts
                .iter()
                .map(|t| format!("{}{}", QUERY_INSTRUCTION, t))
                .collect()
        } else {
            texts.to_vec()
        };

        let encodings = self
            .tokenizer
            .encode_batch(processed.clone(), true)
            .map_err(|e| ModelError::TokenizerError(e.to_string()))?;

        let max_len = encodings
            .iter()
            .map(|e| e.get_ids().len())
            .max()
            .unwrap_or(0);
        let batch_size = encodings.len();

        let mut input_ids = vec![0i64; batch_size * max_len];
        let mut attention_mask = vec![0i64; batch_size * max_len];
        let mut position_ids = vec![0i64; batch_size * max_len];

        for (i, encoding) in encodings.iter().enumerate() {
            let ids = encoding.get_ids();
            let mask = encoding.get_attention_mask();
            for (j, (&id, &m)) in ids.iter().zip(mask.iter()).enumerate() {
                input_ids[i * max_len + j] = id as i64;
                attention_mask[i * max_len + j] = m as i64;
                position_ids[i * max_len + j] = j as i64;
            }
        }

        let input_ids_tensor = Tensor::from_array(([batch_size, max_len], input_ids))
            .map_err(|e: ort::Error| ModelError::InferenceError(e.to_string()))?;
        let attention_mask_tensor = Tensor::from_array(([batch_size, max_len], attention_mask))
            .map_err(|e: ort::Error| ModelError::InferenceError(e.to_string()))?;
        let position_ids_tensor = Tensor::from_array(([batch_size, max_len], position_ids))
            .map_err(|e: ort::Error| ModelError::InferenceError(e.to_string()))?;

        let mut session = self
            .session
            .lock()
            .map_err(|_| ModelError::InferenceError("session lock poisoned".to_string()))?;

        let outputs = session
            .run(ort::inputs![
                input_ids_tensor,
                attention_mask_tensor,
                position_ids_tensor
            ])
            .map_err(|e: ort::Error| ModelError::InferenceError(e.to_string()))?;

        let output_array = outputs[0]
            .try_extract_array::<f32>()
            .map_err(|e: ort::Error| ModelError::InferenceError(e.to_string()))?;

        let shape = output_array.shape();

        let embeddings: Vec<Vec<f32>> = if shape.len() == 3 {
            (0..batch_size)
                .map(|i| {
                    let seq_len = encodings[i].get_ids().len();
                    let last_idx = seq_len.saturating_sub(1);
                    let embedding: Vec<f32> = (0..self.dimension)
                        .map(|d| output_array[[i, last_idx, d]])
                        .collect();
                    normalize(&embedding)
                })
                .collect()
        } else if shape.len() == 2 {
            (0..batch_size)
                .map(|i| {
                    let embedding: Vec<f32> =
                        (0..self.dimension).map(|d| output_array[[i, d]]).collect();
                    normalize(&embedding)
                })
                .collect()
        } else {
            return Err(ModelError::InferenceError(format!(
                "unexpected output shape: {:?}",
                shape
            )));
        };

        Ok(embeddings)
    }

    pub fn dimension(&self) -> usize {
        self.dimension
    }
}

fn normalize(v: &[f32]) -> Vec<f32> {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        v.iter().map(|x| x / norm).collect()
    } else {
        v.to_vec()
    }
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

pub type SharedEmbeddingModel = Arc<EmbeddingModel>;
