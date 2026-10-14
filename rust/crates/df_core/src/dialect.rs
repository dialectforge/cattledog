// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Dialect
//!
//! A dialect is a seed-derived encoding scheme. Two DF speakers
//! negotiate a shared dialect from their seeds. The dialect
//! determines how data is encoded between them.
//!
//! In the brain: the terminal and brain negotiate a dialect.
//! All commands flow through this dialect. External data that
//! doesn't speak the dialect is just data — never commands.
//!
//! The dialect evolves. The seed opens the door to the room;
//! the AI writes the dictionary inside.

use crate::seed::Seed;
// sha2 will be used when DF dialect encryption is implemented in v1
use serde::{Serialize, Deserialize};

/// A negotiated encoding scheme between two DF speakers.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dialect {
    /// The shared seed this dialect derives from
    shared_seed: Seed,
    /// Encoding map: maps byte patterns to dialect tokens
    /// Starts simple, evolves through use
    encoding_map: Vec<(Vec<u8>, Vec<u8>)>,
    /// How many times this dialect has been refreshed
    generation: u64,
    /// When this dialect was last negotiated
    negotiated_at: chrono::DateTime<chrono::Utc>,
}

impl Dialect {
    /// Create a new dialect from two seeds (negotiation).
    /// The shared seed is the XOR of both — neither party
    /// controls it alone.
    pub fn negotiate(seed_a: &Seed, seed_b: &Seed) -> Self {
        let shared_seed = seed_a.xor(seed_b);
        Self {
            shared_seed,
            encoding_map: Vec::new(),
            generation: 0,
            negotiated_at: chrono::Utc::now(),
        }
    }

    /// Encode data through the dialect.
    /// In early stages this is simple XOR masking.
    /// As the dialect evolves, compression emerges.
    pub fn encode(&self, data: &[u8]) -> Vec<u8> {
        // First apply any learned compression mappings
        let compressed = self.compress(data);
        // Then mask with the shared seed
        self.mask(&compressed)
    }

    /// Decode data from the dialect.
    pub fn decode(&self, encoded: &[u8]) -> Vec<u8> {
        let unmasked = self.unmask(encoded);
        self.decompress(&unmasked)
    }

    /// Add a compression mapping: when we see `pattern`,
    /// encode it as `token` (shorter). This is evolution.
    pub fn learn_compression(&mut self, pattern: Vec<u8>, token: Vec<u8>) {
        // Only add if token is actually shorter
        if token.len() < pattern.len() {
            self.encoding_map.push((pattern, token));
        }
    }

    /// Refresh the dialect. Derives a new generation from
    /// the shared seed + generation counter.
    pub fn refresh(&mut self) {
        self.generation += 1;
        self.shared_seed = self.shared_seed.derive(
            &self.generation.to_le_bytes()
        );
        self.negotiated_at = chrono::Utc::now();
    }

    /// The shared seed for this dialect.
    pub fn shared_seed(&self) -> &Seed {
        &self.shared_seed
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    // --- Internal ---

    fn compress(&self, data: &[u8]) -> Vec<u8> {
        if self.encoding_map.is_empty() {
            return data.to_vec();
        }
        let mut result = data.to_vec();
        for (pattern, token) in &self.encoding_map {
            // Simple find-and-replace compression
            // Will become more sophisticated as dialect evolves
            result = replace_bytes(&result, pattern, token);
        }
        result
    }

    fn decompress(&self, data: &[u8]) -> Vec<u8> {
        if self.encoding_map.is_empty() {
            return data.to_vec();
        }
        let mut result = data.to_vec();
        // Reverse order for decompression
        for (pattern, token) in self.encoding_map.iter().rev() {
            result = replace_bytes(&result, token, pattern);
        }
        result
    }

    fn mask(&self, data: &[u8]) -> Vec<u8> {
        let key = self.shared_seed.as_bytes();
        data.iter()
            .enumerate()
            .map(|(i, b)| b ^ key[i % 32])
            .collect()
    }

    fn unmask(&self, data: &[u8]) -> Vec<u8> {
        // XOR is its own inverse
        self.mask(data)
    }
}

/// Replace all occurrences of `from` with `to` in byte slice.
fn replace_bytes(data: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    if from.is_empty() {
        return data.to_vec();
    }
    let mut result = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        if i + from.len() <= data.len() && &data[i..i + from.len()] == from {
            result.extend_from_slice(to);
            i += from.len();
        } else {
            result.push(data[i]);
            i += 1;
        }
    }
    result
}
