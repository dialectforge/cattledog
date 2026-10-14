// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Thalamus — central message bus, inter-region routing, priority queue.
//!
//! All inter-region communication in the v2 brain goes through the thalamus.
//! It is a synchronous priority queue (no async runtime required at this layer)
//! so any region can `send()` without holding a runtime handle.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

// ── MessagePriority ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MessagePriority {
    Low      = 0,
    Normal   = 1,
    High     = 2,
    Critical = 3,
}

// ── RegionStatus ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegionStatus {
    Healthy,
    Degraded { reason: String },
    Offline,
}

// ── BrainMessage ──────────────────────────────────────────────────────────────

/// All messages that flow between brain regions.
/// Payloads use simple owned types to avoid cross-module coupling.
#[derive(Debug, Clone)]
pub enum BrainMessage {
    // ── Query pipeline ────────────────────────────────────────────────────────

    /// Frontal → pipeline: query classified, routing begins.
    IntentClassified {
        query_id: String,
        /// One of: "definition", "mechanism", "comparison", "causal",
        ///         "procedure", "open_ended", "who", "creative"
        intent_tag: String,
        query: String,
    },

    /// Temporal → pipeline: KU extraction complete.
    KuExtracted {
        query_id: String,
        ku_count: usize,
    },

    /// Hippocampus → pipeline: session context snapshot ready.
    ContextReady {
        query_id: String,
        thread_id: String,
        turn_count: u32,
    },

    /// Parietal → pipeline: embeddings computed for a set of nodes.
    EmbeddingsReady {
        query_id: String,
        node_count: usize,
        dimension: usize,
    },

    /// Narration → output: response generated, ready to deliver.
    NarrationComplete {
        query_id: String,
        response: String,
        tokens_used: usize,
        template_used: String,
    },

    // ── Scar / safety ─────────────────────────────────────────────────────────

    /// Amygdala → all: a scar pattern matched incoming content.
    ScarAlert {
        scar_id: String,
        label: String,
        trigger: String,
        similarity: f32,
    },

    // ── Persistence ───────────────────────────────────────────────────────────

    /// Any region → Brainstem: request a save.
    SaveRequested {
        path: String,
        reason: String,
    },

    /// Brainstem → all: save complete.
    SaveCompleted {
        path: String,
        success: bool,
        error: Option<String>,
        nodes_saved: u64,
        bonds_saved: u64,
    },

    // ── Resource governance ───────────────────────────────────────────────────

    /// Hypothalamus → all: resource budget exceeded.
    BudgetExceeded {
        resource: String,
        current: f64,
        limit: f64,
    },

    /// Hypothalamus → all: autonomy tier changed.
    AutonomyChanged {
        new_tier: String,
        reason: String,
    },

    // ── Tool orchestration ────────────────────────────────────────────────────

    /// Cerebellum → output: tool call completed.
    ToolComplete {
        request_id: String,
        tool: String,
        success: bool,
        output: serde_json::Value,
    },

    // ── Health monitoring ─────────────────────────────────────────────────────

    /// Any region → Hypothalamus: request a health check.
    HealthCheck { from: String },

    /// Hypothalamus → from: health check response.
    HealthReport {
        region: String,
        status: RegionStatus,
        node_count: Option<usize>,
        extra: Option<String>,
    },

    // ── Lifecycle ─────────────────────────────────────────────────────────────

    Shutdown { graceful: bool },
}

// ── PrioritizedMessage ────────────────────────────────────────────────────────

#[derive(Debug)]
struct PrioritizedMessage {
    priority: MessagePriority,
    message: BrainMessage,
}

// ── Thalamus ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Thalamus {
    queue: VecDeque<PrioritizedMessage>,
    pub max_queue_size: usize,
    pub dropped_count: u64,
    pub routed_count: u64,
}

impl Default for Thalamus {
    fn default() -> Self {
        Thalamus::new()
    }
}

impl Thalamus {
    pub fn new() -> Self {
        Thalamus {
            queue: VecDeque::new(),
            max_queue_size: 1024,
            dropped_count: 0,
            routed_count: 0,
        }
    }

    /// Enqueue a message at the given priority.
    /// If at capacity, the lowest-priority pending message is dropped first.
    pub fn send(&mut self, message: BrainMessage, priority: MessagePriority) {
        if self.queue.len() >= self.max_queue_size {
            // Find and remove the first (oldest) low-priority message
            if let Some(pos) = self
                .queue
                .iter()
                .position(|m| m.priority < priority)
            {
                self.queue.remove(pos);
                self.dropped_count += 1;
            } else {
                // Queue is full of equal or higher priority — drop the new one
                self.dropped_count += 1;
                return;
            }
        }
        self.queue.push_back(PrioritizedMessage { priority, message });
        self.routed_count += 1;
    }

    /// Convenience for normal-priority messages.
    pub fn post(&mut self, message: BrainMessage) {
        self.send(message, MessagePriority::Normal);
    }

    /// Dequeue the next message in FIFO order (priority is used only for eviction).
    pub fn recv(&mut self) -> Option<BrainMessage> {
        self.queue.pop_front().map(|m| m.message)
    }

    /// Drain all pending messages.
    pub fn drain(&mut self) -> Vec<BrainMessage> {
        self.queue.drain(..).map(|m| m.message).collect()
    }

    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    // ── Priority-aware retrieval ───────────────────────────────────────────────

    /// Dequeue the highest-priority pending message.
    /// Ties are broken by FIFO order (earliest enqueued wins).
    pub fn recv_highest(&mut self) -> Option<BrainMessage> {
        if self.queue.is_empty() {
            return None;
        }
        // Find the position of the max-priority message (first found = oldest).
        let pos = self
            .queue
            .iter()
            .enumerate()
            .max_by_key(|(_, m)| m.priority)
            .map(|(i, _)| i)?;
        self.queue.remove(pos).map(|m| m.message)
    }

    /// Drain all pending messages sorted by priority (highest first).
    /// Within the same priority, FIFO order is preserved (stable sort).
    pub fn drain_by_priority(&mut self) -> Vec<BrainMessage> {
        let mut items: Vec<PrioritizedMessage> = self.queue.drain(..).collect();
        // Stable sort: higher priority value → earlier in output.
        items.sort_by(|a, b| b.priority.cmp(&a.priority));
        items.into_iter().map(|m| m.message).collect()
    }

    /// Dispatch the next message in FIFO order to a handler closure.
    ///
    /// The handler receives the message and its priority level.
    /// Returns `true` if a message was dispatched, `false` if the queue was empty.
    pub fn dispatch_next<F>(&mut self, handler: F) -> bool
    where
        F: FnOnce(BrainMessage, MessagePriority),
    {
        match self.queue.pop_front() {
            Some(pm) => {
                handler(pm.message, pm.priority);
                true
            }
            None => false,
        }
    }
}
