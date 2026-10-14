// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! V1 → V2 data migration pipeline.
//!
//! Reads a v1 brain file (JSON or bincode), converts nodes and bonds into v2
//! typed KUs, and ingests them into an existing v2 Brain.
//!
//! ## Migration stages
//!
//! 1. **Node import** — read each active v1 node's `label` as raw text.
//! 2. **Heuristic typing** — run the v2 temporal lobe's extraction pipeline on
//!    that text; upgrade any sentence that matches a typed pattern
//!    (Definition / Mechanism / etc.) to the appropriate KU.  Sentences that
//!    don't match remain as Raw KUs.
//! 3. **Bond migration** — map v1 bond labels ("reindex", "auto-association",
//!    …) to the closest v2 BondType; default to Context for unknown labels.
//! 4. **Re-index** — rebuild the keyword inverted index for all migrated nodes.

use graph::bond::BondType;

// ── MigrationStats ────────────────────────────────────────────────────────────

/// Statistics collected during a V1 → V2 migration run.
#[derive(Debug, Clone, Default)]
pub struct MigrationStats {
    /// Total active v1 nodes processed.
    pub total_v1_nodes: usize,
    /// Nodes that were successfully re-typed to a non-Raw KU.
    pub successfully_typed: usize,
    /// Nodes that remained as Raw KU (pattern didn't match any extractor).
    pub remained_raw: usize,
    /// V1 bonds successfully mapped to v2 TypedBonds.
    pub bonds_migrated: usize,
    /// Bonds where source or target could not be found (skipped).
    pub bonds_skipped: usize,
}

impl MigrationStats {
    /// True if at least one node was migrated.
    pub fn any_migrated(&self) -> bool {
        self.total_v1_nodes > 0
    }

    /// Fraction of nodes that were typed (0.0–1.0).
    pub fn typing_rate(&self) -> f32 {
        if self.total_v1_nodes == 0 {
            0.0
        } else {
            self.successfully_typed as f32 / self.total_v1_nodes as f32
        }
    }
}

// ── Bond label mapping ────────────────────────────────────────────────────────

/// Map a v1 bond label to the closest v2 BondType.
///
/// The label is first stripped of any colon-qualifier (e.g. `"reindex:avoid"`
/// → `"reindex"`) before matching.  V1 label taxonomy:
///   - "reindex"          → related topic cluster  → Context
///   - "auto-association" → co-activation cluster  → Context
///   - "self-association" → self-reference loop     → Context
///   - "self-derived"     → local inference         → DerivesFrom
///   - "rumination"       → reflective trace        → Context
///   - "user-defined"     → explicit user link      → Context
pub fn map_v1_bond_label(label: &str) -> BondType {
    match v1_label_prefix(label).to_lowercase().as_str() {
        "self-derived" | "derived"            => BondType::DerivesFrom,
        "causes" | "cause"                    => BondType::Causes,
        "requires" | "requires-input"         => BondType::Requires,
        "is-a" | "isa" | "type-of"           => BondType::IsA,
        "has-part" | "haspart" | "part-of"   => BondType::HasPart,
        "example-of" | "exampleof"            => BondType::ExampleOf,
        "sequence" | "next" | "then"          => BondType::Sequence,
        "implements"                           => BondType::Implements,
        "opposes" | "contradicts" | "contrast" => BondType::Opposes,
        // All reindex / association / rumination variants collapse to Context
        _ => BondType::Context,
    }
}

// ── Seed hex extraction helpers ───────────────────────────────────────────────

/// Try to extract a node seed hex string from a v1 bond JSON value.
///
/// V1 bonds store `source` and `target` as `{ "bytes": [u8; 32] }` objects.
/// The bond's own key in the map is already the hex of `seed`, but source/target
/// must be reconstructed from the bytes array.
///
/// Fallback chain (for older serialisation variants):
///   1. `{ "source": { "bytes": [...] } }` → hex-encode the bytes array  ← primary
///   2. `{ "source": "deadbeef..." }`       → plain hex string
///   3. `{ "source_seed": { "bytes": [...] } }` → hex-encode bytes
///   4. `{ "source_seed": "deadbeef..." }`   → plain hex string
///   5. `{ "source_seed": { "hex": "..." } }` → nested hex field
pub fn extract_seed_hex_from_bond(
    bond_val: &serde_json::Value,
    which: &str,
) -> Option<String> {
    // ── Primary: { "source": { "bytes": [u8; 32] } } ─────────────────────────
    if let Some(obj) = bond_val.get(which) {
        if let Some(arr) = obj.get("bytes").and_then(|v| v.as_array()) {
            let bytes: Option<Vec<u8>> = arr.iter()
                .map(|b| b.as_u64().map(|n| n as u8))
                .collect();
            if let Some(b) = bytes {
                if b.len() == 32 {
                    return Some(b.iter().map(|x| format!("{:02x}", x)).collect());
                }
            }
        }
        // Fallback: plain hex string in the field itself
        if let Some(s) = obj.as_str() {
            return Some(s.to_string());
        }
    }
    // ── Legacy: { "source_seed": { "bytes": [...] } } ────────────────────────
    let key = format!("{}_seed", which);
    if let Some(obj) = bond_val.get(&key) {
        if let Some(arr) = obj.get("bytes").and_then(|v| v.as_array()) {
            let bytes: Option<Vec<u8>> = arr.iter()
                .map(|b| b.as_u64().map(|n| n as u8))
                .collect();
            if let Some(b) = bytes {
                if b.len() == 32 {
                    return Some(b.iter().map(|x| format!("{:02x}", x)).collect());
                }
            }
        }
        if let Some(s) = obj.as_str() {
            return Some(s.to_string());
        }
        if let Some(hex) = obj.get("hex").and_then(|v| v.as_str()) {
            return Some(hex.to_string());
        }
    }
    None
}

/// Extract the bond-type prefix from a v1 label.
///
/// V1 labels are either plain (`"auto-association"`) or colon-qualified
/// (`"reindex:avoid"`, `"reindex:fetch"`).  Only the prefix before the first
/// colon carries semantic meaning for migration purposes.
pub fn v1_label_prefix(label: &str) -> &str {
    label.split(':').next().unwrap_or(label).trim()
}
