// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Seed Gating
//!
//! Data does not exist without its seed. The seed is not a lock
//! placed on top of data — the seed is how the data comes into
//! existence. Revoking the seed doesn't hide the data. It
//! destroys the path to it.
//!
//! SeedGated wraps any data with its seed. Access requires the seed.
//! This is the architectural basis for surgical revocation.

use crate::seed::Seed;
use sha2::{Sha256, Digest};
use serde::{Serialize, Deserialize};

/// Data that only exists through its seed.
/// The seed gates creation, access, and revocation.
#[derive(Clone, Serialize, Deserialize)]
pub struct GatedData {
    /// The seed that brought this data into existence
    pub seed: Seed,
    /// The data, XOR-masked with seed-derived key.
    /// Not military-grade encryption — structural gating.
    /// The point is: without the seed, this is noise.
    masked: Vec<u8>,
    /// Length of original data (masking preserves length)
    len: usize,
}

impl GatedData {
    /// Create new gated data. The seed is generated, the data
    /// is masked through the seed. This is birth.
    pub fn create(data: &[u8]) -> Self {
        let seed = Seed::generate();
        let mask = Self::derive_mask(&seed, data.len());
        let masked: Vec<u8> = data.iter()
            .zip(mask.iter())
            .map(|(d, m)| d ^ m)
            .collect();
        Self { seed, masked, len: data.len() }
    }

    /// Create with a specific seed (for deterministic operations).
    pub fn create_with_seed(seed: Seed, data: &[u8]) -> Self {
        let mask = Self::derive_mask(&seed, data.len());
        let masked: Vec<u8> = data.iter()
            .zip(mask.iter())
            .map(|(d, m)| d ^ m)
            .collect();
        Self { seed, masked, len: data.len() }
    }

    /// Access the data. Requires the matching seed.
    /// Returns None if the seed doesn't match.
    pub fn access(&self, seed: &Seed) -> Option<Vec<u8>> {
        if &self.seed != seed {
            return None;
        }
        let mask = Self::derive_mask(seed, self.len);
        let data: Vec<u8> = self.masked.iter()
            .zip(mask.iter())
            .map(|(d, m)| d ^ m)
            .collect();
        Some(data)
    }

    /// Derive a repeatable mask from a seed.
    fn derive_mask(seed: &Seed, len: usize) -> Vec<u8> {
        let mut mask = Vec::with_capacity(len);
        let mut counter: u64 = 0;
        while mask.len() < len {
            let mut hasher = Sha256::new();
            hasher.update(seed.as_bytes());
            hasher.update(&counter.to_le_bytes());
            let block = hasher.finalize();
            mask.extend_from_slice(&block[..std::cmp::min(32, len - mask.len())]);
            counter += 1;
        }
        mask.truncate(len);
        mask
    }
}

impl std::fmt::Debug for GatedData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GatedData(seed={}, {} bytes)", self.seed.short(), self.len)
    }
}

/// Trait for anything that is gated by a seed.
/// Nodes, bonds, scars — they all implement this.
pub trait SeedGated {
    /// The seed that gates this entity
    fn seed(&self) -> &Seed;

    /// Whether this entity is still active (not revoked)
    fn is_active(&self) -> bool;

    /// Revoke this entity. The seed stays as a scar marker,
    /// but the entity is no longer accessible.
    fn revoke(&mut self, reason: &str);
}
