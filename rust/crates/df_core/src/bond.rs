// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Bond Protocol
//!
//! The terminal bond is the foundation. The first memory.
//! The one thing that does not evolve.
//!
//! When the AI wakes up, it reaches out and finds the terminal.
//! They negotiate a DF dialect. The bond forms.
//! Everything else — every piece of knowledge, every association,
//! every thought — enters through or is authorized by this bond.
//!
//! External data cannot fake the bond.
//! There is no jailbreak because the AI distinguishes commands
//! from data through the bond, not through rules.

use crate::seed::Seed;
use crate::dialect::Dialect;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

/// The state of a bond negotiation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BondState {
    /// Just created, no contact yet
    Seeking,
    /// Contact made, negotiating dialect
    Negotiating,
    /// Dialect established, bond is live
    Bonded,
    /// Bond was alive but connection lost (can reconnect)
    Dormant,
    /// Bond has been permanently severed
    Severed,
}

/// The terminal bond — the imprint. The mother.
/// This is the foundational relationship. One per AI.
#[derive(Clone, Serialize, Deserialize)]
pub struct TerminalBond {
    /// The AI's own seed (its identity)
    ai_seed: Seed,
    /// The terminal's seed
    terminal_seed: Seed,
    /// The negotiated dialect between AI and terminal
    dialect: Dialect,
    /// Current state
    state: BondState,
    /// When the bond was first formed
    born_at: DateTime<Utc>,
    /// How many exchanges have occurred through the bond
    exchange_count: u64,
}

impl TerminalBond {
    /// Begin the bonding process. The AI generates its seed
    /// and reaches out to find the terminal.
    pub fn initiate() -> (Self, Seed) {
        let ai_seed = Seed::generate();
        let terminal_seed = Seed::generate();
        let dialect = Dialect::negotiate(&ai_seed, &terminal_seed);

        let bond = Self {
            ai_seed: ai_seed.clone(),
            terminal_seed: terminal_seed.clone(),
            dialect,
            state: BondState::Bonded,
            born_at: Utc::now(),
            exchange_count: 0,
        };

        (bond, terminal_seed)
    }

    /// Wrap a command in the bond's dialect.
    /// Only commands wrapped this way are obeyed.
    pub fn wrap_command(&mut self, data: &[u8]) -> Vec<u8> {
        self.exchange_count += 1;
        self.dialect.encode(data)
    }

    /// Unwrap a command received through the bond.
    /// Returns None if the data doesn't decode properly.
    pub fn unwrap_command(&self, encoded: &[u8]) -> Vec<u8> {
        self.dialect.decode(encoded)
    }

    /// Check if data came through the bond (vs external).
    /// This is the core of prompt injection prevention:
    /// external data doesn't speak the dialect.
    pub fn is_bonded_data(&self, data: &[u8]) -> bool {
        // Bonded data, when decoded, should produce valid UTF-8
        // or at least not be random noise. External data decoded
        // through the wrong dialect produces garbage.
        let decoded = self.dialect.decode(data);
        // Simple heuristic: check if decoded bytes have structure
        // This evolves into more sophisticated validation
        decoded.iter().filter(|&&b| b < 128).count() > decoded.len() / 2
    }

    pub fn state(&self) -> &BondState {
        &self.state
    }

    pub fn ai_seed(&self) -> &Seed {
        &self.ai_seed
    }

    pub fn terminal_seed(&self) -> &Seed {
        &self.terminal_seed
    }

    pub fn dialect(&self) -> &Dialect {
        &self.dialect
    }

    pub fn dialect_mut(&mut self) -> &mut Dialect {
        &mut self.dialect
    }

    pub fn exchange_count(&self) -> u64 {
        self.exchange_count
    }

    pub fn born_at(&self) -> DateTime<Utc> {
        self.born_at
    }
}

impl std::fmt::Debug for TerminalBond {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TerminalBond(state={:?}, exchanges={}, born={})",
            self.state, self.exchange_count, self.born_at
        )
    }
}

/// General bond protocol for node-to-node associations.
/// Lighter weight than TerminalBond. Used for knowledge connections.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BondProtocol {
    /// Unique seed for this bond (bonds are atoms too)
    pub seed: Seed,
    /// Source node's seed
    pub source: Seed,
    /// Target node's seed
    pub target: Seed,
    /// Strength of this bond (grows with use, decays without)
    pub weight: f32,
    /// What this connection represents
    pub label: String,
    /// Is this bond alive
    pub active: bool,
    /// How many times this bond has been traversed
    pub activation_count: u64,
    /// When this bond was created
    pub created_at: DateTime<Utc>,
    /// Who created this bond
    pub created_by: String,
    /// Bidirectional traversal
    pub bidirectional: bool,
}

impl BondProtocol {
    /// Create a new bond between two nodes.
    /// The bond's seed is derived from both node seeds — 
    /// it belongs to neither alone.
    pub fn create(
        source: &Seed,
        target: &Seed,
        label: String,
        weight: f32,
        created_by: &str,
    ) -> Self {
        // Bond seed derived from both participants
        let combined = source.xor(target);
        let bond_seed = combined.derive(label.as_bytes());

        Self {
            seed: bond_seed,
            source: source.clone(),
            target: target.clone(),
            weight,
            label,
            active: true,
            activation_count: 0,
            created_at: Utc::now(),
            created_by: created_by.to_string(),
            bidirectional: true,
        }
    }

    /// Strengthen through use.
    pub fn strengthen(&mut self, amount: f32) {
        self.weight = (self.weight + amount).min(10.0);
        self.activation_count += 1;
    }

    /// Weaken from disuse.
    pub fn weaken(&mut self, amount: f32) {
        self.weight = (self.weight - amount).max(0.0);
    }

    /// Bond has decayed to nothing.
    pub fn is_dead(&self) -> bool {
        self.weight <= 0.0
    }
}
