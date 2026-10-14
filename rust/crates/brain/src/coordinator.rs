// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Thinking Coordinator
//!
//! The deliberate thinking layer. Gives Lu the ability to maintain
//! goals, decompose tasks, propose iterative plans, and run an
//! agentic Gather → Plan → Act → Observe → Loop cycle.
//!
//! Everything maps to existing DF primitives:
//! - Goals → NodeType::Meta nodes
//! - Sub-goals → NodeType::Procedure nodes
//! - Hypotheses → BondProtocol with low weight + "hypothesis:" label
//! - Gather → Brain::recall() + reason()
//! - Act → Brain::learn_fast() + associate()
//! - Observe → coherence measurement + self_reflect patterns
//!
//! All operations route through the terminal bond.
//! All proposed changes require human approval.
//! Scars guard against dangerous reconnections.
//!
//! Created by John Dean Martin

use df_core::Seed;
use graph::network::RecallResult;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
// Generate short IDs without pulling in uuid crate
fn short_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:08x}", (nanos & 0xFFFFFFFF) as u32)
}
// ─── State Machine ──────────────────────────────────────

/// The thinking loop is a state machine. Each call to think_step()
/// advances one transition. The AwaitingApproval state pauses
/// until the human approves/rejects proposals — this is the
/// human-in-the-loop gate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum GoalState {
    /// Spreading activation from goal + conversation context
    Gathering,
    /// Decomposing results into sub-goal proposals
    Planning,
    /// Paused for human review of proposals
    AwaitingApproval,
    /// Executing approved proposals
    Acting,
    /// Measuring coherence, logging observations
    Observing,
    /// Finished — coherence met or max iterations reached
    Complete,
    /// Aborted or irreconcilable contradictions found
    Failed,
}
/// What kind of thinking proposal is this?
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ThinkingProposalType {
    /// Create a new sub-goal (Procedure node) decomposing the goal
    CreateSubGoal,
    /// Form a tentative bond between existing nodes
    FormHypothesisBond,
    /// Promote a hypothesis bond to permanent (raise weight, remove prefix)
    PromoteBond,
    /// Self-derived learning — the brain's own insight from thinking
    LearnInsight,
    /// Revoke something that contradicts established knowledge
    RevokeContradiction,
}

/// Status of an individual sub-goal within a thinking task.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SubGoalStatus {
    /// Created but not yet activated
    Pending,
    /// Currently being explored via spreading activation
    Active,
    /// Activation produced coherent results
    Resolved,
    /// Activation found contradictions or nothing relevant
    Failed,
}
// ─── Core Structs ───────────────────────────────────────

/// Configuration for a thinking session.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThinkConfig {
    /// Maximum iterations before forced completion (default 5, max 20)
    pub max_iterations: u32,
    /// Coherence score at which thinking is "done" (default 0.6)
    pub coherence_threshold: f32,
    /// If true, no real nodes/bonds are created — preview only
    pub dry_run: bool,
    /// If true, non-destructive proposals auto-approve (sub-goals, weak hypotheses)
    pub auto_approve_safe: bool,
}

impl Default for ThinkConfig {
    fn default() -> Self {
        Self {
            max_iterations: 5,
            coherence_threshold: 0.6,
            dry_run: false,
            auto_approve_safe: false,
        }
    }
}
/// A single sub-goal within a thinking task.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubGoal {
    /// The seed of the Procedure node in the graph
    pub seed: Seed,
    /// What this sub-goal is trying to explore/answer
    pub text: String,
    /// Current status
    pub status: SubGoalStatus,
    /// Recall result from activating this sub-goal (populated during Act)
    #[serde(skip)]
    pub activation_result: Option<RecallResult>,
    /// Coherence score for this sub-goal (0.0 = nothing relevant, 1.0 = perfect)
    pub coherence_score: f32,
}

/// A proposed action from the thinking loop.
/// Every proposed modification pauses for human review.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThinkingProposal {
    /// Unique identifier (short uuid)
    pub id: String,
    /// What kind of action
    pub proposal_type: ThinkingProposalType,
    /// Human-readable description of what this will do
    pub description: String,
    /// Seeds involved (source/target for bonds, parent for sub-goals)
    pub target_seeds: Vec<String>,
    /// Pre-checked scar conflicts (empty = safe)
    pub scar_conflicts: Vec<String>,
    /// When this proposal was created
    pub created_at: DateTime<Utc>,
}
/// A single observation from one iteration of the thinking loop.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Observation {
    /// Which iteration produced this observation
    pub iteration: u32,
    /// What action was taken
    pub action: String,
    /// What happened as a result
    pub outcome: String,
    /// How coherence changed (positive = improvement)
    pub coherence_delta: f32,
    /// When this was observed
    pub observed_at: DateTime<Utc>,
}
/// The complete temporary cluster representing a thinking task.
/// Tracks all created seeds for cleanup. Nothing persists unless
/// explicitly promoted.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GoalCluster {
    /// Seed of the root goal Meta node
    pub goal_seed: Seed,
    /// The original goal text
    pub goal_text: String,
    /// Decomposed sub-goals
    pub sub_goals: Vec<SubGoal>,
    /// Seeds of hypothesis bonds (tracked for cleanup)
    pub hypothesis_bond_seeds: Vec<Seed>,
    /// Seeds of ALL nodes created during thinking (tracked for cleanup)
    pub created_node_seeds: Vec<Seed>,
    /// Seeds of nodes promoted to permanent (excluded from cleanup)
    pub promoted_seeds: Vec<Seed>,
    /// Current iteration (0-based)
    pub iteration: u32,
    /// Config for this session
    pub config: ThinkConfig,
    /// Current state of the thinking loop
    pub state: GoalState,
    /// Observations from each iteration
    pub observations: Vec<Observation>,
    /// Proposals awaiting human review
    pub pending_proposals: Vec<ThinkingProposal>,
    /// Proposals that were approved (executed or queued)
    pub approved_proposals: Vec<ThinkingProposal>,
    /// Recall results cached per iteration (for reasoning)
    #[serde(skip)]
    pub activation_cache: Vec<RecallResult>,
    /// When thinking started
    pub started_at: DateTime<Utc>,
    /// Last coherence score
    pub coherence: f32,
}
/// The externally-visible state of a thinking session.
/// Returned by think_step(), think_status(), etc.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThinkingState {
    /// The goal being thought about
    pub goal: String,
    /// Current state machine position
    pub state: GoalState,
    /// Current iteration
    pub iteration: u32,
    /// Current coherence score
    pub coherence: f32,
    /// Number of sub-goals
    pub sub_goal_count: usize,
    /// Number of resolved sub-goals
    pub resolved_count: usize,
    /// Pending proposals needing approval
    pub pending_proposals: Vec<ThinkingProposal>,
    /// Most recent observation (if any)
    pub last_observation: Option<Observation>,
    /// Whether this is a dry run
    pub dry_run: bool,
}
/// Summary returned when thinking completes or is stopped.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThinkingSummary {
    /// The original goal
    pub goal: String,
    /// How many iterations were completed
    pub iterations_completed: u32,
    /// Final coherence score
    pub final_coherence: f32,
    /// Total nodes created during thinking
    pub nodes_created: usize,
    /// Total hypothesis bonds formed
    pub bonds_formed: usize,
    /// Nodes promoted to permanent
    pub nodes_promoted: usize,
    /// Nodes cleaned up (deactivated)
    pub nodes_cleaned: usize,
    /// All observations from the session
    pub observations: Vec<Observation>,
    /// Narrated conclusion from final activation state
    pub conclusion: String,
    /// Was this a dry run?
    pub dry_run: bool,
    /// How long thinking took
    pub duration_secs: f64,
}
// ─── GoalCluster Implementation ─────────────────────────

impl GoalCluster {
    /// Create a new thinking task. The root goal becomes a Meta node.
    /// The node is created by the Brain (which owns the network),
    /// and the seed is passed back here.
    pub fn new(goal_text: &str, goal_seed: Seed, config: ThinkConfig) -> Self {
        let mut created = Vec::new();
        created.push(goal_seed.clone());

        Self {
            goal_seed,
            goal_text: goal_text.to_string(),
            sub_goals: Vec::new(),
            hypothesis_bond_seeds: Vec::new(),
            created_node_seeds: created,
            promoted_seeds: Vec::new(),
            iteration: 0,
            config,
            state: GoalState::Gathering,
            observations: Vec::new(),
            pending_proposals: Vec::new(),
            approved_proposals: Vec::new(),
            activation_cache: Vec::new(),
            started_at: Utc::now(),
            coherence: 0.0,
        }
    }
    /// Record that a sub-goal node was created.
    pub fn add_sub_goal(&mut self, seed: Seed, text: String) {
        self.created_node_seeds.push(seed.clone());
        self.sub_goals.push(SubGoal {
            seed,
            text,
            status: SubGoalStatus::Pending,
            activation_result: None,
            coherence_score: 0.0,
        });
    }

    /// Record that a hypothesis bond was created.
    pub fn add_hypothesis_bond(&mut self, bond_seed: Seed) {
        self.hypothesis_bond_seeds.push(bond_seed);
    }

    /// Mark a node as promoted (excluded from cleanup).
    pub fn promote(&mut self, seed: &Seed) {
        if !self.promoted_seeds.contains(seed) {
            self.promoted_seeds.push(seed.clone());
        }
    }

    /// Is this seed promoted?
    pub fn is_promoted(&self, seed: &Seed) -> bool {
        self.promoted_seeds.contains(seed)
    }
    /// Get seeds that need cleanup (created but not promoted).
    pub fn cleanup_seeds(&self) -> Vec<Seed> {
        self.created_node_seeds.iter()
            .filter(|s| !self.promoted_seeds.contains(s))
            .cloned()
            .collect()
    }

    /// Get hypothesis bond seeds that need cleanup.
    /// All hypothesis bonds are cleaned up — promoted bonds
    /// get recreated as permanent bonds by the Act phase.
    pub fn cleanup_bond_seeds(&self) -> Vec<Seed> {
        self.hypothesis_bond_seeds.clone()
    }

    /// Add an observation from the current iteration.
    pub fn observe(&mut self, action: &str, outcome: &str, coherence_delta: f32) {
        self.observations.push(Observation {
            iteration: self.iteration,
            action: action.to_string(),
            outcome: outcome.to_string(),
            coherence_delta,
            observed_at: Utc::now(),
        });
    }
    /// Compute coherence: ratio of sub-goals resolved with decent scores.
    /// 0.0 = nothing resolved, 1.0 = everything resolved strongly.
    pub fn compute_coherence(&self) -> f32 {
        if self.sub_goals.is_empty() {
            // No sub-goals yet — base coherence on whether we have activation data
            return if self.activation_cache.is_empty() { 0.0 } else { 0.1 };
        }

        let resolved = self.sub_goals.iter()
            .filter(|sg| sg.status == SubGoalStatus::Resolved)
            .count() as f32;
        let total = self.sub_goals.len() as f32;

        // Weighted: resolved ratio * average coherence of resolved sub-goals
        let avg_coherence: f32 = self.sub_goals.iter()
            .filter(|sg| sg.status == SubGoalStatus::Resolved)
            .map(|sg| sg.coherence_score)
            .sum::<f32>() / resolved.max(1.0);

        (resolved / total) * avg_coherence
    }

    /// Should the loop continue?
    pub fn should_continue(&self) -> bool {
        self.state != GoalState::Complete
            && self.state != GoalState::Failed
            && self.iteration < self.config.max_iterations
            && self.coherence < self.config.coherence_threshold
    }
    /// Get the current externally-visible state.
    pub fn to_state(&self) -> ThinkingState {
        let resolved = self.sub_goals.iter()
            .filter(|sg| sg.status == SubGoalStatus::Resolved)
            .count();

        ThinkingState {
            goal: self.goal_text.clone(),
            state: self.state.clone(),
            iteration: self.iteration,
            coherence: self.coherence,
            sub_goal_count: self.sub_goals.len(),
            resolved_count: resolved,
            pending_proposals: self.pending_proposals.clone(),
            last_observation: self.observations.last().cloned(),
            dry_run: self.config.dry_run,
        }
    }

    /// Build the final summary when thinking completes.
    pub fn to_summary(&self, conclusion: String) -> ThinkingSummary {
        let elapsed = Utc::now()
            .signed_duration_since(self.started_at)
            .num_milliseconds() as f64 / 1000.0;

        ThinkingSummary {
            goal: self.goal_text.clone(),
            iterations_completed: self.iteration,
            final_coherence: self.coherence,
            nodes_created: self.created_node_seeds.len(),
            bonds_formed: self.hypothesis_bond_seeds.len(),
            nodes_promoted: self.promoted_seeds.len(),
            nodes_cleaned: self.cleanup_seeds().len(),
            observations: self.observations.clone(),
            conclusion,
            dry_run: self.config.dry_run,
            duration_secs: elapsed,
        }
    }
} // end impl GoalCluster
// ─── Proposal Helpers ───────────────────────────────────

impl ThinkingProposal {
    /// Create a new sub-goal proposal.
    pub fn sub_goal(description: &str, parent_seed: &str) -> Self {
        Self {
            id: short_id(),
            proposal_type: ThinkingProposalType::CreateSubGoal,
            description: description.to_string(),
            target_seeds: vec![parent_seed.to_string()],
            scar_conflicts: Vec::new(),
            created_at: Utc::now(),
        }
    }

    /// Create a hypothesis bond proposal.
    pub fn hypothesis_bond(
        description: &str,
        source_hex: &str,
        target_hex: &str,
        scar_conflicts: Vec<String>,
    ) -> Self {
        Self {
            id: short_id(),
            proposal_type: ThinkingProposalType::FormHypothesisBond,
            description: description.to_string(),
            target_seeds: vec![source_hex.to_string(), target_hex.to_string()],
            scar_conflicts,
            created_at: Utc::now(),
        }
    }
    /// Create an insight learning proposal.
    pub fn learn_insight(description: &str) -> Self {
        Self {
            id: short_id(),
            proposal_type: ThinkingProposalType::LearnInsight,
            description: description.to_string(),
            target_seeds: Vec::new(),
            scar_conflicts: Vec::new(),
            created_at: Utc::now(),
        }
    }

    /// Create a bond promotion proposal.
    pub fn promote_bond(description: &str, bond_seed_hex: &str) -> Self {
        Self {
            id: short_id(),
            proposal_type: ThinkingProposalType::PromoteBond,
            description: description.to_string(),
            target_seeds: vec![bond_seed_hex.to_string()],
            scar_conflicts: Vec::new(),
            created_at: Utc::now(),
        }
    }

    /// Is this a "safe" proposal that can be auto-approved?
    /// Safe = non-destructive: sub-goals and weak hypothesis bonds.
    pub fn is_safe(&self) -> bool {
        matches!(
            self.proposal_type,
            ThinkingProposalType::CreateSubGoal
                | ThinkingProposalType::FormHypothesisBond
        ) && self.scar_conflicts.is_empty()
    }
} // end impl ThinkingProposal