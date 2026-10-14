// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Parietal lobe — semantic embedding and cross-modal binding.
//!
//! The parietal lobe computes BGE-small embeddings for incoming KUs and
//! binds related representations across modalities (text + image + audio).

use df_core::Seed;
use embedding::EmbeddingEngine;
use std::collections::HashMap;

// ── CrossModalBinding ─────────────────────────────────────────────────────────

/// A fused representation that spans multiple modalities for one concept.
#[derive(Debug, Clone)]
pub struct CrossModalBinding {
    /// The primary text node this binding is anchored to.
    pub text_seed: Seed,
    pub image_seed: Option<Seed>,
    pub audio_seed: Option<Seed>,
    /// The fused embedding — average of all available modality embeddings.
    pub combined_embedding: Vec<f32>,
    pub binding_strength: f32,
}

// ── EmbeddingRecord ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EmbeddingRecord {
    pub seed: Seed,
    pub embedding: Vec<f32>,
    pub model: String,
}

// ── ParietalLobe ──────────────────────────────────────────────────────────────

pub struct ParietalLobe {
    pub engine: Box<dyn EmbeddingEngine>,
    pub bindings: Vec<CrossModalBinding>,
    /// node seed hex → embedding (cache, avoid re-encoding)
    pub embedding_cache: HashMap<String, Vec<f32>>,
}

impl std::fmt::Debug for ParietalLobe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParietalLobe")
            .field("bindings", &self.bindings.len())
            .field("cache_size", &self.embedding_cache.len())
            .field("engine", &self.engine.model_name())
            .finish()
    }
}

impl Default for ParietalLobe {
    fn default() -> Self {
        ParietalLobe::new(Box::new(embedding::StubEngine::new(384)))
    }
}

impl ParietalLobe {
    pub fn new(engine: Box<dyn EmbeddingEngine>) -> Self {
        ParietalLobe {
            engine,
            bindings: Vec::new(),
            embedding_cache: HashMap::new(),
        }
    }

    /// Embed text and cache the result under the node seed.
    pub fn embed_node(&mut self, seed: &Seed, text: &str) -> Result<Vec<f32>, embedding::EmbeddingError> {
        let hex = seed.to_hex();
        if let Some(cached) = self.embedding_cache.get(&hex) {
            return Ok(cached.clone());
        }
        let embedding = self.engine.embed(text)?;
        self.embedding_cache.insert(hex, embedding.clone());
        Ok(embedding)
    }

    /// Compute cosine similarity between two embeddings.
    pub fn similarity(a: &[f32], b: &[f32]) -> f32 {
        embedding::cosine_similarity(a, b)
    }

    /// Find the top-k most similar cached embeddings to a query embedding.
    pub fn nearest_neighbors(&self, query: &[f32], top_k: usize) -> Vec<(String, f32)> {
        let mut scored: Vec<(String, f32)> = self
            .embedding_cache
            .iter()
            .map(|(hex, emb)| (hex.clone(), embedding::cosine_similarity(query, emb)))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        scored
    }

    /// Bind a text node with an optional image or audio node.
    pub fn bind(&mut self, text_seed: Seed, image_seed: Option<Seed>, audio_seed: Option<Seed>) {
        // Collect embeddings for each modality
        let mut embeds: Vec<Vec<f32>> = Vec::new();
        for seed in [Some(&text_seed), image_seed.as_ref(), audio_seed.as_ref()]
            .into_iter()
            .flatten()
        {
            if let Some(e) = self.embedding_cache.get(&seed.to_hex()) {
                embeds.push(e.clone());
            }
        }

        if embeds.is_empty() {
            return;
        }

        let dim = embeds[0].len();
        let mut combined = vec![0.0f32; dim];
        for e in &embeds {
            for (c, v) in combined.iter_mut().zip(e.iter()) {
                *c += v;
            }
        }
        let n = embeds.len() as f32;
        for c in &mut combined {
            *c /= n;
        }

        self.bindings.push(CrossModalBinding {
            text_seed,
            image_seed,
            audio_seed,
            combined_embedding: combined,
            binding_strength: 1.0 / n,
        });
    }
}
