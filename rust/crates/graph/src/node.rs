// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Knowledge Node
//!
//! The atomic unit of knowledge. A node is a DF-gated atom.
//! It comes into existence through a seed. Without the seed,
//! the node was never created. Revocation doesn't hide it —
//! it severs the path.

use std::collections::HashMap;
use df_core::{Seed, SeedGated};
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

/// What kind of knowledge this node holds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NodeType {
    /// A factual statement
    Fact,
    /// A conceptual category
    Concept,
    /// A procedural step or skill
    Procedure,
    /// An emotional association
    Emotion,
    /// A meta-observation about own state
    Meta,
    /// A media perception (image, audio, video)
    /// Label holds a descriptor; pattern holds the encoded vector.
    /// Raw bytes are stored as GatedData separately.
    Media,
}

/// How much this node is trusted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Confidence {
    /// Came directly through terminal bond (highest)
    Terminal,
    /// Verified through multiple associations
    Verified,
    /// Single source, not yet cross-referenced
    Unverified,
    /// Contradicts existing knowledge (needs review)
    Contested,
}

/// A knowledge atom in the associative network.
#[derive(Clone, Serialize, Deserialize)]
pub struct KnowledgeNode {
    /// The seed that brought this node into existence
    seed: Seed,
    /// What kind of knowledge
    pub node_type: NodeType,
    /// Human-readable label
    pub label: String,
    /// The DF-encoded pattern (internal representation)
    pub pattern: Vec<f32>,
    /// Trust level
    pub confidence: Confidence,
    /// Who/what created this node
    pub created_by: String,
    /// When this node was born
    pub created_at: DateTime<Utc>,
    /// Seeds of parent nodes this was derived from
    pub parent_seeds: Vec<Seed>,
    /// Is this node still alive
    active: bool,
    /// Why it was revoked (if it was)
    revocation_reason: Option<String>,
    /// How many times this node has been activated during recall
    pub activation_count: u64,
    /// Last time this node was activated
    pub last_activated: Option<DateTime<Utc>>,
    /// Arbitrary metadata (filename, dimensions, duration, etc.)
    /// Used by Media nodes to store origin info alongside the pattern.
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl KnowledgeNode {
    /// Birth. Create a new knowledge atom.
    pub fn new(
        node_type: NodeType,
        label: String,
        pattern: Vec<f32>,
        confidence: Confidence,
        created_by: &str,
    ) -> Self {
        Self {
            seed: Seed::generate(),
            node_type,
            label,
            pattern,
            confidence,
            created_by: created_by.to_string(),
            created_at: Utc::now(),
            parent_seeds: Vec::new(),
            active: true,
            revocation_reason: None,
            activation_count: 0,
            last_activated: None,
            metadata: HashMap::new(),
        }
    }

    /// Record an activation (this node was recalled).
    /// Returns the activation strength based on recency and use.
    pub fn activate(&mut self) -> f32 {
        self.activation_count += 1;
        self.last_activated = Some(Utc::now());
        // Strength grows with use but has diminishing returns
        1.0 - (-0.1 * self.activation_count as f32).exp().recip().recip()
    }

    /// Is this node revoked?
    pub fn is_revoked(&self) -> bool {
        !self.active
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

impl std::fmt::Debug for KnowledgeNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "KnowledgeNode(seed={}, label=\"{}\", type={:?}, active={})",
            self.seed.short(),
            self.label,
            self.node_type,
            self.active,
        )
    }
}

/// A permanent scar left by revocation.
/// Scars are warnings — when new associations try to form
/// that match a scar's pattern, the brain raises an alert.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scar {
    /// The seed that was revoked
    pub revoked_seed: Seed,
    /// What the revoked knowledge was about
    pub label: String,
    /// Why it was revoked
    pub reason: String,
    /// When revocation happened
    pub revoked_at: DateTime<Utc>,
    /// Pattern fingerprint for matching similar future inputs
    pub pattern_fingerprint: Vec<f32>,
}