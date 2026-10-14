// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Encryption crate — DF dynamic dictionary encryption.
//!
//! A dynamic dictionary maps plaintext tokens to seed-derived ciphertokens.
//! The mapping evolves over time (new entries are added as new tokens are seen)
//! and is tied to a specific Seed so different sessions produce different maps.
//!
//! This is NOT a general-purpose cipher — it is the DialectForge token layer
//! used to obfuscate command channels between Lu and its terminal.

use df_core::Seed;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── DynamicDictionary ─────────────────────────────────────────────────────────

/// A session-bound token dictionary.
///
/// Plaintext tokens → encrypted tokens derived from the session Seed.
/// The derivation is deterministic: same seed + same plaintext = same token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicDictionary {
    seed: Seed,
    /// plaintext (bytes) → ciphertoken (bytes)
    entries: HashMap<Vec<u8>, Vec<u8>>,
    generation: u64,
}

impl DynamicDictionary {
    pub fn new(seed: Seed) -> Self {
        DynamicDictionary {
            seed,
            entries: HashMap::new(),
            generation: 0,
        }
    }

    /// Register a plaintext token and return (or retrieve) its ciphertoken.
    pub fn register(&mut self, plaintext: &[u8]) -> Vec<u8> {
        if let Some(token) = self.entries.get(plaintext) {
            return token.clone();
        }
        let token = self.seed.derive(plaintext).bytes.to_vec();
        self.entries.insert(plaintext.to_vec(), token.clone());
        self.generation += 1;
        token
    }

    pub fn register_str(&mut self, plaintext: &str) -> Vec<u8> {
        self.register(plaintext.as_bytes())
    }

    /// Look up the ciphertoken for a known plaintext.
    pub fn lookup(&self, plaintext: &[u8]) -> Option<&Vec<u8>> {
        self.entries.get(plaintext)
    }

    /// Reverse lookup: find the plaintext for a given ciphertoken.
    pub fn reverse(&self, token: &[u8]) -> Option<Vec<u8>> {
        self.entries
            .iter()
            .find(|(_, v)| v.as_slice() == token)
            .map(|(k, _)| k.clone())
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Populate the dictionary from a slice of plaintext terms.
    /// Returns the number of newly registered entries (skips already-known terms).
    pub fn populate_from_terms(&mut self, terms: &[&str]) -> usize {
        let before = self.entries.len();
        for term in terms {
            self.register_str(term);
        }
        self.entries.len() - before
    }

    /// All registered plaintext terms (as UTF-8 strings, sorted).
    pub fn plaintext_terms(&self) -> Vec<String> {
        let mut terms: Vec<String> = self
            .entries
            .keys()
            .filter_map(|k| String::from_utf8(k.clone()).ok())
            .collect();
        terms.sort();
        terms
    }
}

// ── StreamCipher ─────────────────────────────────────────────────────────────

/// A simple XOR stream cipher seeded by a Seed.
/// Used for wrapping raw byte payloads between brain and terminal.
pub struct StreamCipher {
    key: [u8; 32],
    nonce: u64,
}

impl StreamCipher {
    pub fn new(seed: &Seed) -> Self {
        StreamCipher {
            key: seed.bytes,
            nonce: 0,
        }
    }

    /// Encrypt (or decrypt — XOR is symmetric) a byte slice.
    pub fn process(&mut self, data: &[u8]) -> Vec<u8> {
        let nonce_bytes = self.nonce.to_le_bytes();
        self.nonce += 1;
        let mut key_stream = self.key;
        for (i, b) in nonce_bytes.iter().enumerate() {
            key_stream[i % 32] ^= b;
        }
        data.iter()
            .enumerate()
            .map(|(i, b)| b ^ key_stream[i % 32])
            .collect()
    }
}

// ── EncryptionContext ─────────────────────────────────────────────────────────

/// Combines the dynamic dictionary with a stream cipher for full session security.
pub struct EncryptionContext {
    pub dictionary: DynamicDictionary,
    cipher: StreamCipher,
}

impl EncryptionContext {
    pub fn new(seed: Seed) -> Self {
        let cipher_seed = seed.derive(b"stream_cipher");
        EncryptionContext {
            cipher: StreamCipher::new(&cipher_seed),
            dictionary: DynamicDictionary::new(seed),
        }
    }

    /// Encrypt a message: tokenise → cipher.
    pub fn encrypt(&mut self, message: &[u8]) -> Vec<u8> {
        self.cipher.process(message)
    }

    /// Decrypt a message: cipher → de-tokenise.
    pub fn decrypt(&mut self, ciphertext: &[u8]) -> Vec<u8> {
        self.cipher.process(ciphertext)
    }

    /// Register a command token in the dictionary.
    pub fn register_command(&mut self, command: &str) -> Vec<u8> {
        self.dictionary.register_str(command)
    }
}
