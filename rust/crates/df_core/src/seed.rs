// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Seed Generation
//!
//! A Seed is a 256-bit unique identifier. It is the key to everything.
//! Every knowledge atom, every bond, every scar has one.
//!
//! The seed is not a lock placed on top of data.
//! The seed is how the data comes into existence.
//! Without the seed, the data was never created.

use sha2::{Sha256, Digest};
use rand::Rng;
use serde::{Serialize, Deserialize};
use std::fmt;

/// The atomic identity unit. 256-bit, unique, non-reusable.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Seed {
    bytes: [u8; 32],
}

impl Seed {
    /// Generate a new seed from system entropy.
    /// Every call produces a unique, unrepeatable seed.
    pub fn generate() -> Self {
        let mut rng = rand::rng();
        let mut entropy = [0u8; 64];
        rng.fill(&mut entropy[..]);

        // Mix in timestamp for additional uniqueness
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let time_bytes = nanos.to_le_bytes();
        entropy[48..64].copy_from_slice(&time_bytes);

        let mut hasher = Sha256::new();
        hasher.update(&entropy);
        let result = hasher.finalize();

        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&result);
        Self { bytes }
    }

    /// Derive a child seed from a parent seed and context.
    /// Deterministic: same parent + context = same child.
    /// This is how DF operations are seed-derived.
    pub fn derive(&self, context: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(&self.bytes);
        hasher.update(context);
        let result = hasher.finalize();

        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&result);
        Self { bytes }
    }

    /// Raw bytes access for cryptographic operations.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.bytes
    }

    /// Short hex display (first 16 chars) for seed reports.
    pub fn short(&self) -> String {
        hex::encode(&self.bytes[..8])
    }

    /// Full hex display.
    pub fn to_hex(&self) -> String {
        hex::encode(&self.bytes)
    }

    /// Create from hex string (for deserialization/loading).
    pub fn from_hex(hex_str: &str) -> Result<Self, SeedError> {
        let decoded = hex::decode(hex_str)
            .map_err(|_| SeedError::InvalidHex)?;
        if decoded.len() != 32 {
            return Err(SeedError::InvalidLength);
        }
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&decoded);
        Ok(Self { bytes })
    }

    /// XOR two seeds together (used in dialect negotiation).
    pub fn xor(&self, other: &Seed) -> Self {
        let mut bytes = [0u8; 32];
        for i in 0..32 {
            bytes[i] = self.bytes[i] ^ other.bytes[i];
        }
        Self { bytes }
    }
}

impl fmt::Debug for Seed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Seed({}...)", self.short())
    }
}

impl fmt::Display for Seed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.short())
    }
}

#[derive(Debug)]
pub enum SeedError {
    InvalidHex,
    InvalidLength,
}

impl fmt::Display for SeedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SeedError::InvalidHex => write!(f, "Invalid hex string"),
            SeedError::InvalidLength => write!(f, "Seed must be exactly 32 bytes"),
        }
    }
}

impl std::error::Error for SeedError {}
