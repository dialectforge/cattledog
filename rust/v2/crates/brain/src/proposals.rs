// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Proposal approval pipeline — queued changes awaiting human approval.
//!
//! During think_through() the brain may generate structural proposals
//! (create bonds, learn insights, revoke nodes, etc.).  These are queued
//! rather than auto-applied, allowing a human supervisor to approve or reject
//! each one via lu_approve / lu_reject.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── ThinkingProposal ──────────────────────────────────────────────────────────

/// A structural change proposed by the reasoning engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThinkingProposal {
    /// Create a new bond between two existing nodes.
    CreateBond {
        source_seed: String,
        target_seed: String,
        bond_type: String,
        weight: f32,
    },
    /// Increase the weight of an existing bond.
    StrengthenBond {
        bond_seed: String,
        amount: f32,
    },
    /// Decrease the weight of an existing bond.
    WeakenBond {
        bond_seed: String,
        amount: f32,
    },
    /// Revoke (suppress) a node by creating an amygdala scar.
    RevokeNode {
        seed: String,
        reason: String,
    },
    /// Learn a new insight as a raw KU.
    LearnInsight {
        text: String,
        confidence: String,
    },
    /// Register a hypothesis for future exploration.
    FormHypothesis {
        hypothesis: String,
        supporting_seeds: Vec<String>,
    },
}

impl ThinkingProposal {
    /// Human-readable summary of this proposal.
    pub fn summary(&self) -> String {
        match self {
            ThinkingProposal::CreateBond { source_seed, target_seed, bond_type, weight } =>
                format!("Create {} bond ({:.2}w) between {} ↔ {}",
                    bond_type, weight, &source_seed[..8.min(source_seed.len())],
                    &target_seed[..8.min(target_seed.len())]),
            ThinkingProposal::StrengthenBond { bond_seed, amount } =>
                format!("Strengthen bond {} by {:.2}", &bond_seed[..8.min(bond_seed.len())], amount),
            ThinkingProposal::WeakenBond { bond_seed, amount } =>
                format!("Weaken bond {} by {:.2}", &bond_seed[..8.min(bond_seed.len())], amount),
            ThinkingProposal::RevokeNode { seed, reason } =>
                format!("Revoke node {} — {}", &seed[..8.min(seed.len())], reason),
            ThinkingProposal::LearnInsight { text, confidence } =>
                format!("Learn insight ({} confidence): {}", confidence, &text[..60.min(text.len())]),
            ThinkingProposal::FormHypothesis { hypothesis, .. } =>
                format!("Hypothesis: {}", &hypothesis[..80.min(hypothesis.len())]),
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            ThinkingProposal::CreateBond { .. }    => "create_bond",
            ThinkingProposal::StrengthenBond { .. } => "strengthen_bond",
            ThinkingProposal::WeakenBond { .. }    => "weaken_bond",
            ThinkingProposal::RevokeNode { .. }    => "revoke_node",
            ThinkingProposal::LearnInsight { .. }  => "learn_insight",
            ThinkingProposal::FormHypothesis { .. } => "form_hypothesis",
        }
    }
}

// ── PendingProposal ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingProposal {
    pub id: String,
    pub proposal: ThinkingProposal,
    pub created_at: DateTime<Utc>,
    /// Source context: which query triggered this proposal.
    pub source_query: String,
}

impl PendingProposal {
    pub fn new(proposal: ThinkingProposal, source_query: impl Into<String>) -> Self {
        PendingProposal {
            id: uuid::Uuid::new_v4().to_string(),
            proposal,
            created_at: Utc::now(),
            source_query: source_query.into(),
        }
    }
}

// ── PendingProposals ──────────────────────────────────────────────────────────

/// Queue of structural proposals awaiting human approval.
#[derive(Debug, Default)]
pub struct PendingProposals {
    pub proposals: Vec<PendingProposal>,
    pub total_queued: u64,
    pub total_approved: u64,
    pub total_rejected: u64,
    /// Maximum proposals to hold before evicting oldest.
    pub max_size: usize,
}

impl PendingProposals {
    pub fn new(max_size: usize) -> Self {
        PendingProposals {
            proposals: Vec::new(),
            total_queued: 0,
            total_approved: 0,
            total_rejected: 0,
            max_size,
        }
    }

    /// Queue a new proposal.  Returns its assigned ID.
    pub fn queue(&mut self, proposal: ThinkingProposal, source_query: impl Into<String>) -> String {
        if self.proposals.len() >= self.max_size {
            self.proposals.remove(0); // evict oldest
        }
        let pending = PendingProposal::new(proposal, source_query);
        let id = pending.id.clone();
        self.proposals.push(pending);
        self.total_queued += 1;
        id
    }

    /// Remove and return a proposal for approval.  Returns None if not found.
    pub fn approve(&mut self, id: &str) -> Option<PendingProposal> {
        if let Some(pos) = self.proposals.iter().position(|p| p.id == id) {
            self.total_approved += 1;
            Some(self.proposals.remove(pos))
        } else {
            None
        }
    }

    /// Remove a proposal without applying it.  Returns true if found.
    pub fn reject(&mut self, id: &str) -> bool {
        if let Some(pos) = self.proposals.iter().position(|p| p.id == id) {
            self.proposals.remove(pos);
            self.total_rejected += 1;
            true
        } else {
            false
        }
    }

    /// All pending proposals (not yet approved or rejected).
    pub fn pending(&self) -> &[PendingProposal] {
        &self.proposals
    }

    pub fn pending_count(&self) -> usize {
        self.proposals.len()
    }
}
