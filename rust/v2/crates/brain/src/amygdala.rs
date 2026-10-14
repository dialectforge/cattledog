// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Amygdala — scar system v2, emotional tags, confidence modulation.
//!
//! In v2 scars carry embedding fingerprints so the amygdala can warn on
//! *semantic* similarity to revoked content, not just exact seed matches.
//! Emotional tags weight recall: high-arousal nodes get extra activation.

use chrono::{DateTime, Utc};
use df_core::Seed;
use graph::network::ActivatedNode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── ScarV2 ────────────────────────────────────────────────────────────────────

/// A permanent record of revoked knowledge — the path is severed, not hidden.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScarV2 {
    pub id: String,
    pub revoked_seed: Seed,
    pub label: String,
    pub reason: String,
    pub revoked_at: DateTime<Utc>,
    /// BGE-small embedding snapshot of the revoked content.
    /// Used to detect semantically similar incoming proposals.
    pub embedding_fingerprint: Option<Vec<f32>>,
    /// How strongly this scar should suppress similar content (0.0–1.0).
    pub emotional_weight: f32,
    /// Number of times this scar has been triggered on incoming content.
    pub trigger_count: u64,
}

impl ScarV2 {
    pub fn new(
        seed: Seed,
        label: impl Into<String>,
        reason: impl Into<String>,
        embedding: Option<Vec<f32>>,
        emotional_weight: f32,
    ) -> Self {
        ScarV2 {
            id: uuid::Uuid::new_v4().to_string(),
            revoked_seed: seed,
            label: label.into(),
            reason: reason.into(),
            revoked_at: Utc::now(),
            embedding_fingerprint: embedding,
            emotional_weight: emotional_weight.clamp(0.0, 1.0),
            trigger_count: 0,
        }
    }

    /// Check if an incoming embedding is dangerously close to this scar.
    /// Returns Some(similarity) if above the suppression threshold.
    pub fn check_similarity(&self, incoming: &[f32], threshold: f32) -> Option<f32> {
        let fp = self.embedding_fingerprint.as_deref()?;
        let sim = cosine_similarity(fp, incoming);
        if sim >= threshold {
            Some(sim)
        } else {
            None
        }
    }
}

// ── EmotionalTag ──────────────────────────────────────────────────────────────

/// Valence + arousal annotation for a KnowledgeNode.
/// Used to bias recall: high-arousal nodes surface faster, negative nodes are damped.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionalTag {
    /// Seed of the KnowledgeNode being tagged.
    pub node_seed: Seed,
    /// -1.0 = very negative, 0.0 = neutral, 1.0 = very positive
    pub valence: f32,
    /// 0.0 = calm / low salience, 1.0 = highly salient / urgent
    pub arousal: f32,
    /// Confidence in this emotional annotation.
    pub confidence: f32,
    pub updated_at: DateTime<Utc>,
}

impl EmotionalTag {
    pub fn new(node_seed: Seed, valence: f32, arousal: f32, confidence: f32) -> Self {
        EmotionalTag {
            node_seed,
            valence: valence.clamp(-1.0, 1.0),
            arousal: arousal.clamp(0.0, 1.0),
            confidence: confidence.clamp(0.0, 1.0),
            updated_at: Utc::now(),
        }
    }

    /// Activation multiplier: positive, high-arousal tags boost recall.
    pub fn activation_multiplier(&self) -> f32 {
        1.0 + self.arousal * self.confidence * (self.valence * 0.5 + 0.5)
    }
}

// ── Amygdala ──────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct Amygdala {
    pub scars: Vec<ScarV2>,
    /// node seed hex → emotional tag
    pub emotional_tags: HashMap<String, EmotionalTag>,
    /// Similarity threshold above which incoming content triggers a scar warning.
    pub scar_similarity_threshold: f32,
}

impl Amygdala {
    pub fn new() -> Self {
        Amygdala {
            scars: Vec::new(),
            emotional_tags: HashMap::new(),
            scar_similarity_threshold: 0.85,
        }
    }

    /// Record a new scar when a node is revoked.
    pub fn record_scar(
        &mut self,
        seed: Seed,
        label: impl Into<String>,
        reason: impl Into<String>,
        embedding: Option<Vec<f32>>,
    ) -> &ScarV2 {
        let scar = ScarV2::new(seed, label, reason, embedding, 0.8);
        self.scars.push(scar);
        self.scars.last().unwrap()
    }

    /// Check an incoming embedding against all scars.
    /// Returns descriptions of any scars triggered.
    pub fn check_proposal(&mut self, embedding: &[f32]) -> Vec<String> {
        let threshold = self.scar_similarity_threshold;
        let mut warnings: Vec<String> = Vec::new();
        for scar in &mut self.scars {
            if let Some(sim) = scar.check_similarity(embedding, threshold) {
                scar.trigger_count += 1;
                warnings.push(format!(
                    "SCAR: '{}' — {} (similarity: {:.2})",
                    scar.label, scar.reason, sim
                ));
            }
        }
        warnings
    }

    pub fn tag_node(&mut self, seed: Seed, valence: f32, arousal: f32, confidence: f32) {
        let tag = EmotionalTag::new(seed.clone(), valence, arousal, confidence);
        self.emotional_tags.insert(seed.to_hex(), tag);
    }

    pub fn get_tag(&self, seed: &Seed) -> Option<&EmotionalTag> {
        self.emotional_tags.get(&seed.to_hex())
    }

    pub fn activation_boost(&self, seed: &Seed) -> f32 {
        self.get_tag(seed)
            .map(|t| t.activation_multiplier())
            .unwrap_or(1.0)
    }

    /// Apply emotional activation boosts (or penalties) to a mutable slice of
    /// recalled nodes.  Nodes tagged with positive high-arousal emotions get a
    /// higher recall strength; untagged nodes are unchanged (multiplier = 1.0).
    pub fn apply_recall_boosts(&self, nodes: &mut Vec<ActivatedNode>) {
        for node in nodes.iter_mut() {
            let boost = self.activation_boost(&node.seed);
            if (boost - 1.0).abs() > f32::EPSILON {
                node.strength *= boost;
            }
        }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na * nb) }
}
