// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Rumination Loop
//!
//! The 5th language engine component. Lu thinking when nobody's talking.
//!
//! A background thread that periodically:
//! 1. Picks a random active node
//! 2. Runs spreading activation from that node
//! 3. Finds strong co-activations that have NO direct bond
//! 4. Proposes new bonds (queues them for review or auto-creates weak ones)
//!
//! This is where curiosity emerges from the architecture.
//! The brain stumbles across connections nobody explicitly taught.
//!
//! "That's the piece that turns it from a system that only responds
//!  when spoken to into something that's always quietly turning
//!  things over."

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::thread;

/// Stop words to exclude from word-overlap checks in rumination.
const RUMINATION_STOP: &[&str] = &[
    "a", "an", "the", "is", "are", "was", "were", "be", "been", "am",
    "do", "does", "did", "has", "have", "had", "it", "its", "i", "me",
    "my", "we", "our", "you", "your", "he", "she", "him", "her", "his",
    "they", "them", "their", "this", "that", "these", "those", "at",
    "for", "of", "with", "by", "from", "and", "or", "but", "not", "no",
    "to", "in", "on", "as", "if", "so", "than", "very", "too", "also",
    "just", "can", "will", "may", "would", "could", "should", "about",
];

/// Extract meaningful words from a label (lowercase, stop words removed).
fn meaningful_words(text: &str) -> HashSet<String> {
    let stop: HashSet<&str> = RUMINATION_STOP.iter().copied().collect();
    text.to_lowercase()
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .filter(|w| w.len() > 1 && !stop.contains(w.as_str()))
        .collect()
}

/// A discovered gap — two nodes that co-activate strongly
/// but have no direct bond between them.
#[derive(Clone, Debug)]
pub struct RuminationDiscovery {
    /// Seed hex of the origin node (randomly picked)
    pub origin_hex: String,
    pub origin_label: String,
    /// Seed hex of node A in the discovered pair
    pub node_a_hex: String,
    pub node_a_label: String,
    /// Seed hex of node B in the discovered pair
    pub node_b_hex: String,
    pub node_b_label: String,
    /// How strongly both activated from the origin
    pub co_activation_strength: f32,
}

/// Report of what the rumination loop has been thinking about.
#[derive(Clone, Debug, Default)]
pub struct RuminationReport {
    /// Total rumination cycles completed since start
    pub cycles: u64,
    /// Bonds created by rumination
    pub bonds_created: u64,
    /// Discoveries that were skipped (already bonded, too weak, etc.)
    pub skipped: u64,
    /// Most recent discoveries (ring buffer, last N)
    pub recent_discoveries: Vec<RuminationDiscovery>,
}

/// Handle to control the rumination loop from outside.
pub struct RuminationHandle {
    /// Signal to stop the loop
    stop_signal: Arc<AtomicBool>,
    /// The thread handle
    thread: Option<thread::JoinHandle<()>>,
    /// Shared report — the UI reads this to show what Lu is thinking about
    pub report: Arc<Mutex<RuminationReport>>,
}

impl RuminationHandle {
    /// Stop the rumination loop gracefully.
    pub fn stop(&mut self) {
        self.stop_signal.store(true, Ordering::Relaxed);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }

    /// Check if the loop is still running.
    pub fn is_running(&self) -> bool {
        self.thread.as_ref().map_or(false, |h| !h.is_finished())
    }

    /// Get a snapshot of the current report.
    pub fn snapshot(&self) -> RuminationReport {
        self.report.lock().unwrap().clone()
    }
}

impl Drop for RuminationHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Start the rumination loop on a background thread.
///
/// The brain is shared via Arc<Mutex<Brain>>. The loop acquires the
/// lock briefly for each cycle, then releases it so the UI thread
/// (conversations, commands) can use the brain freely.
///
/// `interval` controls how often the loop runs (e.g., every 30 seconds).
/// `min_strength` is the minimum co-activation strength to consider a
/// discovery worth bonding (default 0.15).
///
/// The brain must be wrapped in Arc<Mutex<Brain>> by the caller
/// (main.rs / app.rs). This function takes a clone of that Arc.
pub fn start<B: BrainRuminator + Send + 'static>(
    brain: Arc<Mutex<B>>,
    interval: Duration,
    min_strength: f32,
) -> RuminationHandle {
    let stop_signal = Arc::new(AtomicBool::new(false));
    let stop = stop_signal.clone();
    let report = Arc::new(Mutex::new(RuminationReport::default()));
    let report_clone = report.clone();

    let thread = thread::spawn(move || {
        rumination_loop(brain, stop, report_clone, interval, min_strength);
    });

    RuminationHandle {
        stop_signal,
        thread: Some(thread),
        report,
    }
}

/// Trait that abstracts the brain operations the rumination loop needs.
/// This keeps the loop testable and avoids circular dependency issues.
pub trait BrainRuminator {
    /// Get the hex keys of all active nodes.
    fn active_node_hexes(&self) -> Vec<String>;

    /// Get the label for a node by hex.
    fn node_label(&self, hex: &str) -> Option<String>;

    /// Run spreading activation from a single node.
    /// Returns: Vec of (seed_hex, label, activation_strength).
    fn activate_from(&mut self, seed_hex: &str, depth: usize, top_k: usize)
        -> Vec<(String, String, f32)>;

    /// Check if a direct bond exists between two nodes.
    fn has_bond(&self, hex_a: &str, hex_b: &str) -> bool;

    /// Create a weak bond between two nodes. Returns true if created.
    fn create_rumination_bond(
        &mut self,
        hex_a: &str,
        hex_b: &str,
        strength: f32,
    ) -> bool;
}

/// The actual loop. Runs until stop_signal is set.
fn rumination_loop<B: BrainRuminator>(
    brain: Arc<Mutex<B>>,
    stop: Arc<AtomicBool>,
    report: Arc<Mutex<RuminationReport>>,
    interval: Duration,
    min_strength: f32,
) {
    // Use a simple RNG — pick a "random" index based on cycle count + time
    let mut cycle: u64 = 0;

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }

        // Sleep first — give the brain time to boot, and space between cycles
        let sleep_start = Instant::now();
        while sleep_start.elapsed() < interval {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            thread::sleep(Duration::from_millis(500));
        }

        // ─── One rumination cycle ─────────────────────
        cycle += 1;

        // Step 1: Pick a random active node.
        // We hold the lock briefly to get the node list, then release.
        let (all_hexes, origin_hex, origin_label) = {
            let brain = brain.lock().unwrap();
            let hexes = brain.active_node_hexes();
            if hexes.is_empty() {
                continue; // Brain is empty, nothing to ruminate on
            }
            // Pseudo-random selection using cycle count + timestamp
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;
            let idx = ((cycle.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407)) ^ now) as usize % hexes.len();
            let hex = hexes[idx].clone();
            let label = brain.node_label(&hex).unwrap_or_default();
            (hexes, hex, label)
        };

        // Need at least 10 nodes for rumination to be useful
        if all_hexes.len() < 10 {
            continue;
        }

        // Step 2: Run spreading activation from this node.
        // Cast a wider net (top 20) since we'll filter heavily.
        let activated = {
            let mut brain = brain.lock().unwrap();
            brain.activate_from(&origin_hex, 2, 20)
        };

        if activated.len() < 2 {
            continue; // Not enough activation to find gaps
        }

        // Step 3: Find pairs of strongly co-activated nodes
        // that have NO direct bond between them.
        //
        // SEMANTIC FILTER: To avoid noise bonds (e.g. "triangle height"
        // ↔ "motor skills"), we require that discovered pairs share at
        // least one meaningful word with each other OR with the origin.
        // This ensures rumination bonds are topically grounded.
        let origin_words = meaningful_words(&origin_label);
        let activated_words: Vec<HashSet<String>> = activated.iter()
            .map(|(_, label, _)| meaningful_words(label))
            .collect();

        let mut discoveries: Vec<RuminationDiscovery> = Vec::new();
        {
            let brain = brain.lock().unwrap();
            for i in 0..activated.len() {
                for j in (i + 1)..activated.len() {
                    let (ref hex_a, ref label_a, str_a) = activated[i];
                    let (ref hex_b, ref label_b, str_b) = activated[j];

                    // Both must be above minimum strength
                    if str_a < min_strength || str_b < min_strength {
                        continue;
                    }

                    // Skip if they're already bonded
                    if brain.has_bond(hex_a, hex_b) {
                        continue;
                    }

                    // Semantic check: nodes must share a word with each
                    // other, or both share a word with the origin node.
                    let words_a = &activated_words[i];
                    let words_b = &activated_words[j];
                    let shared_ab = words_a.intersection(words_b).count();
                    let shared_a_origin = words_a.intersection(&origin_words).count();
                    let shared_b_origin = words_b.intersection(&origin_words).count();

                    let semantically_related =
                        shared_ab >= 1 ||                              // direct word overlap
                        (shared_a_origin >= 1 && shared_b_origin >= 1); // both relate to origin

                    if !semantically_related {
                        continue;
                    }

                    let co_strength = (str_a * str_b).sqrt();
                    discoveries.push(RuminationDiscovery {
                        origin_hex: origin_hex.clone(),
                        origin_label: origin_label.clone(),
                        node_a_hex: hex_a.clone(),
                        node_a_label: label_a.clone(),
                        node_b_hex: hex_b.clone(),
                        node_b_label: label_b.clone(),
                        co_activation_strength: co_strength,
                    });
                }
            }
        }

        // Sort by co-activation strength, strongest first
        discoveries.sort_by(|a, b| {
            b.co_activation_strength.partial_cmp(&a.co_activation_strength)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Step 4: Create weak bonds for the top discoveries.
        // Limit to 2 bonds per cycle to avoid flooding the graph.
        let mut created_this_cycle: u64 = 0;
        let top = discoveries.iter().take(2).cloned().collect::<Vec<_>>();
        {
            let mut brain = brain.lock().unwrap();
            for disc in &top {
                if brain.create_rumination_bond(
                    &disc.node_a_hex,
                    &disc.node_b_hex,
                    0.2, // start very weak — strengthen through use
                ) {
                    created_this_cycle += 1;
                }
            }
        }

        // Step 5: Update the report
        {
            let mut r = report.lock().unwrap();
            r.cycles = cycle;
            r.bonds_created += created_this_cycle;
            r.skipped += (discoveries.len() as u64).saturating_sub(created_this_cycle);
            // Keep last 20 discoveries
            for disc in top {
                r.recent_discoveries.push(disc);
                if r.recent_discoveries.len() > 20 {
                    r.recent_discoveries.remove(0);
                }
            }
        }
    }
}
