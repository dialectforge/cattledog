// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Idle auto-research — automatically triggers web research when the brain is idle.
//!
//! Trigger conditions (all must be true):
//!   1. `enabled` is set.
//!   2. The brain has been idle for at least `idle_threshold` (default 5 min).
//!   3. At least `cooldown` has passed since the last auto-research (default 2 min).
//!   4. Fewer than `max_per_period` researches have run in the current idle period (default 3).
//!
//! Usage:
//! ```ignore
//! // In your idle / tick loop:
//! if brain.web_idle.should_research() {
//!     if let Some(query) = brain.curiosity.wonder_queue.pending().first().map(|w| w.question.clone()) {
//!         brain.web_research(&query);
//!         brain.web_idle.record_research();
//!     }
//! }
//! // On every user interaction:
//! brain.web_idle.record_interaction();
//! ```

use std::time::{Duration, Instant};

// ── IdleConfig ────────────────────────────────────────────────────────────────

/// Configuration for idle auto-research behaviour.
#[derive(Debug, Clone)]
pub struct IdleConfig {
    /// How long the brain must be idle before auto-research triggers.
    pub idle_threshold: Duration,
    /// Minimum time between consecutive auto-research runs.
    pub cooldown: Duration,
    /// Maximum auto-research runs allowed per idle period.
    pub max_per_period: usize,
}

impl Default for IdleConfig {
    fn default() -> Self {
        IdleConfig {
            idle_threshold: Duration::from_secs(5 * 60), // 5 minutes
            cooldown:       Duration::from_secs(2 * 60), // 2 minutes
            max_per_period: 3,
        }
    }
}

// ── IdleResearcher ────────────────────────────────────────────────────────────

/// State machine that decides when idle auto-research should run.
#[derive(Debug)]
pub struct IdleResearcher {
    /// Set to `false` to disable all idle auto-research.
    pub enabled: bool,
    pub config: IdleConfig,

    last_interaction: Instant,
    last_research: Option<Instant>,
    /// Start of the current idle period (reset when a new idle period begins).
    period_start: Instant,
    /// Number of auto-research runs in the current period.
    researches_this_period: usize,
}

impl IdleResearcher {
    pub fn new() -> Self {
        let now = Instant::now();
        IdleResearcher {
            enabled:                 true,
            config:                  IdleConfig::default(),
            last_interaction:        now,
            last_research:           None,
            period_start:            now,
            researches_this_period:  0,
        }
    }

    /// Create with a custom configuration (useful for tests with short timeouts).
    pub fn with_config(config: IdleConfig) -> Self {
        let now = Instant::now();
        IdleResearcher {
            enabled:                 true,
            config,
            last_interaction:        now,
            last_research:           None,
            period_start:            now,
            researches_this_period:  0,
        }
    }

    // ── Interaction tracking ──────────────────────────────────────────────────

    /// Call this on every user interaction to reset the idle timer.
    ///
    /// If the brain was previously in an idle period (i.e., idle > threshold),
    /// the period counter is reset so a fresh idle period can begin.
    pub fn record_interaction(&mut self) {
        let now = Instant::now();
        // If we were in an idle period, start a new period.
        if now.duration_since(self.last_interaction) >= self.config.idle_threshold {
            self.period_start = now;
            self.researches_this_period = 0;
        }
        self.last_interaction = now;
    }

    /// Call this immediately after a research run completes.
    pub fn record_research(&mut self) {
        self.last_research = Some(Instant::now());
        self.researches_this_period += 1;
    }

    // ── Decision ─────────────────────────────────────────────────────────────

    /// Returns `true` if idle auto-research should trigger right now.
    ///
    /// Checks all four conditions: enabled, idle threshold, cooldown, period cap.
    pub fn should_research(&self) -> bool {
        if !self.enabled {
            return false;
        }

        let now = Instant::now();

        // Condition 2: idle long enough.
        if now.duration_since(self.last_interaction) < self.config.idle_threshold {
            return false;
        }

        // Condition 3: cooldown between runs.
        if let Some(last) = self.last_research {
            if now.duration_since(last) < self.config.cooldown {
                return false;
            }
        }

        // Condition 4: period cap.
        if self.researches_this_period >= self.config.max_per_period {
            return false;
        }

        true
    }

    // ── Diagnostics ──────────────────────────────────────────────────────────

    /// Seconds until the next research run is permitted (0 = allowed now).
    pub fn secs_until_next(&self) -> u64 {
        let now = Instant::now();

        let idle_elapsed = now.duration_since(self.last_interaction);
        if idle_elapsed < self.config.idle_threshold {
            return (self.config.idle_threshold - idle_elapsed).as_secs();
        }

        if let Some(last) = self.last_research {
            let cooldown_elapsed = now.duration_since(last);
            if cooldown_elapsed < self.config.cooldown {
                return (self.config.cooldown - cooldown_elapsed).as_secs();
            }
        }

        0
    }

    /// Human-readable status string for diagnostics.
    pub fn status(&self) -> String {
        let idle_secs = Instant::now()
            .duration_since(self.last_interaction)
            .as_secs();
        format!(
            "idle_researcher enabled={} runs={}/{} idle={}s next_in={}s",
            self.enabled,
            self.researches_this_period,
            self.config.max_per_period,
            idle_secs,
            self.secs_until_next()
        )
    }

    /// How many runs have been recorded this idle period.
    pub fn runs_this_period(&self) -> usize {
        self.researches_this_period
    }

    /// Seconds since the last user interaction.
    pub fn idle_secs(&self) -> u64 {
        Instant::now().duration_since(self.last_interaction).as_secs()
    }
}

impl Default for IdleResearcher {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn fast_config() -> IdleConfig {
        IdleConfig {
            idle_threshold: Duration::from_millis(50),
            cooldown:       Duration::from_millis(20),
            max_per_period: 2,
        }
    }

    #[test]
    fn test_no_research_when_disabled() {
        let mut r = IdleResearcher::with_config(fast_config());
        r.enabled = false;
        std::thread::sleep(Duration::from_millis(100));
        assert!(!r.should_research());
    }

    #[test]
    fn test_not_idle_yet() {
        let r = IdleResearcher::with_config(IdleConfig {
            idle_threshold: Duration::from_secs(60),
            cooldown:       Duration::from_millis(1),
            max_per_period: 10,
        });
        // Brain was just created — not idle yet.
        assert!(!r.should_research());
    }

    #[test]
    fn test_triggers_after_idle_threshold() {
        let r = IdleResearcher::with_config(fast_config());
        std::thread::sleep(Duration::from_millis(100));
        assert!(r.should_research(), "should trigger once idle threshold is reached");
    }

    #[test]
    fn test_cooldown_respected() {
        let mut r = IdleResearcher::with_config(fast_config());
        std::thread::sleep(Duration::from_millis(100)); // become idle
        assert!(r.should_research());

        r.record_research();
        // Immediately after recording — cooldown not yet elapsed.
        assert!(!r.should_research(), "cooldown should block immediate re-trigger");

        std::thread::sleep(Duration::from_millis(30)); // > cooldown (20ms)
        assert!(r.should_research(), "should allow after cooldown");
    }

    #[test]
    fn test_period_cap() {
        let mut r = IdleResearcher::with_config(fast_config()); // max 2 per period
        std::thread::sleep(Duration::from_millis(100));

        assert!(r.should_research());
        r.record_research();

        std::thread::sleep(Duration::from_millis(30));
        assert!(r.should_research());
        r.record_research();

        std::thread::sleep(Duration::from_millis(30));
        // Period cap reached (2 runs).
        assert!(!r.should_research(), "period cap should block third run");
    }

    #[test]
    fn test_interaction_resets_period() {
        let mut r = IdleResearcher::with_config(fast_config());
        std::thread::sleep(Duration::from_millis(100));

        // Run up to the cap.
        r.record_research();
        r.record_research();
        assert!(!r.should_research());

        // User interaction resets the period — but also resets idle timer.
        r.record_interaction();
        // Not idle yet after fresh interaction.
        assert!(!r.should_research());

        // Become idle again.
        std::thread::sleep(Duration::from_millis(100));
        assert!(r.should_research(), "fresh period should allow research again");
    }

    #[test]
    fn test_secs_until_next_zero_when_ready() {
        let r = IdleResearcher::with_config(fast_config());
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(r.secs_until_next(), 0);
    }

    #[test]
    fn test_status_string() {
        let r = IdleResearcher::new();
        let s = r.status();
        assert!(s.contains("idle_researcher"));
        assert!(s.contains("enabled=true"));
    }
}
