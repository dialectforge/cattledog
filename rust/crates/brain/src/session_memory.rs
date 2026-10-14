// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Left Brain: Session Memory (v2)
//!
//! The operational / working-memory layer of Lu AI, modeled after the project log.
//!
//! ## Architecture
//!
//! The brain is split into two hemispheres:
//!
//! - **Right Brain** (`graph` crate): permanent factual knowledge.
//!   Nodes here are TRUTH — verified, bonded, persistent across sessions.
//!
//! - **Left Brain** (this module): operational session memory.
//!   Structured like the project log: tasks, attempts, beliefs, learnings, logs.
//!   This is what Lu *is doing*, *has tried*, *thinks right now*, and
//!   *has learned this session* — not what she *knows* to be true.
//!
//! ## The Wall
//!
//! Session data can **inform** recall and reasoning — results are tagged
//! `[SESSION]` so callers always know the source. But session data can
//! **never** auto-promote to Right Brain fact nodes. Promotion requires
//! explicit human approval via the `propose → approve` pipeline.
//! This is the anti-hallucination guarantee.
//!
//! ## Structure (the project log-inspired)
//!
//! - **SessionTask**: What Lu is working on (like the project log components/todos).
//!   Has subtasks, status, and links to the thinking layer's goals.
//!
//! - **SessionAttempt**: What Lu has tried (like the project log attempts).
//!   Records approach, outcome, and why it succeeded/failed.
//!
//! - **SessionBelief**: What Lu thinks right now (working hypotheses).
//!   Temporary beliefs with confidence, pattern vectors, TTL.
//!   These are the old SessionFacts, renamed for clarity.
//!
//! - **SessionLearning**: What Lu learned this session (like the project log learnings).
//!   Distilled insights that become promotion candidates — the only path
//!   from Left Brain to Right Brain, and only through human approval.
//!
//! - **SessionLog**: Chronological event stream (like the project log changes).
//!   Everything that happened, in order, for debugging and audit.
//!
//! ## Lifecycle
//!
//! Created fresh at Brain::new(). Not persisted to disk.
//! Each session starts with a clean slate. Beliefs have TTL and decay.
//! Learnings persist until session end, then are lost unless promoted.

use df_core::Seed;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// Default time-to-live for a session belief (seconds). 1 hour.
pub const DEFAULT_BELIEF_TTL_SECS: u64 = 3600;

/// A topic seed carried forward from a prior turn's recall results.
/// These are EXACT node references (not keyword matches) with their
/// activation scores, used to seed spreading activation in follow-up turns.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TopicSeed {
    pub seed_hex: String,
    pub score: f32,
}

/// How many times a belief must surface in recall before it becomes
/// a learning candidate for human review.
pub const LEARNING_THRESHOLD: u32 = 3;

// ━━━ ID Generator ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn short_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:08x}", (nanos & 0xFFFFFFFF) as u32)
}

// ━━━ SessionTask ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Like the project log components/todos: what Lu is working on right now.

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Created but not started
    Pending,
    /// Currently being worked on
    Active,
    /// Paused — waiting for input or approval
    Blocked,
    /// Successfully completed
    Complete,
    /// Abandoned or failed
    Failed,
}

/// A task Lu is working on during this session.
/// Maps to the project log components — structured work tracking.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionTask {
    /// Unique ID within this session
    pub id: String,
    /// What this task is about
    pub description: String,
    /// Current status
    pub status: TaskStatus,
    /// Parent task ID (for subtask hierarchy, like the project log parent_component_id)
    pub parent_id: Option<String>,
    /// Link to thinking layer goal seed (if this task came from /think)
    pub goal_seed: Option<String>,
    /// When this task was created
    pub created_at: DateTime<Utc>,
    /// When this task last changed status
    pub updated_at: DateTime<Utc>,
}

// ━━━ SessionAttempt ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Like the project log attempts: what Lu tried and what happened.

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AttemptOutcome {
    /// Still in progress
    InProgress,
    /// Approach worked
    Success,
    /// Approach partially worked
    Partial,
    /// Approach failed
    Failed,
    /// Approach was blocked (e.g. scar conflict)
    Blocked,
}

/// A record of something Lu tried during a task.
/// Maps to the project log attempts — preserves reasoning history.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionAttempt {
    /// Unique ID
    pub id: String,
    /// Which task this attempt belongs to
    pub task_id: String,
    /// What approach was tried
    pub approach: String,
    /// What happened
    pub outcome: AttemptOutcome,
    /// Why it succeeded/failed (populated after completion)
    pub reason: Option<String>,
    /// When this attempt started
    pub started_at: DateTime<Utc>,
    /// When this attempt completed (None if still in progress)
    pub completed_at: Option<DateTime<Utc>>,
}

// ━━━ SessionBelief ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Working hypotheses. The renamed/evolved SessionFact from v1.

/// A temporary belief formed during a session.
/// These are NOT permanent knowledge — they carry TTL and decay.
/// Promotion to Right Brain requires explicit human approval.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionBelief {
    /// Short identifier (derived seed, first 8 bytes hex)
    pub id: String,
    /// The content of the belief
    pub content: String,
    /// Where this came from (e.g. "user_statement", "inference", "observation")
    pub source: String,
    /// Which task this belief relates to (None = general session context)
    pub task_id: Option<String>,
    /// Confidence (0.0–1.0). NOT graph Confidence.
    pub confidence: f32,
    /// 256-dim pattern vector for similarity retrieval
    pub pattern: Vec<f32>,
    /// When created
    pub created_at: DateTime<Utc>,
    /// TTL in seconds before expiry
    pub ttl_secs: u64,
    /// True only after human approved promotion
    pub promoted: bool,
    /// How many times this surfaced in recall
    pub recall_count: u32,
}

impl SessionBelief {
    pub fn is_expired(&self) -> bool {
        let elapsed = Utc::now()
            .signed_duration_since(self.created_at)
            .num_seconds();
        elapsed as u64 >= self.ttl_secs
    }

    pub fn is_learning_candidate(&self) -> bool {
        !self.promoted && self.recall_count >= LEARNING_THRESHOLD
    }
}

// ━━━ SessionLearning ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Like the project log learnings: distilled insights from the session.
// These are the ONLY things that can cross The Wall (with approval).

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LearningCategory {
    /// Factual discovery about the world
    Fact,
    /// Pattern or principle Lu observed
    Pattern,
    /// Something that worked well (best practice)
    BestPractice,
    /// Something to avoid (gotcha / anti-pattern)
    Gotcha,
    /// Architectural or structural insight
    Architecture,
}

/// A distilled insight from the session.
/// Learnings are created when beliefs surface repeatedly, or when
/// Lu explicitly distills an attempt outcome into a reusable insight.
/// They are promotion candidates — the only bridge across The Wall.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionLearning {
    /// Unique ID
    pub id: String,
    /// The distilled insight
    pub insight: String,
    /// Category of learning
    pub category: LearningCategory,
    /// What context produced this learning
    pub context: String,
    /// Source belief IDs or attempt IDs that led to this
    pub derived_from: Vec<String>,
    /// Has this been promoted to Right Brain? (only via human approval)
    pub promoted: bool,
    /// When this learning was distilled
    pub created_at: DateTime<Utc>,
}

// ━━━ SessionLog ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Like the project log changes: chronological event stream.

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LogEventType {
    TaskCreated,
    TaskStatusChanged,
    AttemptStarted,
    AttemptCompleted,
    BeliefAdded,
    BeliefDecayed,
    BeliefPromoted,
    LearningDistilled,
    LearningPromoted,
    ContextAdded,
    GoalLinked,
    SessionCleared,
}

/// A single event in the session timeline.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionLogEntry {
    /// Sequential event number
    pub seq: u64,
    /// What kind of event
    pub event_type: LogEventType,
    /// Human-readable description
    pub description: String,
    /// Related entity ID (task, attempt, belief, or learning)
    pub entity_id: Option<String>,
    /// When this happened
    pub timestamp: DateTime<Utc>,
}

// ━━━ SessionContext ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Rolling conversation context (carried over from v1).

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionContext {
    pub recent_queries: Vec<String>,
    pub active_goal: Option<String>,
    max_queries: usize,
}

impl Default for SessionContext {
    fn default() -> Self {
        Self {
            recent_queries: Vec::new(),
            active_goal: None,
            max_queries: 10,
        }
    }
}

impl SessionContext {
    pub fn add_query(&mut self, query: &str) {
        if !query.trim().is_empty() {
            self.recent_queries.push(query.to_string());
            if self.recent_queries.len() > self.max_queries {
                self.recent_queries.remove(0);
            }
        }
    }
}

// ━━━ SessionMemory (The Left Brain) ━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The Left Brain: the project log-structured operational session memory.
/// Created fresh per session. Not persisted to disk.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionMemory {
    /// Session identity seed (fresh per session)
    seed: Seed,
    /// Rolling conversation context
    pub context: SessionContext,
    /// Active tasks (like the project log components)
    pub tasks: HashMap<String, SessionTask>,
    /// Attempt history (like the project log attempts)
    pub attempts: Vec<SessionAttempt>,
    /// Working beliefs with pattern vectors
    pub beliefs: HashMap<String, SessionBelief>,
    /// Distilled learnings (promotion candidates)
    pub learnings: Vec<SessionLearning>,
    /// Chronological event log (like the project log changes)
    pub log: Vec<SessionLogEntry>,
    /// Next sequence number for log entries
    log_seq: u64,
    /// Topic seeds from the prior turn's recall results.
    /// These are the top activated nodes (exact references) that
    /// narration used, carried forward for conversation continuity.
    /// Transient — not persisted, fresh each session.
    #[serde(skip, default)]
    pub topic_seeds: Vec<TopicSeed>,
}

impl Default for SessionMemory {
    fn default() -> Self {
        Self {
            seed: Seed::generate(),
            context: SessionContext::default(),
            tasks: HashMap::new(),
            attempts: Vec::new(),
            beliefs: HashMap::new(),
            learnings: Vec::new(),
            log: Vec::new(),
            log_seq: 0,
            topic_seeds: Vec::new(),
        }
    }
}

impl SessionMemory {
    pub fn new() -> Self { Self::default() }

    // ── Logging ──────────────────────────────────────────────────

    fn log_event(&mut self, event_type: LogEventType, description: &str, entity_id: Option<&str>) {
        self.log_seq += 1;
        self.log.push(SessionLogEntry {
            seq: self.log_seq,
            event_type,
            description: description.to_string(),
            entity_id: entity_id.map(|s| s.to_string()),
            timestamp: Utc::now(),
        });
    }

    // ── Context ──────────────────────────────────────────────────

    pub fn add_context(&mut self, query: &str) {
        self.context.add_query(query);
        self.log_event(LogEventType::ContextAdded, query, None);
    }

    pub fn set_active_goal(&mut self, goal: Option<&str>) {
        self.context.active_goal = goal.map(|s| s.to_string());
    }

    // ── Tasks ────────────────────────────────────────────────────

    /// Create a new task. Returns the task ID.
    pub fn create_task(
        &mut self, description: &str, parent_id: Option<&str>,
    ) -> String {
        let id = short_id();
        let now = Utc::now();
        let task = SessionTask {
            id: id.clone(),
            description: description.to_string(),
            status: TaskStatus::Pending,
            parent_id: parent_id.map(|s| s.to_string()),
            goal_seed: None,
            created_at: now,
            updated_at: now,
        };
        self.tasks.insert(id.clone(), task);
        self.log_event(LogEventType::TaskCreated, description, Some(&id));
        id
    }

    /// Link a task to a thinking layer goal seed.
    pub fn link_task_to_goal(&mut self, task_id: &str, goal_seed: &str) {
        if let Some(task) = self.tasks.get_mut(task_id) {
            task.goal_seed = Some(goal_seed.to_string());
            self.log_event(
                LogEventType::GoalLinked,
                &format!("Task {} linked to goal {}", task_id, goal_seed),
                Some(task_id),
            );
        }
    }

    /// Update a task's status.
    pub fn update_task_status(&mut self, task_id: &str, status: TaskStatus) {
        if let Some(task) = self.tasks.get_mut(task_id) {
            let old = format!("{:?}", task.status);
            task.status = status.clone();
            task.updated_at = Utc::now();
            self.log_event(
                LogEventType::TaskStatusChanged,
                &format!("Task {} → {:?} (was {})", task_id, status, old),
                Some(task_id),
            );
        }
    }

    /// Get the currently active task (if any).
    pub fn active_task(&self) -> Option<&SessionTask> {
        self.tasks.values().find(|t| t.status == TaskStatus::Active)
    }

    /// Get subtasks of a given task.
    pub fn subtasks(&self, parent_id: &str) -> Vec<&SessionTask> {
        self.tasks.values()
            .filter(|t| t.parent_id.as_deref() == Some(parent_id))
            .collect()
    }

    // ── Attempts ─────────────────────────────────────────────────

    /// Start an attempt on a task. Returns the attempt ID.
    pub fn start_attempt(&mut self, task_id: &str, approach: &str) -> String {
        let id = short_id();
        let attempt = SessionAttempt {
            id: id.clone(),
            task_id: task_id.to_string(),
            approach: approach.to_string(),
            outcome: AttemptOutcome::InProgress,
            reason: None,
            started_at: Utc::now(),
            completed_at: None,
        };
        self.attempts.push(attempt);
        self.log_event(
            LogEventType::AttemptStarted,
            &format!("Trying: {}", approach),
            Some(&id),
        );
        id
    }

    /// Complete an attempt with an outcome and reason.
    pub fn complete_attempt(
        &mut self, attempt_id: &str, outcome: AttemptOutcome, reason: &str,
    ) {
        if let Some(attempt) = self.attempts.iter_mut().find(|a| a.id == attempt_id) {
            attempt.outcome = outcome.clone();
            attempt.reason = Some(reason.to_string());
            attempt.completed_at = Some(Utc::now());
            self.log_event(
                LogEventType::AttemptCompleted,
                &format!("{:?}: {}", outcome, reason),
                Some(attempt_id),
            );
        }
    }

    /// Get all attempts for a task, chronologically.
    pub fn task_attempts(&self, task_id: &str) -> Vec<&SessionAttempt> {
        self.attempts.iter().filter(|a| a.task_id == task_id).collect()
    }

    // ── Beliefs ──────────────────────────────────────────────────

    /// Add a working belief. Returns belief ID.
    /// `pattern` must be 256-dim normalized Vec<f32>.
    pub fn add_belief(
        &mut self,
        content: &str,
        source: &str,
        task_id: Option<&str>,
        confidence: f32,
        pattern: Vec<f32>,
    ) -> String {
        let fact_seed = self.seed.derive(content.as_bytes());
        let id = fact_seed.short();

        // Dedup: update existing belief rather than creating duplicate
        if let Some(existing) = self.beliefs.get_mut(&id) {
            existing.confidence = confidence.clamp(0.0, 1.0);
            existing.source = source.to_string();
            return id;
        }

        let belief = SessionBelief {
            id: id.clone(),
            content: content.to_string(),
            source: source.to_string(),
            task_id: task_id.map(|s| s.to_string()),
            confidence: confidence.clamp(0.0, 1.0),
            pattern,
            created_at: Utc::now(),
            ttl_secs: DEFAULT_BELIEF_TTL_SECS,
            promoted: false,
            recall_count: 0,
        };

        self.beliefs.insert(id.clone(), belief);
        self.log_event(LogEventType::BeliefAdded, content, Some(&id));
        id
    }

    /// Find beliefs relevant to a query pattern (cosine similarity > 0.70).
    /// Increments recall_count on matches. Returns up to `top_k` results.
    /// Results tagged `[SESSION]` by Brain::recall().
    pub fn get_relevant_beliefs(
        &mut self, pattern: &[f32], top_k: usize,
    ) -> Vec<(String, String, String, f32, f32)> {
        let mut matches: Vec<(String, String, String, f32, f32)> = self
            .beliefs.iter_mut()
            .filter(|(_, b)| !b.is_expired() && !b.promoted)
            .filter_map(|(id, belief)| {
                let sim = cosine_similarity(pattern, &belief.pattern);
                if sim > 0.70 {
                    belief.recall_count += 1;
                    Some((
                        id.clone(), belief.content.clone(),
                        belief.source.clone(), belief.confidence, sim,
                    ))
                } else { None }
            })
            .collect();

        matches.sort_by(|a, b| b.4.partial_cmp(&a.4).unwrap_or(std::cmp::Ordering::Equal));

        // Check for new learning candidates
        for (id, _, _, _, _) in &matches {
            if let Some(belief) = self.beliefs.get(id) {
                if belief.is_learning_candidate() {
                    self.auto_distill_learning(id);
                }
            }
        }

        matches.into_iter().take(top_k).collect()
    }

    /// Remove expired beliefs. Returns count removed.
    pub fn decay_old_beliefs(&mut self) -> usize {
        let before = self.beliefs.len();
        let expired: Vec<String> = self.beliefs.iter()
            .filter(|(_, b)| b.is_expired())
            .map(|(id, _)| id.clone())
            .collect();
        for id in &expired {
            self.log_event(LogEventType::BeliefDecayed, &format!("Belief {} expired", id), Some(id));
            self.beliefs.remove(id);
        }
        before - self.beliefs.len()
    }

    // ── Learnings ────────────────────────────────────────────────

    /// Auto-distill a learning from a belief that surfaced enough times.
    fn auto_distill_learning(&mut self, belief_id: &str) {
        // Don't create duplicate learnings for the same belief
        if self.learnings.iter().any(|l| l.derived_from.contains(&belief_id.to_string())) {
            return;
        }
        if let Some(belief) = self.beliefs.get(belief_id) {
            let learning = SessionLearning {
                id: short_id(),
                insight: belief.content.clone(),
                category: LearningCategory::Pattern,
                context: format!(
                    "Surfaced {} times in recall (source: {})",
                    belief.recall_count, belief.source
                ),
                derived_from: vec![belief_id.to_string()],
                promoted: false,
                created_at: Utc::now(),
            };
            let lid = learning.id.clone();
            self.learnings.push(learning);
            self.log_event(
                LogEventType::LearningDistilled,
                &format!("Auto-distilled from belief {}", belief_id),
                Some(&lid),
            );
        }
    }

    /// Manually distill a learning from an attempt outcome.
    /// This is how Lu explicitly extracts reusable insights.
    pub fn distill_learning(
        &mut self,
        insight: &str,
        category: LearningCategory,
        context: &str,
        derived_from: Vec<String>,
    ) -> String {
        let learning = SessionLearning {
            id: short_id(),
            insight: insight.to_string(),
            category,
            context: context.to_string(),
            derived_from,
            promoted: false,
            created_at: Utc::now(),
        };
        let id = learning.id.clone();
        self.learnings.push(learning);
        self.log_event(
            LogEventType::LearningDistilled,
            insight,
            Some(&id),
        );
        id
    }

    /// Get a learning for promotion review.
    pub fn get_learning_for_promotion(&self, id: &str) -> Option<&SessionLearning> {
        self.learnings.iter().find(|l| l.id == id && !l.promoted)
    }

    /// Mark a learning as promoted after human approval.
    pub fn mark_learning_promoted(&mut self, id: &str) {
        let insight = self.learnings.iter().find(|l| l.id == id).map(|l| l.insight.clone());
        if let Some(learning) = self.learnings.iter_mut().find(|l| l.id == id) {
            learning.promoted = true;
        }
        if let Some(insight) = insight {
            self.log_event(LogEventType::LearningPromoted, &insight, Some(id));
        }
    }

    /// Get unpromoted learnings (promotion candidates).
    pub fn promotion_candidates(&self) -> Vec<&SessionLearning> {
        self.learnings.iter().filter(|l| !l.promoted).collect()
    }

    // ── Backward Compat (v1 API surface) ─────────────────────────
    // These keep existing Brain methods working during transition.

    /// v1 compat: add_session_fact → add_belief (no task_id)
    pub fn add_session_fact(
        &mut self, content: &str, source: &str, confidence: f32, pattern: Vec<f32>,
    ) -> String {
        self.add_belief(content, source, None, confidence, pattern)
    }

    /// v1 compat: get_relevant_context → get_relevant_beliefs
    pub fn get_relevant_context(
        &mut self, pattern: &[f32], top_k: usize,
    ) -> Vec<(String, String, String, f32, f32)> {
        self.get_relevant_beliefs(pattern, top_k)
    }

    /// v1 compat: decay_old_facts → decay_old_beliefs
    pub fn decay_old_facts(&mut self) -> usize {
        self.decay_old_beliefs()
    }

    /// v1 compat: get_fact_for_promotion
    pub fn get_fact_for_promotion(&self, id: &str) -> Option<&SessionBelief> {
        self.beliefs.get(id).filter(|b| !b.promoted && !b.is_expired())
    }

    /// v1 compat: mark_promoted
    pub fn mark_promoted(&mut self, id: &str) {
        let content = self.beliefs.get(id).map(|b| b.content.clone());
        if let Some(belief) = self.beliefs.get_mut(id) {
            belief.promoted = true;
        }
        if let Some(content) = content {
            self.log_event(LogEventType::BeliefPromoted, &content, Some(id));
        }
    }

    // ── Topic Seeds (conversation continuity) ─────────────────────

    /// Store topic seeds from the current turn's recall results.
    /// These are the top N activated nodes that narration actually used.
    pub fn set_topic_seeds(&mut self, seeds: Vec<TopicSeed>) {
        self.topic_seeds = seeds;
    }

    /// Get topic seeds from the prior turn for conversation continuity.
    /// Returns a slice capped at max_count.
    pub fn get_topic_seeds(&self, max_count: usize) -> &[TopicSeed] {
        let end = self.topic_seeds.len().min(max_count);
        &self.topic_seeds[..end]
    }

    // ── Clear ────────────────────────────────────────────────────

    pub fn clear(&mut self) {
        self.context = SessionContext::default();
        self.tasks.clear();
        self.attempts.clear();
        self.beliefs.clear();
        self.learnings.clear();
        self.topic_seeds.clear();
        self.log_event(LogEventType::SessionCleared, "Full session memory wipe", None);
        // Keep log for audit trail — only clear data
    }

    // ── Status ───────────────────────────────────────────────────

    pub fn status(&self) -> SessionStatus {
        let active_beliefs: Vec<SessionBeliefSummary> = self.beliefs.values()
            .filter(|b| !b.is_expired() && !b.promoted)
            .map(|b| SessionBeliefSummary {
                id: b.id.clone(),
                content: b.content.clone(),
                source: b.source.clone(),
                task_id: b.task_id.clone(),
                confidence: b.confidence,
                recall_count: b.recall_count,
                is_learning_candidate: b.is_learning_candidate(),
                age_secs: Utc::now().signed_duration_since(b.created_at)
                    .num_seconds().max(0) as u64,
                ttl_secs: b.ttl_secs,
            })
            .collect();

        let task_summaries: Vec<SessionTaskSummary> = self.tasks.values()
            .map(|t| SessionTaskSummary {
                id: t.id.clone(),
                description: t.description.clone(),
                status: format!("{:?}", t.status),
                parent_id: t.parent_id.clone(),
                goal_seed: t.goal_seed.clone(),
                attempt_count: self.attempts.iter()
                    .filter(|a| a.task_id == t.id).count(),
            })
            .collect();

        let learning_summaries: Vec<SessionLearningSummary> = self.learnings.iter()
            .filter(|l| !l.promoted)
            .map(|l| SessionLearningSummary {
                id: l.id.clone(),
                insight: l.insight.clone(),
                category: format!("{:?}", l.category),
                derived_from: l.derived_from.clone(),
            })
            .collect();

        SessionStatus {
            active_beliefs: active_beliefs.len(),
            promoted_count: self.beliefs.values().filter(|b| b.promoted).count(),
            expired_count: self.beliefs.values().filter(|b| b.is_expired()).count(),
            task_count: self.tasks.len(),
            active_task_count: self.tasks.values()
                .filter(|t| t.status == TaskStatus::Active).count(),
            attempt_count: self.attempts.len(),
            learning_count: self.learnings.len(),
            log_entries: self.log.len(),
            recent_queries: self.context.recent_queries.clone(),
            active_goal: self.context.active_goal.clone(),
            tasks: task_summaries,
            beliefs: active_beliefs,
            learnings: learning_summaries,
            recent_log: self.log.iter().rev().take(10)
                .map(|e| format!("[{}] {:?}: {}", e.seq, e.event_type, e.description))
                .collect(),
        }
    }
} // end impl SessionMemory

// ━━━ Display Types ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionTaskSummary {
    pub id: String,
    pub description: String,
    pub status: String,
    pub parent_id: Option<String>,
    pub goal_seed: Option<String>,
    pub attempt_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionBeliefSummary {
    pub id: String,
    pub content: String,
    pub source: String,
    pub task_id: Option<String>,
    pub confidence: f32,
    pub recall_count: u32,
    pub is_learning_candidate: bool,
    pub age_secs: u64,
    pub ttl_secs: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionLearningSummary {
    pub id: String,
    pub insight: String,
    pub category: String,
    pub derived_from: Vec<String>,
}

/// Full snapshot of session memory state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionStatus {
    pub active_beliefs: usize,
    pub promoted_count: usize,
    pub expired_count: usize,
    pub task_count: usize,
    pub active_task_count: usize,
    pub attempt_count: usize,
    pub learning_count: usize,
    pub log_entries: usize,
    pub recent_queries: Vec<String>,
    pub active_goal: Option<String>,
    pub tasks: Vec<SessionTaskSummary>,
    pub beliefs: Vec<SessionBeliefSummary>,
    pub learnings: Vec<SessionLearningSummary>,
    pub recent_log: Vec<String>,
}

// ━━━ Internal Helpers ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() { return 0.0; }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 { return 0.0; }
    (dot / (mag_a * mag_b)).clamp(-1.0, 1.0)
}
