// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Brainstem — persistence, auto-save, crash recovery, graph versioning.
//!
//! File format:
//!   magic[4]  = b"LUBR"
//!   format_v  = 2  (bumped when serialization layout changes)
//!   schema_v  = 1  (bumped when KU / bond type set changes)
//!   ... bincode payload ...
//!
//! The brainstem also manages version tags so you can checkpoint the graph
//! and roll back to a named state.

use chrono::{DateTime, Utc};
use graph::network::AssociativeNetwork;
use serde::{Deserialize, Serialize};

use crate::curiosity::WonderItem;
use crate::feature_flags::FeatureFlags;
use crate::self_improvement::{HotPathIndex, QueryPatternIndex, RecallQualityLog};
use crate::session_memory::{SessionBelief, SessionLearning};

// ── Magic bytes and format versioning ─────────────────────────────────────────

pub const MAGIC: &[u8; 4] = b"LUBR";
/// Bumped to 3 for Phase 5: switched serialisation from bincode → JSON
/// and added full hippocampus/amygdala/tier persistence.
pub const FORMAT_VERSION: u32 = 3;
pub const SCHEMA_VERSION: u32 = 1;

// ── BrainFileHeader ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainFileHeader {
    pub magic: [u8; 4],
    pub format_version: u32,
    pub schema_version: u32,
    pub created_at: DateTime<Utc>,
    pub last_saved: DateTime<Utc>,
    pub node_count: u64,
    pub bond_count: u64,
    /// SHA-256 of the bincode payload that follows the header.
    pub checksum: [u8; 32],
}

// ── VersionTag ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionTag {
    pub tag: String,
    pub timestamp: DateTime<Utc>,
    pub node_count: u64,
    pub bond_count: u64,
    pub description: Option<String>,
}

// ── BrainFile ─────────────────────────────────────────────────────────────────

/// The complete serialized brain state.
#[derive(Debug, Serialize, Deserialize)]
pub struct BrainFile {
    pub header: BrainFileHeader,
    /// The full knowledge graph.
    pub network: AssociativeNetwork,
    /// Hippocampus snapshot — episodes, threads, style as JSON.
    pub hippocampus_snapshot: serde_json::Value,
    /// Amygdala scar list snapshot.
    pub amygdala_snapshot: serde_json::Value,
    /// Ordered history of version tags applied to this file.
    pub version_history: Vec<VersionTag>,
    /// Serialised autonomy tier label for restoration on load.
    #[serde(default)]
    pub autonomy_tier_label: Option<String>,
    /// Persisted wonder items from the curiosity engine.
    #[serde(default)]
    pub wonder_items: Vec<WonderItem>,
    /// Persisted performance metrics (total_queries, hit_rate, etc.).
    #[serde(default)]
    pub performance_metrics: Option<crate::self_improvement::PerformanceMetrics>,
    /// Rolling recall quality log — absent in older brain files (defaults to empty).
    #[serde(default)]
    pub quality_log: RecallQualityLog,
    /// Query-pattern quality index — absent in older brain files.
    #[serde(default)]
    pub pattern_index: QueryPatternIndex,
    /// Hot traversal-path index — absent in older brain files.
    #[serde(default)]
    pub hot_path_index: HotPathIndex,
    /// Per-node quality history: seed hex → (low_count, high_count).
    #[serde(default)]
    pub node_quality_history: std::collections::HashMap<String, (u32, u32)>,
    /// Phase 3: serialised RecallParameters for the self-tuning controller.
    /// Stored as a JSON blob so older brain files load cleanly (field absent
    /// → controller starts with defaults).
    #[serde(default)]
    pub recall_params_snapshot: serde_json::Value,
    /// Session-scoped learnings — persisted across saves so long-running
    /// sessions survive a reload.  Absent in older brain files (defaults empty).
    #[serde(default)]
    pub session_learnings: Vec<SessionLearning>,
    /// Runtime feature flag state — persisted so flags survive a reload.
    /// Absent in older brain files (defaults to FeatureFlags::default()).
    #[serde(default)]
    pub feature_flags: FeatureFlags,
    /// Session beliefs — timed assertions about recent topics.  Persisted so
    /// belief state survives a mid-session brain reload.
    #[serde(default)]
    pub session_beliefs: Vec<SessionBelief>,
    /// Hex strings of the top-N recall nodes from the last turn before save.
    /// Restored into SessionState.topic_seeds so the first post-reload turn
    /// can continue spreading activation from the prior conversation context.
    #[serde(default)]
    pub session_topic_seeds: Vec<String>,
    /// The last ≤10 raw query strings from the session, for keyword seeding.
    #[serde(default)]
    pub session_recent_queries: Vec<String>,
    /// Lifetime wonder-item generation counter — absent in older brain files.
    #[serde(default)]
    pub wonder_total_generated: u64,
    /// Lifetime wonder-item resolution counter — absent in older brain files.
    #[serde(default)]
    pub wonder_total_resolved: u64,
    /// Topics with consistently weak recall — fed into gap-detection strategies.
    /// Absent in older brain files (defaults to empty).
    #[serde(default)]
    pub weak_query_topics: Vec<String>,
}

// ── Brainstem ─────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct Brainstem {
    pub storage_path: Option<std::path::PathBuf>,
    pub backup_dir: Option<std::path::PathBuf>,
    pub auto_save_interval_secs: u64,
    pub last_saved: Option<DateTime<Utc>>,
    pub save_count: u64,
    pub pending_recovery: bool,
    pub version_tags: Vec<VersionTag>,
}

impl Brainstem {
    pub fn new() -> Self {
        Brainstem {
            auto_save_interval_secs: 300,
            ..Default::default()
        }
    }

    pub fn with_path(mut self, path: std::path::PathBuf) -> Self {
        self.storage_path = Some(path);
        self
    }

    /// True if an auto-save is due.
    pub fn auto_save_due(&self) -> bool {
        if self.auto_save_interval_secs == 0 {
            return false;
        }
        match self.last_saved {
            None => true,
            Some(t) => {
                let elapsed = Utc::now().signed_duration_since(t).num_seconds();
                elapsed >= self.auto_save_interval_secs as i64
            }
        }
    }

    pub fn record_save(&mut self, node_count: u64, bond_count: u64) {
        self.last_saved = Some(Utc::now());
        self.save_count += 1;
        // Auto-create a version tag every 10 saves
        if self.save_count % 10 == 0 {
            self.version_tags.push(VersionTag {
                tag: format!("auto-{}", self.save_count),
                timestamp: Utc::now(),
                node_count,
                bond_count,
                description: Some("automatic checkpoint".to_string()),
            });
        }
    }

    pub fn create_version_tag(
        &mut self,
        tag: impl Into<String>,
        node_count: u64,
        bond_count: u64,
        description: Option<String>,
    ) {
        self.version_tags.push(VersionTag {
            tag: tag.into(),
            timestamp: Utc::now(),
            node_count,
            bond_count,
            description,
        });
    }

    /// Build a backup path for the current storage file.
    pub fn backup_path(&self) -> Option<std::path::PathBuf> {
        let src = self.storage_path.as_ref()?;
        let dir = self.backup_dir.clone().unwrap_or_else(|| {
            src.parent().unwrap_or(std::path::Path::new(".")).to_path_buf()
        });
        let stem = src.file_stem()?.to_string_lossy();
        let ts = Utc::now().format("%Y%m%dT%H%M%S");
        Some(dir.join(format!("{}_{}.lbr.bak", stem, ts)))
    }
}
