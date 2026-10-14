// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Typed bonds — the 11 semantic relations that can hold between KnowledgeNodes.
//!
//! In v1 bonds had a free-form `label: String`.  In v2 every bond carries a
//! typed `BondType` so the traversal engine can filter, weight, and reason
//! over *relation kinds* rather than hoping the string matches.

use chrono::{DateTime, Utc};
use df_core::Seed;
use serde::{Deserialize, Serialize};

// ── BondType ──────────────────────────────────────────────────────────────────

/// The 11 typed semantic relations in Lu's knowledge graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BondType {
    /// "A IS_A B" — taxonomy / inheritance.  (dog IS_A animal)
    IsA,
    /// "A HAS_PART B" — structural composition.  (car HAS_PART engine)
    HasPart,
    /// "A CAUSES B" — causal relationship.  (friction CAUSES heat)
    Causes,
    /// "A REQUIRES B" — dependency / prerequisite.  (login REQUIRES password)
    Requires,
    /// "A OPPOSES B" — contradiction or antonymy.  (order OPPOSES chaos)
    Opposes,
    /// "A EXAMPLE_OF B" — instantiation.  (Rust EXAMPLE_OF systems language)
    ExampleOf,
    /// "A SEQUENCE B" — temporal / procedural ordering.  (step 1 SEQUENCE step 2)
    Sequence,
    /// "A DERIVES_FROM B" — origin or derivation.  (Haskell DERIVES_FROM ML)
    DerivesFrom,
    /// "A DEPICTS B" — representational (image/audio depicting a concept).
    Depicts,
    /// "A IMPLEMENTS B" — code/pattern implementing an abstraction.
    Implements,
    /// "A CONTEXT B" — A is valid or meaningful only within context B.
    Context,
}

impl BondType {
    pub fn label(&self) -> &'static str {
        match self {
            BondType::IsA        => "IS_A",
            BondType::HasPart    => "HAS_PART",
            BondType::Causes     => "CAUSES",
            BondType::Requires   => "REQUIRES",
            BondType::Opposes    => "OPPOSES",
            BondType::ExampleOf  => "EXAMPLE_OF",
            BondType::Sequence   => "SEQUENCE",
            BondType::DerivesFrom => "DERIVES_FROM",
            BondType::Depicts    => "DEPICTS",
            BondType::Implements => "IMPLEMENTS",
            BondType::Context    => "CONTEXT",
        }
    }

    /// Returns true if the inverse of this bond is the same type.
    /// (Symmetric relations: OPPOSES, CONTEXT)
    pub fn is_symmetric(&self) -> bool {
        matches!(self, BondType::Opposes | BondType::Context)
    }

    /// Weight multiplier used during typed recall traversal.
    /// Stronger default weight = higher structural priority in spreading activation.
    pub fn default_weight(&self) -> f32 {
        match self {
            BondType::IsA         => 1.5,
            BondType::HasPart     => 1.2,
            BondType::Causes      => 1.3,
            BondType::Requires    => 1.1,
            BondType::Opposes     => 0.8,
            BondType::ExampleOf   => 1.0,
            BondType::Sequence    => 1.2,
            BondType::DerivesFrom => 1.0,
            BondType::Depicts     => 0.9,
            BondType::Implements  => 1.1,
            BondType::Context     => 0.7,
        }
    }
}

// ── TypedBond ─────────────────────────────────────────────────────────────────

/// A directed, weighted, typed edge between two KnowledgeNodes.
///
/// The bond's own `seed` is derived from `source XOR target` hashed with the
/// bond type label — the same pair of nodes always produces the same bond seed
/// for a given relation type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedBond {
    /// Unique identity of this bond.
    pub seed: Seed,
    pub source_seed: Seed,
    pub target_seed: Seed,
    pub bond_type: BondType,
    /// Activation weight — grows with use (Hebbian), decays without.
    pub weight: f32,
    pub created_at: DateTime<Utc>,
    pub activation_count: u64,
    pub active: bool,
    pub revocation_reason: Option<String>,
}

impl TypedBond {
    pub fn new(source: Seed, target: Seed, bond_type: BondType, weight: f32) -> Self {
        let seed = source.xor(&target).derive(bond_type.label().as_bytes());
        TypedBond {
            seed,
            source_seed: source,
            target_seed: target,
            bond_type,
            weight: weight.clamp(0.0, 10.0),
            created_at: Utc::now(),
            activation_count: 0,
            active: true,
            revocation_reason: None,
        }
    }

    pub fn strengthen(&mut self, amount: f32) {
        self.weight = (self.weight + amount).min(10.0);
        self.activation_count += 1;
    }

    pub fn weaken(&mut self, amount: f32) {
        self.weight = (self.weight - amount).max(0.0);
    }

    pub fn revoke(&mut self, reason: &str) {
        self.active = false;
        self.revocation_reason = Some(reason.to_string());
    }

    pub fn is_dead(&self) -> bool {
        !self.active || self.weight <= 0.0
    }
}
