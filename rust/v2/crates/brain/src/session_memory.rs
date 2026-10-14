// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Session-scoped left-brain memory — tasks, beliefs, and learnings.
//!
//! Unlike the hippocampus (which manages episodic memories and consolidation),
//! session memory is strictly per-conversation and ephemeral.  It tracks:
//!   - Tasks: goals the user or brain is pursuing this session
//!   - Beliefs: short-lived confident assertions (with TTL expiry)
//!   - Learnings: insights derived from completed tasks

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── TaskStatus ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

impl TaskStatus {
    pub fn label(&self) -> &'static str {
        match self {
            TaskStatus::Pending    => "pending",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::Completed  => "completed",
            TaskStatus::Failed     => "failed",
        }
    }
}

// ── SessionAttempt ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionAttempt {
    pub method: String,
    pub outcome: String,
    pub timestamp: DateTime<Utc>,
}

impl SessionAttempt {
    pub fn new(method: impl Into<String>, outcome: impl Into<String>) -> Self {
        SessionAttempt {
            method: method.into(),
            outcome: outcome.into(),
            timestamp: Utc::now(),
        }
    }
}

// ── SessionTask ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTask {
    pub id: String,
    pub description: String,
    pub status: TaskStatus,
    pub attempts: Vec<SessionAttempt>,
    pub created_at: DateTime<Utc>,
}

impl SessionTask {
    pub fn new(description: impl Into<String>) -> Self {
        SessionTask {
            id: uuid::Uuid::new_v4().to_string(),
            description: description.into(),
            status: TaskStatus::Pending,
            attempts: Vec::new(),
            created_at: Utc::now(),
        }
    }

    pub fn record_attempt(&mut self, method: impl Into<String>, outcome: impl Into<String>) {
        self.attempts.push(SessionAttempt::new(method, outcome));
        self.status = TaskStatus::InProgress;
    }

    pub fn complete(&mut self) {
        self.status = TaskStatus::Completed;
    }

    pub fn fail(&mut self) {
        self.status = TaskStatus::Failed;
    }
}

// ── SessionBelief ─────────────────────────────────────────────────────────────

/// A short-lived confident assertion that expires after `ttl_seconds`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionBelief {
    pub id: String,
    pub statement: String,
    /// 0.0–1.0 confidence level.
    pub confidence: f32,
    /// How long this belief should be held (seconds).
    pub ttl_seconds: u64,
    pub created_at: DateTime<Utc>,
}

impl SessionBelief {
    pub fn new(statement: impl Into<String>, confidence: f32, ttl_seconds: u64) -> Self {
        SessionBelief {
            id: uuid::Uuid::new_v4().to_string(),
            statement: statement.into(),
            confidence: confidence.clamp(0.0, 1.0),
            ttl_seconds,
            created_at: Utc::now(),
        }
    }

    pub fn is_expired(&self) -> bool {
        let age = (Utc::now() - self.created_at).num_seconds();
        age < 0 || age as u64 >= self.ttl_seconds
    }

    /// Remaining lifetime in seconds (0 if expired).
    pub fn remaining_secs(&self) -> u64 {
        let age = (Utc::now() - self.created_at).num_seconds().max(0) as u64;
        self.ttl_seconds.saturating_sub(age)
    }
}

// ── SessionLearning ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLearning {
    pub id: String,
    pub insight: String,
    pub source_task_id: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl SessionLearning {
    pub fn new(insight: impl Into<String>, source_task_id: Option<String>) -> Self {
        SessionLearning {
            id: uuid::Uuid::new_v4().to_string(),
            insight: insight.into(),
            source_task_id,
            timestamp: Utc::now(),
        }
    }
}

// ── SessionState ──────────────────────────────────────────────────────────────

/// Complete per-conversation session state.
///
/// # Left-Brain / Right-Brain wall
/// All fields here are **operational/working memory only**.  Data in this struct
/// can *inform* recall scoring but CANNOT auto-promote to the Right Brain graph
/// without explicit human approval.  This is the anti-hallucination guarantee.
#[derive(Debug, Default)]
pub struct SessionState {
    pub tasks: Vec<SessionTask>,
    pub beliefs: Vec<SessionBelief>,
    pub learnings: Vec<SessionLearning>,
    /// Rolling window of the last 10 raw query strings from this session.
    /// Serialised so the window survives a brain reload mid-session.
    pub recent_queries: Vec<String>,
    /// Hex strings of the top-recall node seeds from the *previous* turn.
    /// Carried forward to seed spreading activation on the next turn,
    /// maintaining conversational continuity across turns and reloads.
    pub topic_seeds: Vec<String>,
}

impl SessionState {
    pub fn new() -> Self {
        SessionState::default()
    }

    /// Push `query` onto the rolling recent-query window (capped at 10).
    pub fn push_query(&mut self, query: &str) {
        self.recent_queries.push(query.to_string());
        if self.recent_queries.len() > 10 {
            self.recent_queries.remove(0);
        }
    }

    /// Replace the stored topic seeds with a fresh set of hex strings.
    pub fn set_topic_seeds(&mut self, hexes: Vec<String>) {
        self.topic_seeds = hexes;
    }

    /// Add a new task and return its ID.
    pub fn add_task(&mut self, description: &str) -> String {
        let task = SessionTask::new(description);
        let id = task.id.clone();
        self.tasks.push(task);
        id
    }

    /// Record an attempt on an existing task.  Returns false if task not found.
    pub fn record_attempt(&mut self, task_id: &str, method: &str, outcome: &str) -> bool {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) {
            task.record_attempt(method, outcome);
            true
        } else {
            false
        }
    }

    pub fn complete_task(&mut self, task_id: &str) -> bool {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) {
            task.complete();
            true
        } else {
            false
        }
    }

    /// Add a timed belief.
    pub fn add_belief(&mut self, statement: &str, confidence: f32, ttl_seconds: u64) -> String {
        let belief = SessionBelief::new(statement, confidence, ttl_seconds);
        let id = belief.id.clone();
        self.beliefs.push(belief);
        id
    }

    /// Remove beliefs whose TTL has elapsed.  Returns how many were pruned.
    pub fn prune_expired_beliefs(&mut self) -> usize {
        let before = self.beliefs.len();
        self.beliefs.retain(|b| !b.is_expired());
        before - self.beliefs.len()
    }

    /// Currently active (non-expired) beliefs.
    pub fn active_beliefs(&self) -> Vec<&SessionBelief> {
        self.beliefs.iter().filter(|b| !b.is_expired()).collect()
    }

    /// Active beliefs whose statement contains at least one 4+ character word
    /// from `query`.  Used to surface relevant context before narration.
    pub fn relevant_beliefs(&self, query: &str) -> Vec<&SessionBelief> {
        let query_words: Vec<String> = query
            .split_whitespace()
            .filter(|w| w.len() >= 4)
            .map(|w| w.to_lowercase())
            .collect();
        if query_words.is_empty() {
            return vec![];
        }
        self.beliefs
            .iter()
            .filter(|b| {
                if b.is_expired() {
                    return false;
                }
                let stmt_lower = b.statement.to_lowercase();
                query_words.iter().any(|w| stmt_lower.contains(w.as_str()))
            })
            .collect()
    }

    /// Add a session-scoped learning.
    pub fn add_learning(&mut self, insight: &str, source_task_id: Option<String>) -> String {
        let learning = SessionLearning::new(insight, source_task_id);
        let id = learning.id.clone();
        self.learnings.push(learning);
        id
    }

    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    pub fn active_belief_count(&self) -> usize {
        self.beliefs.iter().filter(|b| !b.is_expired()).count()
    }

    pub fn learning_count(&self) -> usize {
        self.learnings.len()
    }
}
