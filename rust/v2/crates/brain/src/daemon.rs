// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Background daemon — schedule-driven autonomous brain activity.
//!
//! Inspired by the brain's **default mode network**: even when the user is
//! silent, the brain continues consolidating memories, exploring gaps, and
//! reflecting on its own performance.
//!
//! ## Design — no spawned threads
//!
//! Spawning OS threads in a library crate creates lifecycle and ownership
//! headaches.  Instead, the caller (terminal REPL, Tauri backend, or MCP
//! server event loop) calls `Brain::tick()` at whatever cadence it likes.
//! `DaemonMode::tick()` checks elapsed times and returns a list of
//! `DaemonAction` values that `Brain::tick()` dispatches.
//!
//! ## Typical usage
//!
//! ```text
//! brain.flags.daemon_mode = true;
//! brain.daemon.start();
//!
//! // In your event loop (e.g. every second):
//! brain.tick();
//! ```

use std::time::{Duration, Instant};

// ── DaemonAction ──────────────────────────────────────────────────────────────

/// What the daemon wants `Brain::tick()` to execute on this tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonAction {
    /// Run a rumination step (bond discovery + contradiction detection + decay).
    Ruminate,
    /// Check whether idle web research should fire; if so, run it.
    IdleResearch,
    /// Run a hippocampus consolidation pass.
    Consolidate,
    /// Run a self-improvement meta-reflection.
    SelfReflect,
    /// Generate a full system health report and log it.
    HealthReport,
}

// ── DaemonStatus ─────────────────────────────────────────────────────────────

/// Live snapshot of the daemon's activity state.
///
/// `Instant` is not serialisable; this is runtime-only telemetry.
#[derive(Debug, Default)]
pub struct DaemonStatus {
    pub last_rumination:    Option<Instant>,
    pub last_consolidation: Option<Instant>,
    pub last_research:      Option<Instant>,
    pub last_reflection:    Option<Instant>,
    pub last_health:        Option<Instant>,
    /// Total number of `tick()` calls since `start()`.
    pub ticks_total: u64,
}

impl DaemonStatus {
    pub fn new() -> Self {
        DaemonStatus::default()
    }

    /// Human-readable one-liner suitable for the `:status` REPL command.
    pub fn format(&self) -> String {
        let fmt = |opt: &Option<Instant>| match opt {
            None    => "never".to_string(),
            Some(t) => {
                let secs = t.elapsed().as_secs();
                if secs < 60 { format!("{}s ago", secs) }
                else { format!("{}m ago", secs / 60) }
            }
        };
        format!(
            "DaemonStatus {{ ticks={} | ruminate={} | consolidate={} | research={} | reflect={} | health={} }}",
            self.ticks_total,
            fmt(&self.last_rumination),
            fmt(&self.last_consolidation),
            fmt(&self.last_research),
            fmt(&self.last_reflection),
            fmt(&self.last_health),
        )
    }
}

// ── DaemonSchedule ────────────────────────────────────────────────────────────

/// The configurable schedule driving daemon actions.
///
/// All intervals default to the cognitive-science-inspired values from the
/// spec; the caller may override them (e.g. speed them up in tests).
#[derive(Debug)]
pub struct DaemonSchedule {
    /// How often to run a rumination step (default: 30 s).
    pub rumination_interval:    Duration,
    /// How often to check idle web research (default: 5 min).
    pub idle_research_interval: Duration,
    /// How often to run a consolidation pass (default: 10 min).
    pub consolidation_interval: Duration,
    /// How often to run self-improvement reflection (default: 30 min).
    pub reflection_interval:    Duration,
    /// How often to emit a full health report (default: 60 min).
    pub health_interval:        Duration,
}

impl Default for DaemonSchedule {
    fn default() -> Self {
        DaemonSchedule {
            rumination_interval:    Duration::from_secs(30),
            idle_research_interval: Duration::from_secs(300),   // 5 min
            consolidation_interval: Duration::from_secs(600),   // 10 min
            reflection_interval:    Duration::from_secs(1_800), // 30 min
            health_interval:        Duration::from_secs(3_600), // 60 min
        }
    }
}

// ── DaemonMode ────────────────────────────────────────────────────────────────

/// The background daemon.
///
/// Call `start()` to activate, then call `tick()` from your event loop.
#[derive(Debug)]
pub struct DaemonMode {
    pub schedule: DaemonSchedule,
    pub status:   DaemonStatus,
    /// True once `start()` has been called and `stop()` has not.
    pub running: bool,
}

impl Default for DaemonMode {
    fn default() -> Self {
        DaemonMode {
            schedule: DaemonSchedule::default(),
            status:   DaemonStatus::new(),
            running:  false,
        }
    }
}

impl DaemonMode {
    pub fn new() -> Self {
        DaemonMode::default()
    }

    /// Activate the daemon.  Until this is called, `tick()` is a no-op.
    pub fn start(&mut self) {
        self.running = true;
    }

    /// Deactivate the daemon.
    pub fn stop(&mut self) {
        self.running = false;
    }

    /// Called by the event loop on every cycle.
    ///
    /// Returns the (possibly empty) list of actions that are due on this tick.
    /// Each action is due when either it has never run or its interval has
    /// elapsed since the last run.
    ///
    /// **Side-effect**: updates `status` timestamps for every action returned.
    pub fn tick(&mut self) -> Vec<DaemonAction> {
        self.status.ticks_total += 1;
        if !self.running {
            return vec![];
        }

        let now = Instant::now();
        let mut actions = Vec::new();

        if is_due(&self.status.last_rumination, self.schedule.rumination_interval) {
            self.status.last_rumination = Some(now);
            actions.push(DaemonAction::Ruminate);
        }
        if is_due(&self.status.last_research, self.schedule.idle_research_interval) {
            self.status.last_research = Some(now);
            actions.push(DaemonAction::IdleResearch);
        }
        if is_due(&self.status.last_consolidation, self.schedule.consolidation_interval) {
            self.status.last_consolidation = Some(now);
            actions.push(DaemonAction::Consolidate);
        }
        if is_due(&self.status.last_reflection, self.schedule.reflection_interval) {
            self.status.last_reflection = Some(now);
            actions.push(DaemonAction::SelfReflect);
        }
        if is_due(&self.status.last_health, self.schedule.health_interval) {
            self.status.last_health = Some(now);
            actions.push(DaemonAction::HealthReport);
        }

        actions
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn is_due(last: &Option<Instant>, interval: Duration) -> bool {
    match last {
        None    => true,
        Some(t) => t.elapsed() >= interval,
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn daemon_off_by_default() {
        let mut d = DaemonMode::new();
        // Before start() — should return nothing even though intervals are "due"
        let actions = d.tick();
        assert!(actions.is_empty(), "daemon should be off by default");
    }

    #[test]
    fn daemon_fires_all_on_first_tick_after_start() {
        let mut d = DaemonMode::new();
        d.start();
        let actions = d.tick();
        // All 5 actions are due on the very first tick (None → always due).
        assert_eq!(actions.len(), 5);
    }

    #[test]
    fn daemon_does_not_double_fire() {
        let mut d = DaemonMode::new();
        // Speed up all intervals to 100 ms for the test.
        d.schedule.rumination_interval    = Duration::from_millis(100);
        d.schedule.idle_research_interval = Duration::from_millis(100);
        d.schedule.consolidation_interval = Duration::from_millis(100);
        d.schedule.reflection_interval    = Duration::from_millis(100);
        d.schedule.health_interval        = Duration::from_millis(100);
        d.start();
        let first  = d.tick(); // fires all
        let second = d.tick(); // should fire nothing (intervals not elapsed)
        assert_eq!(first.len(), 5);
        assert!(second.is_empty(), "should not re-fire before interval elapses");
    }

    #[test]
    fn daemon_tick_count_increments_even_when_stopped() {
        let mut d = DaemonMode::new();
        d.tick();
        d.tick();
        assert_eq!(d.status.ticks_total, 2);
    }
}
