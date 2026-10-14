// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! clean_and_bond — cleans garbage nodes and creates subject-overlap bonds
//! in the migrated v2 brain at %APPDATA%/lu-desktop/brain.json.
//!
//! Steps:
//!   1. Load brain from %APPDATA%/lu-desktop/brain.json
//!   2. Remove garbage nodes:
//!      - Blank/empty-subject Contrast nodes ("vs (general)" pattern)
//!      - Nodes whose content is a data URI or contains a long base64 blob
//!      - Nodes with content < 10 chars
//!      - Nodes whose content is purely numeric (^\d+[\.\d]*$)
//!      - Duplicate-label nodes (keep longest content, revoke rest)
//!   3. Create Context bonds between nodes sharing subjects:
//!      - Exact subject match → weight 0.8
//!      - Substring overlap (same first word) → weight 0.5
//!      - Cap: max 10 bonds per node
//!   4. Rebuild keyword index and save
//!   5. Print stats + sample bonds

use brain::Brain;
use df_core::{Seed, SeedGated};
use graph::bond::BondType;
use graph::knowledge_unit::KnowledgeUnit;
use std::collections::HashMap;
use std::path::PathBuf;

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    println!("=== Lu Brain v2 — Clean & Bond ===\n");

    let brain_path = get_brain_path();
    println!("Brain: {}", brain_path.display());

    if !brain_path.exists() {
        eprintln!("Brain file not found: {}", brain_path.display());
        std::process::exit(1);
    }

    // ── Load ──────────────────────────────────────────────────────────────────
    println!("Loading brain (this may take a moment for large files)...");
    let mut brain = match Brain::load(&brain_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("FAILED to load brain: {}", e);
            std::process::exit(1);
        }
    };
    brain.set_storage(brain_path.clone());

    let initial_stats = brain.network.stats();
    println!(
        "Loaded: {} active nodes, {} revoked, {} total bonds\n",
        initial_stats.active_nodes, initial_stats.revoked_nodes, initial_stats.total_bonds
    );

    // ── Step 1: Garbage removal ───────────────────────────────────────────────
    println!("=== STEP 1: Garbage removal ===");
    let garbage = collect_garbage_seeds(&brain);

    let (mut n_contrast, mut n_base64, mut n_short, mut n_numeric, mut n_dup) =
        (0usize, 0usize, 0usize, 0usize, 0usize);

    for (seed, reason) in &garbage {
        if reason.starts_with("blank_contrast") {
            n_contrast += 1;
        } else if reason.starts_with("base64") {
            n_base64 += 1;
        } else if reason.starts_with("short") {
            n_short += 1;
        } else if reason.starts_with("numeric") {
            n_numeric += 1;
        } else if reason.starts_with("duplicate") {
            n_dup += 1;
        }
        brain.revoke(seed, reason);
    }

    println!("  Blank/empty Contrast  : {}", n_contrast);
    println!("  Base64 / data-URI     : {}", n_base64);
    println!("  Short content (<10)   : {}", n_short);
    println!("  Numeric-only          : {}", n_numeric);
    println!("  Duplicate labels      : {}", n_dup);
    println!("  ─────────────────────────────────");
    println!("  Total removed         : {}", garbage.len());
    let after_cleanup = brain.network.stats();
    println!("  Active nodes remaining: {}\n", after_cleanup.active_nodes);

    // ── Step 2: Subject-overlap bonding ──────────────────────────────────────
    println!("=== STEP 2: Subject-overlap bonding ===");
    let bond_stats = create_subject_bonds(&mut brain);

    println!("  Exact-subject bonds   : {}", bond_stats.exact_bonds);
    println!("  Substring-overlap     : {}", bond_stats.substring_bonds);
    println!("  ─────────────────────────────────");
    println!("  Total bonds created   : {}", bond_stats.total);
    println!("  Avg bonds / node      : {:.2}",
        if bond_stats.nodes_bonded > 0 {
            (bond_stats.total * 2) as f64 / bond_stats.nodes_bonded as f64
        } else { 0.0 }
    );
    println!("  Nodes bonded          : {}", bond_stats.nodes_bonded);

    if !bond_stats.top_subjects.is_empty() {
        println!("\n  Top 10 most-connected subjects:");
        for (subj, count) in &bond_stats.top_subjects {
            println!("    {:4} bonds — {}", count, subj);
        }
    }
    println!();

    // ── Step 3: Rebuild index + save ─────────────────────────────────────────
    println!("=== STEP 3: Rebuild index + save ===");
    brain.network.rebuild_keyword_index();
    println!("  Keyword index rebuilt.");

    match brain.save() {
        Ok(_) => println!("  Saved: {}", brain_path.display()),
        Err(e) => {
            eprintln!("  SAVE FAILED: {}", e);
            std::process::exit(1);
        }
    }

    // ── Step 4: Verification ─────────────────────────────────────────────────
    println!("\n=== STEP 4: Verification ===");
    let final_stats = brain.network.stats();
    let file_size = std::fs::metadata(&brain_path)
        .map(|m| m.len())
        .unwrap_or(0);

    println!("  Nodes before cleanup  : {}", initial_stats.active_nodes);
    println!("  Nodes after cleanup   : {}", final_stats.active_nodes);
    println!(
        "  Nodes removed         : {}",
        initial_stats.active_nodes.saturating_sub(final_stats.active_nodes)
    );
    println!("  Active bonds total    : {}", final_stats.active_bonds);
    println!("  Bonds created         : {}", bond_stats.total);
    println!("  File size             : {:.2} MB", file_size as f64 / 1_048_576.0);

    // Sample 5 random active bonds
    let sample: Vec<_> = brain
        .network
        .all_bonds()
        .into_iter()
        .filter(|b| b.active)
        .take(5)
        .collect();

    if !sample.is_empty() {
        println!("\n  Sample bonds:");
        for bond in &sample {
            let src = brain
                .network
                .get_node(&bond.source_seed)
                .map(|n| n.label())
                .unwrap_or_else(|| "?".to_string());
            let tgt = brain
                .network
                .get_node(&bond.target_seed)
                .map(|n| n.label())
                .unwrap_or_else(|| "?".to_string());
            let src_short: String = src.chars().take(45).collect();
            let tgt_short: String = tgt.chars().take(45).collect();
            println!(
                "    [{:?}] \"{}\" → \"{}\" (w={:.2})",
                bond.bond_type, src_short, tgt_short, bond.weight
            );
        }
    }

    println!("\nDone.\n");
}

// ── Brain path ────────────────────────────────────────────────────────────────

fn get_brain_path() -> PathBuf {
    if let Some(arg) = std::env::args().nth(1) {
        return PathBuf::from(arg);
    }
    let appdata = std::env::var("APPDATA")
        .unwrap_or_else(|_| std::env::var("HOME").unwrap_or_else(|_| ".".to_string()));
    PathBuf::from(appdata).join("lu-desktop").join("brain.json")
}

// ── Garbage-detection helpers ─────────────────────────────────────────────────

/// Returns true if this is a blank-subject or generic Contrast node.
/// These are the "vs (general)" garbage nodes produced by bad migration.
fn is_blank_contrast(ku: &KnowledgeUnit) -> bool {
    if let KnowledgeUnit::Contrast {
        subject_a,
        subject_b,
        dimension,
        ..
    } = ku
    {
        subject_a.trim().is_empty()
            || subject_b.trim().is_empty()
            || dimension.trim().to_lowercase() == "general"
            || (subject_a.trim().len() < 2 && subject_b.trim().len() < 2)
    } else {
        false
    }
}

/// Returns true if `s` contains more than 500 consecutive base64-alphabet bytes.
/// Catches embedded base64 SVG/image blobs.
fn has_large_base64_blob(s: &str) -> bool {
    let mut run = 0usize;
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=' {
            run += 1;
            if run > 500 {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

/// Returns true if `s` matches `^\d+[\.\d]*$` (purely numeric).
fn is_numeric_only(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() {
        return false;
    }
    let mut chars = t.chars();
    match chars.next() {
        Some(c) if c.is_ascii_digit() => chars.all(|c| c.is_ascii_digit() || c == '.'),
        _ => false,
    }
}

// ── Garbage collection ────────────────────────────────────────────────────────

/// Collects (seed, reason) pairs for all garbage nodes.
///
/// Rules applied in priority order:
///   1. Blank-subject Contrast
///   2. Base64 / data-URI content
///   3. Content shorter than 10 bytes
///   4. Purely numeric content
///   5. Duplicate label (keep node with longest raw content)
fn collect_garbage_seeds(brain: &Brain) -> Vec<(Seed, String)> {
    let mut to_revoke: Vec<(Seed, String)> = Vec::new();
    // Tracks seeds already queued for revocation (to avoid double-adding duplicates).
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    // For duplicate detection: normalised_label → (seed, content_len)
    let mut label_best: HashMap<String, (Seed, usize)> = HashMap::new();
    let mut dup_losers: Vec<(Seed, String)> = Vec::new();

    for node in brain.network.active_nodes() {
        let raw = node.knowledge_unit.raw_text();
        let seed = node.seed().clone();
        let hex = seed.to_hex();

        // 1. Blank Contrast
        if is_blank_contrast(&node.knowledge_unit) {
            seen.insert(hex);
            to_revoke.push((seed, "blank_contrast: empty or generic Contrast node".to_string()));
            continue;
        }

        // 2. Base64 / data-URI
        if raw.starts_with("data:") || has_large_base64_blob(raw) {
            seen.insert(hex);
            to_revoke.push((seed, "base64: data URI or embedded base64 blob".to_string()));
            continue;
        }

        // 3. Short content
        if raw.len() < 10 {
            seen.insert(hex);
            to_revoke.push((seed, format!("short: only {} bytes", raw.len())));
            continue;
        }

        // 4. Numeric-only
        if is_numeric_only(raw) {
            seen.insert(hex);
            to_revoke.push((seed, "numeric: purely numeric content".to_string()));
            continue;
        }

        // 5. Duplicate-label detection
        // Normalise label to avoid case/whitespace false negatives.
        let norm_label = node.knowledge_unit.label().to_lowercase();
        let content_len = raw.len();

        // Separate the borrow on label_best from the later insert by cloning first.
        let existing = label_best.get(&norm_label).map(|(s, l)| (s.clone(), *l));
        match existing {
            None => {
                label_best.insert(norm_label, (seed, content_len));
            }
            Some((existing_seed, existing_len)) => {
                if content_len > existing_len {
                    // Current node is richer — demote the previous winner.
                    dup_losers.push((
                        existing_seed,
                        "duplicate: superseded by a longer node with same label".to_string(),
                    ));
                    label_best.insert(norm_label, (seed, content_len));
                } else {
                    // Existing node is richer — demote the current one.
                    dup_losers.push((
                        seed,
                        "duplicate: shorter than existing node with same label".to_string(),
                    ));
                }
            }
        }
    }

    // Flush duplicate losers (skip any already queued under a primary garbage reason).
    for (seed, reason) in dup_losers {
        let hex = seed.to_hex();
        if !seen.contains(&hex) {
            seen.insert(hex);
            to_revoke.push((seed, reason));
        }
    }

    to_revoke
}

// ── Subject-overlap bonding ───────────────────────────────────────────────────

struct BondingStats {
    exact_bonds: usize,
    substring_bonds: usize,
    total: usize,
    nodes_bonded: usize,
    top_subjects: Vec<(String, usize)>,
}

/// Creates Context bonds between active nodes whose subjects overlap.
///
/// Phase A — exact subject match (weight 0.8):
///   Nodes that share the same normalised subject string are bonded pairwise.
///
/// Phase B — substring overlap (weight 0.5):
///   Nodes grouped by their first subject word.  Within each first-word bucket,
///   pairs whose subjects contain each other as substrings receive a weaker bond.
///
/// Both phases cap at 10 bonds per node to prevent hub explosion.
fn create_subject_bonds(brain: &mut Brain) -> BondingStats {
    // Collect owned (Seed, normalised_subject) before any mutable borrow.
    let node_subjects: Vec<(Seed, String)> = brain
        .network
        .active_nodes()
        .into_iter()
        .filter_map(|node| {
            let subj = node.knowledge_unit.subject()?;
            let norm = subj.trim().to_lowercase();
            if norm.len() >= 3 {
                Some((node.seed().clone(), norm))
            } else {
                None
            }
        })
        .collect();

    // Build subject → Vec<Seed> index.
    let mut subject_map: HashMap<String, Vec<Seed>> = HashMap::new();
    for (seed, subj) in &node_subjects {
        subject_map.entry(subj.clone()).or_default().push(seed.clone());
    }

    // Per-node bond counter (keyed by hex string).
    let mut bonds_per_node: HashMap<String, usize> = HashMap::new();
    let mut exact_bonds = 0usize;
    let mut substring_bonds = 0usize;
    let mut bonded_set: std::collections::HashSet<String> = std::collections::HashSet::new();

    // ── Phase A: exact subject bonds (w = 0.8) ────────────────────────────────
    // We must collect the seeds into an owned structure first so the borrow on
    // subject_map doesn't conflict with the mutable borrow on brain below.
    let exact_groups: Vec<Vec<Seed>> = subject_map
        .values()
        .filter(|v| v.len() >= 2)
        .cloned()
        .collect();

    for seeds in &exact_groups {
        for i in 0..seeds.len() {
            let ha = seeds[i].to_hex();
            if *bonds_per_node.get(&ha).unwrap_or(&0) >= 10 {
                continue;
            }
            for j in (i + 1)..seeds.len() {
                let hb = seeds[j].to_hex();
                if *bonds_per_node.get(&ha).unwrap_or(&0) >= 10 {
                    break; // this seed_i is full — move to next i
                }
                if *bonds_per_node.get(&hb).unwrap_or(&0) >= 10 {
                    continue;
                }
                if brain
                    .associate(&seeds[i], &seeds[j], BondType::Context, 0.8)
                    .is_ok()
                {
                    *bonds_per_node.entry(ha.clone()).or_insert(0) += 1;
                    *bonds_per_node.entry(hb.clone()).or_insert(0) += 1;
                    bonded_set.insert(ha.clone());
                    bonded_set.insert(hb);
                    exact_bonds += 1;
                }
            }
        }
    }

    // ── Phase B: substring-overlap bonds (w = 0.5) ───────────────────────────
    // Group distinct subjects by first word to limit pair comparisons.
    let mut first_word_groups: HashMap<String, Vec<(String, Vec<Seed>)>> = HashMap::new();
    for (subj, seeds) in &subject_map {
        if let Some(fw) = subj.split_whitespace().next() {
            if fw.len() >= 3 {
                first_word_groups
                    .entry(fw.to_string())
                    .or_default()
                    .push((subj.clone(), seeds.clone()));
            }
        }
    }

    for group in first_word_groups.values() {
        if group.len() < 2 {
            continue;
        }
        for i in 0..group.len() {
            for j in (i + 1)..group.len() {
                let (subj_a, seeds_a) = &group[i];
                let (subj_b, seeds_b) = &group[j];

                // Exact-match pairs were already handled in Phase A.
                if subj_a == subj_b {
                    continue;
                }

                // Require one subject to contain the other as a substring.
                let overlap = subj_a.contains(subj_b.as_str())
                    || subj_b.contains(subj_a.as_str());
                if !overlap {
                    continue;
                }

                // Bond up to 5 representatives from each side.
                'outer: for seed_a in seeds_a.iter().take(5) {
                    let ha = seed_a.to_hex();
                    if *bonds_per_node.get(&ha).unwrap_or(&0) >= 10 {
                        continue 'outer; // seed_a's cap reached
                    }
                    for seed_b in seeds_b.iter().take(5) {
                        let hb = seed_b.to_hex();
                        if ha == hb {
                            continue;
                        }
                        if *bonds_per_node.get(&ha).unwrap_or(&0) >= 10 {
                            continue 'outer;
                        }
                        if *bonds_per_node.get(&hb).unwrap_or(&0) >= 10 {
                            continue;
                        }
                        if brain
                            .associate(seed_a, seed_b, BondType::Context, 0.5)
                            .is_ok()
                        {
                            *bonds_per_node.entry(ha.clone()).or_insert(0) += 1;
                            *bonds_per_node.entry(hb.clone()).or_insert(0) += 1;
                            bonded_set.insert(ha.clone());
                            bonded_set.insert(hb);
                            substring_bonds += 1;
                        }
                    }
                }
            }
        }
    }

    // ── Top 10 subjects by total bond count of member nodes ──────────────────
    let mut subject_scores: Vec<(String, usize)> = subject_map
        .iter()
        .map(|(subj, seeds)| {
            let total: usize = seeds
                .iter()
                .map(|s| *bonds_per_node.get(&s.to_hex()).unwrap_or(&0))
                .sum();
            (subj.clone(), total)
        })
        .filter(|(_, c)| *c > 0)
        .collect();
    subject_scores.sort_by(|a, b| b.1.cmp(&a.1));
    subject_scores.truncate(10);

    BondingStats {
        exact_bonds,
        substring_bonds,
        total: exact_bonds + substring_bonds,
        nodes_bonded: bonded_set.len(),
        top_subjects: subject_scores,
    }
}
