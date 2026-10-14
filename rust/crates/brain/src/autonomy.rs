// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Autonomous Execution Layer
//!
//! Gives Lu the ability to execute tasks with tiered safety controls.
//!
//! ## Execution Tiers
//!
//! Every action type maps to an execution tier:
//! - **Observe**: Read-only operations, always allowed.
//! - **Suggest**: Propose actions that require human approval.
//! - **Act**: Execute pre-approved action types automatically.
//! - **Create**: Spawn new goals/nodes autonomously (highest privilege).
//!
//! Default policy: everything at Suggest. Users opt-in to autonomy
//! by elevating specific action types. This is maximally safe.
//!
//! ## Impact Analysis
//!
//! Before any action executes, `assess_impact()` estimates what it
//! would change: affected nodes, risk level, reversibility. Actions
//! touching scars or hub nodes get elevated risk. Node deletion is
//! always Critical and never auto-approved.
//!
//! ## Integration
//!
//! Layers on TOP of the Thinking Layer's `is_safe()` check.
//! The existing propose/approve flow is preserved — autonomy just
//! decides whether a proposal can skip the human gate.
//!
//! Created by John Dean Martin

use crate::coordinator::{ThinkingProposal, ThinkingProposalType};
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

// ━━━ Execution Tiers ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Privilege level for an action type. Higher tiers = more autonomy.
/// Default for everything is Suggest (human-in-the-loop).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ExecutionTier {
    /// Read-only operations. Always allowed, no approval needed.
    Observe = 0,
    /// Propose actions. Human must approve before execution.
    Suggest = 1,
    /// Execute pre-approved action types automatically.
    Act = 2,
    /// Spawn new goals, nodes, or structural changes autonomously.
    Create = 3,
}

impl std::fmt::Display for ExecutionTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Observe => write!(f, "Observe"),
            Self::Suggest => write!(f, "Suggest"),
            Self::Act => write!(f, "Act"),
            Self::Create => write!(f, "Create"),
        }
    }
}

/// Canonical action types that map to execution tiers.
/// These cover everything the thinking layer can propose.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionType {
    /// Read/recall operations (always Observe tier)
    Recall,
    /// Create a sub-goal node
    CreateSubGoal,
    /// Form a hypothesis bond (temporary, low-weight)
    FormHypothesis,
    /// Promote a hypothesis bond to permanent
    PromoteBond,
    /// Learn a new insight (create permanent node)
    LearnInsight,
    /// Revoke a bond or node that contradicts knowledge
    Revoke,
    /// Strengthen an existing bond
    StrengthenBond,
    /// Weaken an existing bond
    WeakenBond,
    /// Create a new permanent node
    CreateNode,
    /// Delete a node (always Critical risk)
    DeleteNode,
}

impl std::fmt::Display for ActionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl ActionType {
    /// Map a ThinkingProposalType to an ActionType.
    pub fn from_proposal_type(pt: &ThinkingProposalType) -> Self {
        match pt {
            ThinkingProposalType::CreateSubGoal => Self::CreateSubGoal,
            ThinkingProposalType::FormHypothesisBond => Self::FormHypothesis,
            ThinkingProposalType::PromoteBond => Self::PromoteBond,
            ThinkingProposalType::LearnInsight => Self::LearnInsight,
            ThinkingProposalType::RevokeContradiction => Self::Revoke,
        }
    }
}

// ━━━ Risk Assessment ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Risk level for a proposed action.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskLevel {
    /// No meaningful graph impact (reads, status checks)
    Negligible = 0,
    /// Creates temporary or low-weight structures
    Low = 1,
    /// Modifies existing bonds or promotes temporary structures
    Medium = 2,
    /// Touches scars, hub nodes (>50 bonds), or revokes knowledge
    High = 3,
    /// Node deletion or structural damage — never auto-approved
    Critical = 4,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Negligible => write!(f, "Negligible"),
            Self::Low => write!(f, "Low"),
            Self::Medium => write!(f, "Medium"),
            Self::High => write!(f, "High"),
            Self::Critical => write!(f, "Critical"),
        }
    }
}

/// Full impact assessment for a proposed action.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImpactAssessment {
    /// What this action will do (human-readable)
    pub action_description: String,
    /// Seed hex strings of nodes that will be affected
    pub affected_nodes: Vec<String>,
    /// Estimated risk
    pub risk_level: RiskLevel,
    /// Can this action be undone?
    pub reversible: bool,
    /// Estimated number of bond changes (positive = creates, negative = removes)
    pub estimated_bond_changes: i32,
    /// Why this risk level was assigned
    pub risk_reason: String,
}

impl ImpactAssessment {
    /// Assess impact of a ThinkingProposal.
    ///
    /// `has_scar_conflicts`: does the proposal touch scarred paths?
    /// `max_bond_count`: highest bond count among affected nodes (hub detection).
    pub fn from_proposal(
        proposal: &ThinkingProposal,
        has_scar_conflicts: bool,
        max_bond_count: usize,
    ) -> Self {
        let action_type = ActionType::from_proposal_type(&proposal.proposal_type);
        let is_hub = max_bond_count > 50;

        let (risk_level, risk_reason, reversible, bond_changes) = match &action_type {
            ActionType::Recall => (
                RiskLevel::Negligible,
                "Read-only operation".to_string(),
                true, 0,
            ),
            ActionType::CreateSubGoal => (
                RiskLevel::Low,
                "Creates temporary sub-goal node".to_string(),
                true, 0,
            ),
            ActionType::FormHypothesis => {
                if has_scar_conflicts {
                    (RiskLevel::High,
                     "Hypothesis bond touches scarred path".to_string(),
                     true, 1)
                } else {
                    (RiskLevel::Low,
                     "Creates temporary low-weight bond".to_string(),
                     true, 1)
                }
            },
            ActionType::PromoteBond => {
                let risk = if is_hub { RiskLevel::High } else { RiskLevel::Medium };
                let reason = if is_hub {
                    format!("Promoting bond on hub node ({} bonds)", max_bond_count)
                } else {
                    "Promotes hypothesis to permanent bond".to_string()
                };
                (risk, reason, false, 1)
            },
            ActionType::LearnInsight => (
                RiskLevel::Medium,
                "Creates permanent knowledge node".to_string(),
                false, 0,
            ),
            ActionType::Revoke => {
                let risk = if has_scar_conflicts || is_hub {
                    RiskLevel::High
                } else {
                    RiskLevel::Medium
                };
                (risk,
                 "Revokes existing knowledge — creates scar".to_string(),
                 false, -1)
            },
            ActionType::StrengthenBond => (
                RiskLevel::Low,
                "Increases existing bond weight".to_string(),
                true, 0,
            ),
            ActionType::WeakenBond => (
                RiskLevel::Medium,
                "Decreases existing bond weight".to_string(),
                true, 0,
            ),
            ActionType::CreateNode => (
                RiskLevel::Medium,
                "Creates permanent node in graph".to_string(),
                false, 0,
            ),
            ActionType::DeleteNode => (
                RiskLevel::Critical,
                "DESTRUCTIVE: removes node and all its bonds".to_string(),
                false, -(max_bond_count as i32),
            ),
        };

        Self {
            action_description: proposal.description.clone(),
            affected_nodes: proposal.target_seeds.clone(),
            risk_level,
            reversible,
            estimated_bond_changes: bond_changes,
            risk_reason,
        }
    }
}

// ━━━ Execution Policy ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Maps each action type to its required execution tier.
/// This is the user-configurable permission table.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionPolicy {
    /// Tier assignments. Missing entries default to Suggest.
    tiers: HashMap<ActionType, ExecutionTier>,
}

impl Default for ExecutionPolicy {
    /// Default: maximally safe. Everything at Suggest except Recall (Observe).
    fn default() -> Self {
        let mut tiers = HashMap::new();
        tiers.insert(ActionType::Recall, ExecutionTier::Observe);
        tiers.insert(ActionType::CreateSubGoal, ExecutionTier::Suggest);
        tiers.insert(ActionType::FormHypothesis, ExecutionTier::Suggest);
        tiers.insert(ActionType::PromoteBond, ExecutionTier::Suggest);
        tiers.insert(ActionType::LearnInsight, ExecutionTier::Suggest);
        tiers.insert(ActionType::Revoke, ExecutionTier::Suggest);
        tiers.insert(ActionType::StrengthenBond, ExecutionTier::Suggest);
        tiers.insert(ActionType::WeakenBond, ExecutionTier::Suggest);
        tiers.insert(ActionType::CreateNode, ExecutionTier::Suggest);
        tiers.insert(ActionType::DeleteNode, ExecutionTier::Suggest);
        Self { tiers }
    }
}

impl ExecutionPolicy {
    /// Get the tier for an action type. Defaults to Suggest if unset.
    pub fn get_tier(&self, action: &ActionType) -> ExecutionTier {
        self.tiers.get(action).cloned().unwrap_or(ExecutionTier::Suggest)
    }

    /// Elevate an action type to a higher (or lower) tier.
    /// Returns the previous tier.
    pub fn set_tier(&mut self, action: ActionType, tier: ExecutionTier) -> ExecutionTier {
        let prev = self.get_tier(&action);
        self.tiers.insert(action, tier);
        prev
    }

    /// Can this action execute without human approval at the current tier?
    pub fn can_auto_execute(&self, action: &ActionType) -> bool {
        let tier = self.get_tier(action);
        tier >= ExecutionTier::Act
    }

    /// Snapshot for display.
    pub fn to_summary(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = self.tiers.iter()
            .map(|(a, t)| (a.to_string(), t.to_string()))
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }
}

// ━━━ Execution Pipeline ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Result of attempting to execute a queued action.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ExecutionResult {
    /// Action executed successfully
    Success { action_id: String, description: String },
    /// Action failed during execution
    Failed { action_id: String, reason: String },
    /// Action blocked — needs human approval at current tier
    Blocked { action_id: String, reason: String },
}

/// A queued action awaiting execution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueuedAction {
    /// Unique ID (same as the ThinkingProposal id)
    pub id: String,
    /// The action type
    pub action_type: ActionType,
    /// Human-readable description
    pub description: String,
    /// Target seed hex strings for the action
    pub target_seeds: Vec<String>,
    /// Pre-computed impact assessment
    pub impact: ImpactAssessment,
    /// When this was queued
    pub queued_at: DateTime<Utc>,
    /// Has this been approved by a human? (overrides tier check)
    pub human_approved: bool,
}

/// The execution pipeline: queue + policy + execution logic.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionPipeline {
    /// Actions waiting to execute
    queue: Vec<QueuedAction>,
    /// Completed execution results (for audit trail)
    history: Vec<ExecutionResult>,
}

impl Default for ExecutionPipeline {
    fn default() -> Self {
        Self {
            queue: Vec::new(),
            history: Vec::new(),
        }
    }
}

impl ExecutionPipeline {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a proposal for execution. Computes impact assessment.
    pub fn enqueue(
        &mut self,
        proposal: &ThinkingProposal,
        has_scar_conflicts: bool,
        max_bond_count: usize,
    ) -> String {
        let impact = ImpactAssessment::from_proposal(
            proposal, has_scar_conflicts, max_bond_count,
        );
        let action_type = ActionType::from_proposal_type(&proposal.proposal_type);
        let id = proposal.id.clone();

        let action = QueuedAction {
            id: id.clone(),
            action_type,
            description: proposal.description.clone(),
            target_seeds: proposal.target_seeds.clone(),
            impact,
            queued_at: Utc::now(),
            human_approved: false,
        };

        self.queue.push(action);
        id
    }

    /// Mark a queued action as human-approved (overrides tier).
    pub fn approve(&mut self, action_id: &str) -> bool {
        if let Some(action) = self.queue.iter_mut().find(|a| a.id == action_id) {
            action.human_approved = true;
            true
        } else {
            false
        }
    }

    /// Check if the next action in the queue can execute under the given policy.
    /// Returns the action and whether it's allowed.
    pub fn peek_next(&self, policy: &ExecutionPolicy) -> Option<(&QueuedAction, bool)> {
        self.queue.first().map(|action| {
            let allowed = action.human_approved
                || (policy.can_auto_execute(&action.action_type)
                    && action.impact.risk_level < RiskLevel::Critical);
            (action, allowed)
        })
    }

    /// Try to execute the next queued action.
    /// Returns Blocked if tier doesn't allow auto-execution.
    /// On success, returns the QueuedAction so the caller (Brain)
    /// can perform the actual graph mutation.
    pub fn try_execute_next(
        &mut self,
        policy: &ExecutionPolicy,
    ) -> Option<(ExecutionResult, Option<QueuedAction>)> {
        if self.queue.is_empty() {
            return None;
        }

        let action = &self.queue[0];
        let allowed = action.human_approved
            || (policy.can_auto_execute(&action.action_type)
                && action.impact.risk_level < RiskLevel::Critical);

        if allowed {
            let action = self.queue.remove(0);
            let result = ExecutionResult::Success {
                action_id: action.id.clone(),
                description: action.description.clone(),
            };
            self.history.push(result.clone());
            Some((result, Some(action)))
        } else {
            let reason = format!(
                "Tier {} required for {:?}, current tier is {} (risk: {})",
                ExecutionTier::Act,
                action.action_type,
                policy.get_tier(&action.action_type),
                action.impact.risk_level,
            );
            let result = ExecutionResult::Blocked {
                action_id: action.id.clone(),
                reason,
            };
            Some((result, None))
        }
    }

    /// Get all queued actions.
    pub fn queued(&self) -> &[QueuedAction] {
        &self.queue
    }

    /// Get execution history.
    pub fn history(&self) -> &[ExecutionResult] {
        &self.history
    }

    /// Remove all queued actions. History is preserved.
    pub fn clear_queue(&mut self) {
        self.queue.clear();
    }

    /// Number of actions waiting.
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }
}

// ━━━ Autonomy Engine ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Top-level wrapper: policy + pipeline.
/// Lives on Brain as `brain.autonomy`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AutonomyEngine {
    /// The permission table
    pub policy: ExecutionPolicy,
    /// The execution queue + history
    pub pipeline: ExecutionPipeline,
}

impl Default for AutonomyEngine {
    fn default() -> Self {
        Self {
            policy: ExecutionPolicy::default(),
            pipeline: ExecutionPipeline::new(),
        }
    }
}

impl AutonomyEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Shorthand: can this proposal auto-execute?
    pub fn can_auto_approve(&self, proposal: &ThinkingProposal) -> bool {
        let action_type = ActionType::from_proposal_type(&proposal.proposal_type);
        // Must pass BOTH: tier allows AND is_safe AND not Critical risk
        proposal.is_safe()
            && self.policy.can_auto_execute(&action_type)
    }

    /// Full status snapshot for display.
    pub fn status(&self) -> AutonomyStatus {
        AutonomyStatus {
            policy: self.policy.to_summary(),
            queued_actions: self.pipeline.queued().len(),
            queue: self.pipeline.queued().iter().map(|a| QueuedActionSummary {
                id: a.id.clone(),
                action_type: a.action_type.to_string(),
                description: a.description.clone(),
                risk_level: a.impact.risk_level.to_string(),
                reversible: a.impact.reversible,
                human_approved: a.human_approved,
            }).collect(),
            recent_history: self.pipeline.history().iter().rev().take(10).map(|r| {
                match r {
                    ExecutionResult::Success { action_id, description } =>
                        format!("✓ {} — {}", action_id, description),
                    ExecutionResult::Failed { action_id, reason } =>
                        format!("✗ {} — {}", action_id, reason),
                    ExecutionResult::Blocked { action_id, reason } =>
                        format!("⊘ {} — {}", action_id, reason),
                }
            }).collect(),
        }
    }
}

// ━━━ Display Types ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Summary of a queued action for status display.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueuedActionSummary {
    pub id: String,
    pub action_type: String,
    pub description: String,
    pub risk_level: String,
    pub reversible: bool,
    pub human_approved: bool,
}

/// Full autonomy status snapshot.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AutonomyStatus {
    /// Current tier assignments: (action_type, tier)
    pub policy: Vec<(String, String)>,
    /// Number of actions in queue
    pub queued_actions: usize,
    /// Details of queued actions
    pub queue: Vec<QueuedActionSummary>,
    /// Recent execution history (most recent first, up to 10)
    pub recent_history: Vec<String>,
}
