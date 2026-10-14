// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Lu Brain v2 — desktop terminal shell (Phase 5 enhanced REPL).
//!
//! Commands:
//!   :teach <text>        — ingest text into the knowledge graph
//!   :ask <query>         — simple single-pass recall query
//!   :think <query>       — complex query through the thinking loop
//!   :status              — brain stats (nodes, bonds, episodes, style vector)
//!   :save [<path>]       — save brain to file (path optional)
//!   :load <path>         — load brain from file
//!   :tier <level>        — set autonomy tier
//!   :scar <topic> <tag>  — implant an emotional scar (label then tag as args)
//!   :consolidate         — force a hippocampus consolidation pass
//!   :dictionary          — show dynamic dictionary top terms
//!   :help                — list all commands
//!   :quit / :exit        — save and exit
//!   <anything else>      — respond (full pipeline with episodic memory,
//!                          thinking loop for complex queries)

use brain::Brain;
use std::io::{BufRead, Write};

fn main() {
    println!("╔══════════════════════════════════════╗");
    println!("║      Lu Brain v2.0 — Terminal        ║");
    println!("╚══════════════════════════════════════╝");

    let storage = std::path::PathBuf::from("lu_brain_v2.lbr");

    let mut brain = if storage.exists() {
        match Brain::load(&storage) {
            Ok(b) => {
                let stats = b.stats();
                println!("Loaded brain: {} nodes, {} bonds, {} episodes",
                    stats.total_nodes, stats.total_bonds,
                    b.hippocampus.episodes.len());
                b
            }
            Err(e) => {
                eprintln!("Failed to load brain ({}), starting fresh.", e);
                Brain::new()
            }
        }
    } else {
        println!("No brain file found — starting fresh.");
        Brain::new()
    };

    brain.set_storage(storage);

    let stdin  = std::io::stdin();
    let stdout = std::io::stdout();

    loop {
        print!("lu> ");
        stdout.lock().flush().unwrap();

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => break,
            Err(e) => { eprintln!("Input error: {}", e); break; }
            Ok(_) => {}
        }

        let input = line.trim();
        if input.is_empty() { continue; }

        match input {
            // ── Lifecycle ─────────────────────────────────────────────────────
            ":quit" | ":exit" | "quit" | "exit" => {
                println!("Saving and exiting…");
                if let Err(e) = brain.save() {
                    eprintln!("Save error: {}", e);
                }
                break;
            }

            ":help" => {
                println!("─── Commands ─────────────────────────────────────────");
                println!("  :teach <text>          ingest text into the knowledge graph");
                println!("  :ask <query>           simple single-pass recall");
                println!("  :think <query>         complex query through thinking loop");
                println!("  :status                brain stats (nodes/bonds/episodes/style)");
                println!("  :save [<path>]         save brain to file");
                println!("  :load <path>           load brain from file");
                println!("  :tier <level>          set autonomy tier");
                println!("                         (supervised|assisted|collaborative|autonomous)");
                println!("  :scar <topic> <tag>    implant an emotional scar");
                println!("  :consolidate           force hippocampus consolidation");
                println!("  :dictionary            show dynamic dictionary top terms");
                println!("  :help                  this help");
                println!("  :quit / :exit          save and exit");
                println!("  <text>                 respond (full pipeline)");
            }

            // ── Status ────────────────────────────────────────────────────────
            ":status" => {
                let stats  = brain.stats();
                let health = brain.health();
                let style  = &brain.hippocampus.style;
                println!("── Brain Status ──────────────────────────────────────");
                println!("  Nodes (total/active): {}/{}", stats.total_nodes, stats.active_nodes);
                println!("  Bonds (total/active): {}/{}", stats.total_bonds, stats.active_bonds);
                println!("  Episodes:             {}", brain.hippocampus.episodes.len());
                println!("  Scars:                {}", health.scar_count);
                println!("  Autonomy tier:        {}", health.autonomy_tier.label());
                println!("  Style vector:");
                println!("    formality    = {:.4}  (samples: {})", style.formality, style.sample_count);
                println!("    verbosity    = {:.4}", style.verbosity);
                println!("    technicality = {:.4}", style.technicality);
                println!("    empathy      = {:.4}", style.empathy);
                if !health.warnings.is_empty() {
                    println!("  Warnings:");
                    for w in &health.warnings {
                        println!("    * {}", w);
                    }
                }
            }

            // ── Consolidate ───────────────────────────────────────────────────
            ":consolidate" => {
                let report = brain.consolidate();
                println!(
                    "Consolidation done. Episodes processed: {}, bonds strengthened: {}.",
                    report.episodes_processed, report.bonds_strengthened
                );
            }

            // ── Dictionary ────────────────────────────────────────────────────
            ":dictionary" => {
                let dict = brain.build_dictionary();
                let terms = dict.plaintext_terms();
                println!("Dynamic dictionary: {} entries (generation {})",
                    dict.entry_count(), dict.generation());
                let preview: Vec<&str> = terms.iter().take(40).map(|s| s.as_str()).collect();
                if preview.is_empty() {
                    println!("  (no entries yet — teach Lu some text first)");
                } else {
                    println!("  {}", preview.join("  "));
                }
            }

            // ── Parameterised commands ────────────────────────────────────────
            s if s.starts_with(":teach ") => {
                let text = s[7..].trim();
                if text.is_empty() {
                    eprintln!("Usage: :teach <text>");
                } else {
                    let seeds = brain.learn_text(text, graph::node::Confidence::Terminal);
                    println!("Learned {} knowledge unit(s).", seeds.len());
                    for seed in &seeds {
                        println!("  • {}", seed.short());
                    }
                }
            }

            s if s.starts_with(":ask ") => {
                let query = s[5..].trim();
                if query.is_empty() {
                    eprintln!("Usage: :ask <query>");
                } else {
                    let recall = brain.recall(query, 3, 10);
                    if recall.activated_nodes.is_empty() {
                        println!("(no results for '{}')", query);
                    } else {
                        println!();
                        for node in recall.activated_nodes.iter().take(5) {
                            println!("  [{:.2}] {}", node.strength, node.knowledge_unit.raw_text());
                        }
                        println!();
                    }
                }
            }

            s if s.starts_with(":think ") => {
                let query = s[7..].trim();
                if query.is_empty() {
                    eprintln!("Usage: :think <query>");
                } else {
                    println!();
                    let answer = brain.think_through(query);
                    println!("{}", answer);
                    println!();
                }
            }

            s if s.starts_with(":save") => {
                let rest = s[5..].trim();
                if !rest.is_empty() {
                    let p = std::path::PathBuf::from(rest);
                    brain.set_storage(p);
                }
                match brain.save() {
                    Ok(()) => {
                        let stats = brain.stats();
                        println!("Saved. ({} nodes, {} bonds)", stats.total_nodes, stats.total_bonds);
                    }
                    Err(e) => eprintln!("Save error: {}", e),
                }
            }

            s if s.starts_with(":load ") => {
                let path_str = s[6..].trim();
                if path_str.is_empty() {
                    eprintln!("Usage: :load <path>");
                } else {
                    let p = std::path::PathBuf::from(path_str);
                    match Brain::load(&p) {
                        Ok(b) => {
                            let stats = b.stats();
                            println!("Loaded: {} nodes, {} bonds, {} episodes",
                                stats.total_nodes, stats.total_bonds,
                                b.hippocampus.episodes.len());
                            brain = b;
                            brain.set_storage(p);
                        }
                        Err(e) => eprintln!("Load error: {}", e),
                    }
                }
            }

            s if s.starts_with(":tier ") => {
                let tier_str = s[6..].trim();
                let tier = match tier_str {
                    "supervised"    => Some(brain::hypothalamus::AutonomyTier::Supervised),
                    "assisted"      => Some(brain::hypothalamus::AutonomyTier::Assisted),
                    "collaborative" => Some(brain::hypothalamus::AutonomyTier::Collaborative),
                    "autonomous"    => Some(brain::hypothalamus::AutonomyTier::Autonomous),
                    other => {
                        eprintln!("Unknown tier '{}'. Options: supervised|assisted|collaborative|autonomous", other);
                        None
                    }
                };
                if let Some(t) = tier {
                    brain.hypothalamus.set_autonomy_tier(t);
                    println!("Autonomy tier set to: {}", brain.hypothalamus.autonomy_tier.label());
                }
            }

            s if s.starts_with(":scar ") => {
                // :scar <topic> <tag>  — positional: first word is label, rest is reason
                let rest = s[6..].trim();
                let mut parts = rest.splitn(2, ' ');
                let label  = parts.next().unwrap_or("").trim();
                let reason = parts.next().unwrap_or("user-implanted").trim();
                if label.is_empty() {
                    eprintln!("Usage: :scar <topic> [<reason>]");
                } else {
                    brain.implant_scar(label, reason, vec![]);
                    println!("Scar implanted: '{}' (reason: {})", label, reason);
                }
            }

            // ── Default — full respond pipeline ───────────────────────────────
            query => {
                let response = brain.respond(query);
                println!();
                println!("{}", response);
                println!();
            }
        }
    }
}
