// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! KnowledgeNode — the atom of Lu's understanding.
//!
//! In v1 a node stored a pattern vector and a free-form label.
//! In v2 a node stores a typed KnowledgeUnit (semantic content) PLUS an
//! optional semantic_embedding (computed by the parietal lobe's BGE-small
//! model).  The two representations are complementary: the KU is for
//! narration and logic; the embedding is for similarity search.

use chrono::{DateTime, Utc};
use df_core::{Seed, SeedGated};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::knowledge_unit::KnowledgeUnit;

// ── Confidence ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Confidence {
    /// Came directly through the terminal bond — highest trust.
    Terminal,
    /// Verified through multiple independent associations.
    Verified,
    /// Single source, not yet cross-referenced.
    Unverified,
    /// Contradicts existing knowledge — needs review.
    Contested,
    /// Derived by reasoning, not directly observed.
    Inferred,
}

impl Default for Confidence {
    fn default() -> Self {
        Confidence::Unverified
    }
}

impl Confidence {
    pub fn weight(&self) -> f32 {
        match self {
            Confidence::Terminal   => 1.0,
            Confidence::Verified   => 0.9,
            Confidence::Unverified => 0.6,
            Confidence::Inferred   => 0.5,
            Confidence::Contested  => 0.2,
        }
    }
}

// ── KnowledgeNode ─────────────────────────────────────────────────────────────

/// A typed, seed-gated node in Lu's associative knowledge graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeNode {
    seed: Seed,
    /// The semantic content — structured per KU type.
    pub knowledge_unit: KnowledgeUnit,
    /// BGE-small embedding for similarity search (384-dim).
    /// None until the parietal lobe has processed this node.
    pub semantic_embedding: Option<Vec<f32>>,
    pub confidence: Confidence,
    pub activation_count: u64,
    pub created_at: DateTime<Utc>,
    /// Free-form metadata: filename, source URL, dimensions, etc.
    pub metadata: HashMap<String, String>,
    active: bool,
    revocation_reason: Option<String>,
}

impl KnowledgeNode {
    pub fn new(knowledge_unit: KnowledgeUnit, confidence: Confidence) -> Self {
        let seed = Seed::generate();
        KnowledgeNode {
            seed,
            knowledge_unit,
            semantic_embedding: None,
            confidence,
            activation_count: 0,
            created_at: Utc::now(),
            metadata: HashMap::new(),
            active: true,
            revocation_reason: None,
        }
    }

    pub fn with_seed(seed: Seed, knowledge_unit: KnowledgeUnit, confidence: Confidence) -> Self {
        KnowledgeNode {
            seed,
            knowledge_unit,
            semantic_embedding: None,
            confidence,
            activation_count: 0,
            created_at: Utc::now(),
            metadata: HashMap::new(),
            active: true,
            revocation_reason: None,
        }
    }

    /// Record an activation and return the effective activation strength.
    pub fn activate(&mut self) -> f32 {
        self.activation_count += 1;
        self.confidence.weight() * (1.0 + (self.activation_count as f32).ln() * 0.1)
    }

    /// Human-readable label derived from the knowledge unit.
    pub fn label(&self) -> String {
        self.knowledge_unit.label()
    }

    pub fn revocation_reason(&self) -> Option<&str> {
        self.revocation_reason.as_deref()
    }
}

impl SeedGated for KnowledgeNode {
    fn seed(&self) -> &Seed {
        &self.seed
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn revoke(&mut self, reason: &str) {
        self.active = false;
        self.revocation_reason = Some(reason.to_string());
    }
}
