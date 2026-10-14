// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Bond Label Connector Map
//!
//! Translates bond relationship labels into natural language connectors.
//! Lu's bonds have operational labels ("reindex:water", "auto-association",
//! "self-association: co-activated during recall", etc.) rather than
//! semantic labels ("causes", "is-a"). This module bridges that gap.
//!
//! No NLP library. No LLM. Just a map and some string logic.

use std::collections::HashMap;

/// All known bond label categories and their English connectors.
/// Multiple options per category allow Lu to vary speech.
pub struct ConnectorMap {
    /// category_key -> list of connector phrases
    connectors: HashMap<&'static str, Vec<&'static str>>,
    /// Track recently used connectors to avoid repetition
    recent: Vec<String>,
    /// Max recent history to track
    max_recent: usize,
}

impl ConnectorMap {
    pub fn new() -> Self {
        let mut connectors = HashMap::new();

        // Reindex bonds — nodes share a common word; topically related
        connectors.insert("reindex", vec![
            "Related to this,",
            "Similarly,",
            "Along the same lines,",
            "On that topic,",
        ]);

        // Auto-association — batch training linked semantically similar nodes
        connectors.insert("auto-association", vec![
            "Relatedly,",
            "In the same area,",
            "Closely connected,",
            "On a related note,",
        ]);

        // Self-association — co-activated during recall; contextually linked
        connectors.insert("self-association", vec![
            "This connects to",
            "Worth noting,",
            "In context,",
        ]);

        // Self-derived — brain inferred this connection; implication/consequence
        connectors.insert("self-derived", vec![
            "This means",
            "As a result,",
            "From this,",
            "By extension,",
        ]);

        // Rumination — discovered during idle thinking; reflective insight
        connectors.insert("rumination", vec![
            "Thinking about this further,",
            "It's also worth noting that",
            "On reflection,",
        ]);

        // User-defined associations — explicit links the user created.
        // Note: connector_for_bond() in narration.rs returns "" for user-defined,
        // so these are only used if get_connector() is called directly.
        connectors.insert("user-defined", vec![
            "Also,",
        ]);

        // Fallback — when we can't classify the bond.
        // connector_for_bond() returns "" for unknown bonds too.
        connectors.insert("unknown", vec![
            "Also,",
        ]);

        Self {
            connectors,
            recent: Vec::new(),
            max_recent: 4,
        }
    }

    /// Classify a bond label into a connector category.
    /// Bond labels look like:
    ///   "reindex:water"
    ///   "auto-association"
    ///   "self-association: co-activated during recall"
    ///   "self-derived"
    ///   "rumination: co-activated without direct bond"
    ///   or any user-defined string
    pub fn classify_label(label: &str) -> &'static str {
        let lower = label.to_lowercase();
        if lower.starts_with("reindex:") || lower.starts_with("reindex_") {
            "reindex"
        } else if lower.contains("auto-association") || lower.contains("auto_association") {
            "auto-association"
        } else if lower.contains("self-association") || lower.contains("co-activated during recall") {
            "self-association"
        } else if lower.contains("self-derived") || lower.contains("self_derived") {
            "self-derived"
        } else if lower.contains("rumination") {
            "rumination"
        } else {
            // Could be user-defined or unknown
            "user-defined"
        }
    }

    /// Get a connector phrase for the given bond label.
    /// Avoids recently used connectors to prevent repetition.
    pub fn get_connector(&mut self, bond_label: &str) -> &'static str {
        let category = Self::classify_label(bond_label);
        let options = self.connectors.get(category)
            .or_else(|| self.connectors.get("unknown"))
            .expect("unknown category must exist");

        // Find first option not in recent history
        let choice = options.iter()
            .find(|&&opt| !self.recent.iter().any(|r| r == opt))
            .copied()
            .unwrap_or(options[0]); // fallback to first if all are recent

        // Track this choice
        self.recent.push(choice.to_string());
        if self.recent.len() > self.max_recent {
            self.recent.remove(0);
        }

        choice
    }

    /// Reset the recent history (e.g., between responses)
    pub fn reset_recent(&mut self) {
        self.recent.clear();
    }
}
