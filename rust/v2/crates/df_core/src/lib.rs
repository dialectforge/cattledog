// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! df_core v2 — DialectForge protocol layer.
//!
//! Seeds are identities, not locks.  Every node, bond, and perception comes
//! into existence THROUGH a Seed — revocation severs paths, not data.
//! This crate is a clean copy of the v1 interface, re-exported for v2 crates.

use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ── Seed ─────────────────────────────────────────────────────────────────────

/// 256-bit unique identity.  Generated from system entropy + timestamp.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Seed {
    pub bytes: [u8; 32],
}

#[derive(Debug, thiserror::Error)]
pub enum SeedError {
    #[error("Invalid hex string: '{0}' (expected 64 hex chars)")]
    InvalidHex(String),
}

impl Seed {
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        // XOR timestamp entropy into first 8 bytes
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let ts_bytes = ts.to_le_bytes();
        for (i, b) in ts_bytes.iter().enumerate() {
            bytes[i] ^= b;
        }
        Seed { bytes }
    }

    /// Deterministic child seed — same inputs always yield the same child.
    pub fn derive(&self, context: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(self.bytes);
        hasher.update(context);
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&hasher.finalize());
        Seed { bytes }
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.bytes
    }

    pub fn to_hex(&self) -> String {
        self.bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    pub fn short(&self) -> String {
        self.to_hex()[..16].to_string()
    }

    pub fn from_hex(hex: &str) -> Result<Self, SeedError> {
        if hex.len() != 64 {
            return Err(SeedError::InvalidHex(hex.to_string()));
        }
        let mut bytes = [0u8; 32];
        for i in 0..32 {
            bytes[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
                .map_err(|_| SeedError::InvalidHex(hex.to_string()))?;
        }
        Ok(Seed { bytes })
    }

    /// XOR two seeds — used during dialect negotiation.
    pub fn xor(&self, other: &Seed) -> Self {
        let mut bytes = [0u8; 32];
        for i in 0..32 {
            bytes[i] = self.bytes[i] ^ other.bytes[i];
        }
        Seed { bytes }
    }
}

impl std::fmt::Display for Seed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.short())
    }
}

// ── SeedGated trait ───────────────────────────────────────────────────────────

pub trait SeedGated {
    fn seed(&self) -> &Seed;
    fn is_active(&self) -> bool;
    /// Severs the path to this entity — doesn't hide the data.
    fn revoke(&mut self, reason: &str);
}

// ── Dialect ───────────────────────────────────────────────────────────────────

/// Shared encoding layer negotiated between two seed-bearing parties.
/// External data that doesn't speak the dialect is never treated as a command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dialect {
    pub shared_seed: Seed,
    pub generation: u64,
    pub negotiated_at: DateTime<Utc>,
}

impl Dialect {
    pub fn negotiate(seed_a: &Seed, seed_b: &Seed) -> Self {
        Dialect {
            shared_seed: seed_a.xor(seed_b),
            generation: 0,
            negotiated_at: Utc::now(),
        }
    }

    pub fn encode(&self, data: &[u8]) -> Vec<u8> {
        let key = self.shared_seed.derive(b"encode");
        data.iter()
            .enumerate()
            .map(|(i, b)| b ^ key.bytes[i % 32])
            .collect()
    }

    pub fn decode(&self, encoded: &[u8]) -> Vec<u8> {
        // XOR cipher is symmetric
        self.encode(encoded)
    }

    /// Rotate to a new generation — invalidates previous dialect tokens.
    pub fn refresh(&mut self) {
        self.shared_seed = self.shared_seed.derive(b"refresh");
        self.generation += 1;
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn shared_seed(&self) -> &Seed {
        &self.shared_seed
    }
}

// ── BondState ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BondState {
    Seeking,
    Negotiating,
    Bonded,
    Dormant,
    Severed,
}

// ── TerminalBond ──────────────────────────────────────────────────────────────

/// The trust channel between Lu's core and its terminal.
/// All commands arrive through this bond — anything outside it is just data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalBond {
    pub ai_seed: Seed,
    pub terminal_seed: Seed,
    pub dialect: Dialect,
    pub state: BondState,
    pub born_at: DateTime<Utc>,
    pub exchange_count: u64,
}

impl TerminalBond {
    /// Create a fresh bond.  Returns the bond and the terminal's seed to share.
    pub fn initiate() -> (Self, Seed) {
        let ai_seed = Seed::generate();
        let terminal_seed = Seed::generate();
        let dialect = Dialect::negotiate(&ai_seed, &terminal_seed);
        let bond = TerminalBond {
            ai_seed: ai_seed.clone(),
            terminal_seed: terminal_seed.clone(),
            dialect,
            state: BondState::Bonded,
            born_at: Utc::now(),
            exchange_count: 0,
        };
        (bond, terminal_seed)
    }

    pub fn wrap_command(&mut self, data: &[u8]) -> Vec<u8> {
        self.exchange_count += 1;
        self.dialect.encode(data)
    }

    pub fn unwrap_command(&self, encoded: &[u8]) -> Vec<u8> {
        self.dialect.decode(encoded)
    }

    pub fn is_bonded(&self) -> bool {
        self.state == BondState::Bonded
    }

    pub fn ai_seed(&self) -> &Seed { &self.ai_seed }
    pub fn terminal_seed(&self) -> &Seed { &self.terminal_seed }
    pub fn dialect(&self) -> &Dialect { &self.dialect }
    pub fn state(&self) -> &BondState { &self.state }
    pub fn exchange_count(&self) -> u64 { self.exchange_count }
    pub fn born_at(&self) -> DateTime<Utc> { self.born_at }
}

// ── BondProtocol ──────────────────────────────────────────────────────────────

/// A weighted, directed connection between two seeded entities.
/// The bond's own seed is derived from source XOR target — making it
/// deterministic for any given pair of nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BondProtocol {
    pub seed: Seed,
    pub source: Seed,
    pub target: Seed,
    pub weight: f32,
    pub label: String,
    pub active: bool,
    pub activation_count: u64,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub bidirectional: bool,
}

impl BondProtocol {
    pub fn create(
        source: &Seed,
        target: &Seed,
        label: String,
        weight: f32,
        created_by: &str,
    ) -> Self {
        BondProtocol {
            seed: source.xor(target).derive(label.as_bytes()),
            source: source.clone(),
            target: target.clone(),
            weight: weight.clamp(0.0, 10.0),
            label,
            active: true,
            activation_count: 0,
            created_at: Utc::now(),
            created_by: created_by.to_string(),
            bidirectional: false,
        }
    }

    pub fn strengthen(&mut self, amount: f32) {
        self.weight = (self.weight + amount).min(10.0);
        self.activation_count += 1;
    }

    pub fn weaken(&mut self, amount: f32) {
        self.weight = (self.weight - amount).max(0.0);
    }

    pub fn is_dead(&self) -> bool {
        self.weight <= 0.0
    }
}
