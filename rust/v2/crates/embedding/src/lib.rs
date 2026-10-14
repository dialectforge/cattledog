// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Embedding crate — swappable semantic embedding backends.
//!
//! The `EmbeddingEngine` trait is the single interface the parietal lobe uses.
//! Two implementations ship with the scaffold:
//!
//!   - `StubEngine`    — TF-IDF token-projection embeddings; always compiles,
//!                       no native libraries needed.  Used for tests + dev.
//!                       Texts that share vocabulary produce similar embeddings,
//!                       making semantic search meaningful even without a real model.
//!   - `BgeSmallEngine` — BGE-small-en-v1.5 (384 dim) via ONNX Runtime.
//!                        Enabled by the `onnx` feature; requires the ort crate
//!                        and a pre-downloaded model.

use thiserror::Error;

// ── EmbeddingError ────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum EmbeddingError {
    #[error("Model not loaded: {0}")]
    ModelNotLoaded(String),
    #[error("Encoding failed: {0}")]
    EncodingFailed(String),
    #[error("Dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },
    #[error("IO error: {0}")]
    Io(String),
}

// ── EmbeddingEngine trait ─────────────────────────────────────────────────────

/// Swappable embedding backend.  All implementations must be Send + Sync so
/// they can be held behind a `Box<dyn EmbeddingEngine>` in `ParietalLobe`.
pub trait EmbeddingEngine: Send + Sync {
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError>;
    fn dimension(&self) -> usize;
    fn model_name(&self) -> &str;
}

// ── Free functions ────────────────────────────────────────────────────────────

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32  = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let na:  f32  = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb:  f32  = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na * nb) }
}

/// Normalize a vector to unit length in-place.
pub fn normalize(v: &mut Vec<f32>) {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

// ── TF-IDF tokenization helpers ───────────────────────────────────────────────

/// Stop words excluded from token-projection embeddings.
pub const EMBED_STOP_WORDS: &[&str] = &[
    "the", "a", "an", "is", "are", "was", "were", "be", "been",
    "and", "or", "but", "in", "on", "at", "to", "for", "of",
    "it", "its", "by", "from", "with", "that", "this", "which",
    "as", "not", "also", "can", "may", "will", "has", "have",
    "had", "use", "used", "uses", "both", "more", "than",
    "do", "does", "did", "how", "what", "when", "where", "who",
    "why", "their", "they", "them", "about", "very", "just",
    "into", "over", "such", "each", "many", "most", "being",
];

/// Tokenise `text` into lowercase stems, removing stop words and short tokens.
/// Also appends bigrams for adjacent significant pairs.
pub fn embed_tokenize(text: &str) -> Vec<String> {
    let tokens: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .filter(|s| s.len() >= 3 && !EMBED_STOP_WORDS.contains(&s.as_str()))
        .collect();

    let mut result = tokens.clone();
    // Bigrams encode phrase-level meaning: "machine_learning" ≠ "machine" alone.
    for i in 0..tokens.len().saturating_sub(1) {
        result.push(format!("{}_{}", tokens[i], tokens[i + 1]));
    }
    result
}

/// FNV-1a hash — fast and well-distributed for token → dimension mapping.
fn fnv1a(s: &str) -> u64 {
    s.bytes().fold(0xcbf29ce484222325_u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100000001b3)
    })
}

// ── StubEngine ────────────────────────────────────────────────────────────────

/// TF-IDF token-projection stub — deterministic, no native libraries needed.
///
/// Each unique token is hashed to a sparse set of dimensions.  Two texts that
/// share vocabulary will overlap in those dimensions, producing a positive
/// cosine similarity proportional to vocabulary overlap.
///
/// This is significantly better than a pure hash-of-whole-text approach:
/// similar topics cluster together, enabling meaningful semantic search.
pub struct StubEngine {
    dimension: usize,
}

impl StubEngine {
    pub fn new(dimension: usize) -> Self {
        assert!(dimension > 0, "dimension must be positive");
        StubEngine { dimension }
    }
}

impl EmbeddingEngine for StubEngine {
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let tokens = embed_tokenize(text);
        let dim = self.dimension;
        let mut v = vec![0.0f32; dim];

        if tokens.is_empty() {
            // Fallback for text that contains only stop words / short tokens:
            // use a deterministic character hash so the vector is non-zero.
            let seed = text
                .bytes()
                .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
            for i in 0..dim {
                let x = seed
                    .wrapping_add(i as u64)
                    .wrapping_mul(6_364_136_223_846_793_005_u64);
                v[i] = ((x >> 33) as f32 / u32::MAX as f32) * 2.0 - 1.0;
            }
        } else {
            // Token-projection: each token scatters weight across 4 dimensions.
            // Bigrams use a slightly lower weight (0.7) to avoid over-weighting
            // phrase matches relative to individual term matches.
            for token in &tokens {
                let h = fnv1a(token);
                let weight: f32 = if token.contains('_') { 0.7 } else { 1.0 };
                for k in 0..4usize {
                    let h2 = h.wrapping_add(k as u64).wrapping_mul(0x9e3779b97f4a7c15);
                    let d = (h2 % dim as u64) as usize;
                    // Sign is determined by bit 63 of the hash — uniformly ±1.
                    let sign: f32 = if (h2 >> 63) == 0 { 1.0 } else { -1.0 };
                    v[d] += sign * weight;
                }
            }
        }

        normalize(&mut v);
        Ok(v)
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        texts.iter().map(|t| self.embed(t)).collect()
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_name(&self) -> &str {
        "stub-tfidf-384"
    }
}

// ── BgeSmallEngine ────────────────────────────────────────────────────────────

/// BGE-small-en-v1.5 — 384-dimensional sentence embeddings.
///
/// Usage:
/// 1. Download the ONNX model: `BAAI/bge-small-en-v1.5` from HuggingFace.
/// 2. Enable the `onnx` Cargo feature.
/// 3. Call `BgeSmallEngine::new(model_path)`.
pub struct BgeSmallEngine {
    pub model_path: std::path::PathBuf,
}

impl BgeSmallEngine {
    pub const DIMENSION: usize = 384;
    pub const MODEL_NAME: &'static str = "BAAI/bge-small-en-v1.5";

    pub fn new(model_path: std::path::PathBuf) -> Self {
        BgeSmallEngine { model_path }
    }
}

impl EmbeddingEngine for BgeSmallEngine {
    fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbeddingError> {
        Err(EmbeddingError::ModelNotLoaded(
            "BgeSmallEngine requires the 'onnx' feature and a downloaded model. \
             Use StubEngine for development."
                .to_string(),
        ))
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        texts.iter().map(|t| self.embed(t)).collect()
    }

    fn dimension(&self) -> usize {
        Self::DIMENSION
    }

    fn model_name(&self) -> &str {
        Self::MODEL_NAME
    }
}

// ── BGE-small ONNX semantic embedding (CattleDog keystone) ──────────────────
// Real dense embeddings via fastembed (ort/ONNX Runtime under the hood) loading
// the locally-downloaded BGE-small-en-v1.5 model + tokenizer. No npm, no axios.
use std::sync::OnceLock;

fn _bge_dir() -> std::path::PathBuf {
    std::env::var("LU_BGE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("v2/models/bge-small-en-v1.5"))
}

static _BGE: OnceLock<Option<fastembed::TextEmbedding>> = OnceLock::new();

fn _bge() -> Option<&'static fastembed::TextEmbedding> {
    _BGE.get_or_init(|| {
        let d = _bge_dir();
        let rd = |f: &str| std::fs::read(d.join(f)).ok();
        let tok = fastembed::TokenizerFiles {
            tokenizer_file: rd("tokenizer.json")?,
            config_file: rd("config.json")?,
            special_tokens_map_file: rd("special_tokens_map.json")?,
            tokenizer_config_file: rd("tokenizer_config.json")?,
        };
        let onnx = rd("model.onnx")?;
        let m = fastembed::UserDefinedEmbeddingModel::new(onnx, tok)
            .with_pooling(fastembed::Pooling::Cls);
        match fastembed::TextEmbedding::try_new_from_user_defined(
            m, fastembed::InitOptionsUserDefined::default()) {
            Ok(t) => { eprintln!("[bge] BGE-small ONNX embedder loaded"); Some(t) }
            Err(e) => { eprintln!("[bge] init failed, falling back to lexical: {e}"); None }
        }
    }).as_ref()
}

/// Real dense semantic embedding (BGE-small-en-v1.5, 384-dim). Falls back to the
/// lexical StubEngine if the model cannot load.
pub fn semantic_embed(text: &str) -> Vec<f32> {
    if let Some(engine) = _bge() {
        if let Ok(mut v) = engine.embed(vec![text], None) {
            if let Some(e) = v.pop() { return e; }
        }
    }
    StubEngine::new(384).embed(text).unwrap_or_else(|_| vec![0.0; 384])
}
