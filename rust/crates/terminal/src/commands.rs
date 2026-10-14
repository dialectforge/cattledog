// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Command Processing
//!
//! Parses user input and dispatches to brain operations.
//! All commands flow through the terminal bond.

use brain::Brain;
use brain::coordinator;
use df_core::Seed;

/// Process a user command and return a response.
pub fn process(brain: &mut Brain, input: &str) -> String {
    let input = input.trim();
    if input.is_empty() {
        return String::new();
    }

    // Slash = command, everything else = conversation.
    // The terminal is a place to TALK to the brain.
    // Commands are the exception, not the rule.
    if input.starts_with('/') {
        let without_slash = &input[1..];
        let (cmd, arg) = match without_slash.split_once(' ') {
            Some((c, a)) => (c.to_lowercase(), a.to_string()),
            None => (without_slash.to_lowercase(), String::new()),
        };

        match cmd.as_str() {
            "learn" => cmd_learn(brain, &arg),
            "ask" | "recall" => cmd_ask(brain, &arg),
            "ingest" => cmd_ingest(brain, &arg),
            "ingest_file" | "if" => cmd_ingest_file(brain, &arg),
            "approve_all" | "aa" => cmd_approve_all(brain, &arg),
            "associate" => cmd_associate(brain, &arg),
            "revoke" => cmd_revoke(brain, &arg),
            "seed_report" | "sr" => cmd_seed_report(brain),
            "propose" => cmd_propose(brain, &arg),
            "approve" => cmd_approve(brain, &arg),
            "reject" => cmd_reject(brain, &arg),
            "talk" => cmd_talk(brain, &arg),
            "respond" => cmd_respond(brain, &arg),
            "reason" => cmd_reason(brain, &arg),
            "reflect" => cmd_reflect(brain),
            "resize" => cmd_resize(brain, &arg),
            "stats" => cmd_stats(brain),
            "scars" => cmd_scars(brain),
            "save" => cmd_save(brain),
            "think" => cmd_think(brain, &arg),
            "think_step" | "ts" => cmd_think_step(brain),
            "think_approve" | "ta" => cmd_think_approve(brain, &arg),
            "think_reject" | "tr" => cmd_think_reject(brain, &arg),
            "think_simulate" | "tsim" => cmd_think_simulate(brain, &arg),
            "think_status" => cmd_think_status(brain),
            "think_stop" => cmd_think_stop(brain),
            "session" => cmd_session(brain),
            "session_promote" | "sp" => cmd_session_promote(brain, &arg),
            "session_clear" | "sc" => cmd_session_clear(brain),
            "media_search" | "ms" => cmd_media_search(brain, &arg),
            "autonomy" => cmd_autonomy(brain),
            "elevate" => cmd_elevate(brain, &arg),
            "execute" => cmd_execute(brain),
            "curious" => cmd_curious(brain),
            "wonder" | "wq" => cmd_wonder(brain, &arg),
            "interests" => cmd_interests(brain),
            "improve" | "si" => cmd_self_improvement(brain),
            "meta_reflect" | "mr" => cmd_meta_reflect(brain),
            "bridges" | "invention" => cmd_bridges(brain),
            "health" => cmd_health(brain),
            "cleanup" | "glossary_cleanup" => cmd_cleanup_glossary(brain),
            "synthesis" | "syn" => cmd_synthesis(brain),
            "variants" | "var" => cmd_variants(brain, &arg),
            "variants_status" | "vs" => cmd_variants_status(brain),
            "variants_promote" | "vp" => cmd_variants_promote(brain, &arg),
            "tools" => cmd_tool_status(brain),
            "tool_detect" | "td" => cmd_tool_detect(brain, &arg),
            "tool_approve" | "tap" => cmd_tool_approve(brain, &arg),
            "tool_reject" | "trj" => cmd_tool_reject(brain, &arg),
            "tool_pending" | "tp" => cmd_tool_pending(brain),
            "tool_ready" | "tex" => cmd_tool_ready(brain),
            "help" => cmd_help(),
            _ => format!("Unknown command: /{}. Type /help for commands.", cmd),
        }
    } else {
        // No slash = orchestrator. Smart routing to the right subsystem.
        cmd_orchestrate(brain, input)
    }
}

fn cmd_learn(brain: &mut Brain, content: &str) -> String {
    if content.is_empty() {
        return "Usage: learn <content>".into();
    }
    let seed = brain.learn(content, graph::Confidence::Terminal, "terminal");
    format!("Learned. Seed: {}", seed)
}

fn cmd_ask(brain: &mut Brain, query: &str) -> String {
    if query.is_empty() {
        return "Usage: ask <query>".into();
    }
    let result = brain.recall(query, 3, 10);
    let mut lines = vec![format!("Query: {}\n", query)];

    if !result.scar_warnings.is_empty() {
        lines.push("⚠ SCAR WARNINGS:".into());
        for w in &result.scar_warnings {
            lines.push(format!("  ! {}", w));
        }
        lines.push(String::new());
    }

    if result.activated_nodes.is_empty() {
        lines.push("No knowledge activated. The brain is empty on this topic.".into());
    } else {
        lines.push("Activated knowledge:".into());
        for node in &result.activated_nodes {
            lines.push(format!(
                "  [{}] {:.1}% | {}",
                &node.seed_hex[..12],
                node.contribution * 100.0,
                node.label
            ));
        }
    }

    if !result.traversal_path.is_empty() {
        lines.push(String::new());
        lines.push("Traversal path:".into());
        for hop in result.traversal_path.iter().take(10) {
            lines.push(format!(
                "  {} --({:.2})--> {} (hop {})",
                &hop.from[..8.min(hop.from.len())],
                hop.strength,
                &hop.to[..8.min(hop.to.len())],
                hop.hop
            ));
        }
    }

    lines.join("\n")
}

/// Conversational response — Left Brain narration pipeline.
/// Uses brain.ask() which runs: recall → classify_query → narration engine.
fn cmd_talk(brain: &mut Brain, query: &str) -> String {
    if query.is_empty() {
        return "Say something! I'll tell you what I know.".into();
    }
    let response = brain.ask(query, 3, 10);
    let _ = brain.save();

    let mut lines = Vec::new();
    lines.push(response.text);

    // Show sources in compact form
    if !response.sources.is_empty() {
        lines.push(String::new());
        lines.push(format!("  ({} facts, {:.0}% coherence)",
            response.atoms_used,
            response.coherence_score * 100.0));
    }
    if !response.scar_warnings.is_empty() {
        for w in &response.scar_warnings {
            lines.push(format!("  ⚠ {}", w));
        }
    }
    // Show image URLs when Lu has visual knowledge
    if !response.image_urls.is_empty() {
        lines.push(String::new());
        for (url, desc) in &response.image_urls {
            lines.push(format!("  🖼 {} — {}", desc, url));
        }
    }
    lines.join("\n")
}

fn cmd_reason(brain: &mut Brain, query: &str) -> String {
    if query.is_empty() {
        return "Usage: reason <query>".into();
    }
    let result = brain.reason(query, 3);
    let mut lines = vec![format!("Query: {}\n", query)];

    lines.push(format!("Direct answer: {}", result.direct_answer));
    lines.push(String::new());

    if result.chains.is_empty() {
        lines.push("No reasoning chains found (need more connected knowledge).".into());
    } else {
        lines.push(format!("Reasoning chains ({}):", result.chains.len()));
        for (i, chain) in result.chains.iter().enumerate() {
            lines.push(format!("  Chain {} (confidence: {:.1}%):", i + 1, chain.confidence));
            for (j, step) in chain.steps.iter().enumerate() {
                let prefix = if j == 0 { "    >" } else { "    →" };
                lines.push(format!("{} {}", prefix, step.label));
            }
            lines.push(format!("    Synthesis: {}", chain.synthesis));
            lines.push(String::new());
        }
    }

    lines.push(format!("Conclusion: {}", result.conclusion));
    lines.join("\n")
}

fn cmd_respond(brain: &mut Brain, query: &str) -> String {
    if query.is_empty() {
        return "Ask me something.".into();
    }
    let response = brain.respond(query, 3, 10);
    let mut lines = vec![response.text.clone()];

    // Show metadata in compact form
    lines.push(String::new());
    lines.push(format!(
        "[{} atoms | coherence {:.0}% | {} hops]",
        response.atoms_used,
        response.coherence_score * 100.0,
        response.traversal_hops,
    ));

    if !response.scar_warnings.is_empty() {
        lines.push(String::new());
        for w in &response.scar_warnings {
            lines.push(format!("! SCAR: {}", w));
        }
    }

    lines.join("\n")
}

fn cmd_ingest(brain: &mut Brain, text: &str) -> String {
    if text.is_empty() {
        return "Usage: ingest <text to ingest>".into();
    }
    let report = brain.ingest_text(text, "terminal");
    let mut lines = vec![
        format!("INGESTION REPORT"),
        format!("Sentences: {} | Ready: {} | Blocked: {} | Duplicates: {}",
            report.total_sentences, report.proposals_ready,
            report.proposals_blocked, report.duplicates_skipped),
    ];

    if report.proposals.is_empty() {
        lines.push("No proposals generated.".into());
    } else {
        lines.push(String::new());
        let mut ready_ids = Vec::new();
        for p in &report.proposals {
            let status_marker = if p.status == "READY" { "+" } else { "!" };
            lines.push(format!("  [{}] {} {}", status_marker, &p.id[..8], &p.content[..p.content.len().min(70)]));
            if p.status == "READY" {
                ready_ids.push(p.id.clone());
            }
        }
        if !ready_ids.is_empty() {
            lines.push(String::new());
            lines.push(format!("Type 'approve_all {}' to learn all READY proposals.",
                ready_ids.iter().map(|id| &id[..8]).collect::<Vec<_>>().join(" ")));
            // Store the full IDs for approve_all
            lines.push(format!("[ids:{}]", ready_ids.join(",")));
        }
    }
    lines.join("\n")
}

fn cmd_ingest_file(brain: &mut Brain, path: &str) -> String {
    let path = path.trim();
    if path.is_empty() {
        return "Usage: ingest_file <path>  (supports .txt, .md, .pdf)".into();
    }
    match brain.ingest_file(path) {
        Ok(report) => {
            let mut lines = vec![
                format!("INGESTION: {}", report.source),
                format!("Sentences: {} | Ready: {} | Blocked: {} | Duplicates: {}",
                    report.total_sentences, report.proposals_ready,
                    report.proposals_blocked, report.duplicates_skipped),
            ];
            if !report.proposals.is_empty() {
                lines.push(String::new());
                let mut ready_ids = Vec::new();
                for p in &report.proposals {
                    let status_marker = if p.status == "READY" { "+" } else { "!" };
                    lines.push(format!("  [{}] {} {}",
                        status_marker, &p.id[..8],
                        &p.content[..p.content.len().min(70)]));
                    if p.status == "READY" {
                        ready_ids.push(p.id.clone());
                    }
                }
                if !ready_ids.is_empty() {
                    lines.push(String::new());
                    lines.push(format!("{} proposals ready. Type 'approve_all' to learn them all.", ready_ids.len()));
                    lines.push(format!("[ids:{}]", ready_ids.join(",")));
                }
            }
            lines.join("\n")
        }
        Err(e) => format!("Failed: {}", e),
    }
}

fn cmd_approve_all(brain: &mut Brain, arg: &str) -> String {
    // Parse comma-separated IDs or space-separated short IDs
    let ids: Vec<String> = if arg.contains(',') {
        arg.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    } else {
        arg.split_whitespace().map(|s| s.to_string()).collect()
    };

    if ids.is_empty() {
        return "Usage: approve_all <id1,id2,...> or approve_all <id1> <id2> ...".into();
    }

    let results = brain.approve_all_ready(&ids);
    if results.is_empty() {
        "No proposals approved. IDs may not match pending proposals.".into()
    } else {
        let mut lines = vec![format!("Approved {} proposals:", results.len())];
        for (id, seed) in &results {
            lines.push(format!("  [{}] -> seed {}", &id[..8.min(id.len())], seed));
        }
        lines.join("\n")
    }
}

fn cmd_associate(brain: &mut Brain, arg: &str) -> String {
    let parts: Vec<&str> = arg.splitn(3, ' ').collect();
    if parts.len() < 3 {
        return "Usage: associate <seed_a_hex> <seed_b_hex> <description>".into();
    }
    let seed_a = match Seed::from_hex(parts[0]) {
        Ok(s) => s,
        Err(_) => return format!("Invalid seed: {}", parts[0]),
    };
    let seed_b = match Seed::from_hex(parts[1]) {
        Ok(s) => s,
        Err(_) => return format!("Invalid seed: {}", parts[1]),
    };
    match brain.associate(&seed_a, &seed_b, parts[2], 1.0, "terminal") {
        Ok(seed) => format!("Bond created. Seed: {}", seed),
        Err(e) => e,
    }
}

fn cmd_revoke(brain: &mut Brain, arg: &str) -> String {
    let (hex, reason) = match arg.split_once(' ') {
        Some((h, r)) => (h, r),
        None => return "Usage: revoke <seed_hex> <reason>".into(),
    };
    let seed = if hex.len() < 64 {
        // Short prefix — search for matching node
        match brain.find_seed_by_prefix(hex) {
            Some(s) => s,
            None => return format!("No node found with prefix: {}", hex),
        }
    } else {
        match Seed::from_hex(hex) {
            Ok(s) => s,
            Err(_) => return format!("Invalid seed: {}", hex),
        }
    };
    let report = brain.revoke(&seed, reason);
    let mut lines = vec![
        format!("REVOKED: {}", report.label.unwrap_or_default()),
        format!("Type: {}", report.entity_type.unwrap_or_default()),
        format!("Scar created: {}", report.scar_created),
    ];
    if !report.children_to_review.is_empty() {
        lines.push(String::new());
        lines.push("CHILDREN TO REVIEW:".into());
        for (seed, label) in &report.children_to_review {
            lines.push(format!("  [{}] {}", &seed[..12], label));
        }
    }
    lines.join("\n")
}

fn cmd_seed_report(brain: &Brain) -> String {
    match brain.last_seed_report() {
        Some(result) => {
            let mut lines = vec!["SEED REPORT".into(), "═".repeat(50)];
            for node in &result.activated_nodes {
                lines.push(format!(
                    "  [{}] {:5.1}% | {} (conf={:?})",
                    &node.seed_hex[..12],
                    node.contribution * 100.0,
                    node.label,
                    node.confidence,
                ));
            }
            lines.push("═".repeat(50));
            lines.join("\n")
        }
        None => "No recent recall to report on.".into(),
    }
}

fn cmd_propose(brain: &mut Brain, content: &str) -> String {
    if content.is_empty() {
        return "Usage: propose <content>".into();
    }
    let proposal = brain.propose(content, "external");
    let mut lines = vec![
        format!("LEARNING PROPOSAL [{}]", proposal.id),
        "═".repeat(50),
        format!("Content: {}", &proposal.content[..proposal.content.len().min(100)]),
        format!("Recommendation: {}", proposal.recommendation),
    ];
    if !proposal.scar_conflicts.is_empty() {
        lines.push(String::new());
        lines.push("⚠ SCAR CONFLICTS:".into());
        for sc in &proposal.scar_conflicts {
            lines.push(format!("  ! {}", sc));
        }
    }
    if !proposal.similar_existing.is_empty() {
        lines.push(String::new());
        lines.push("Similar existing knowledge:".into());
        for (seed, label, sim) in &proposal.similar_existing {
            lines.push(format!("  [{}] {} (sim={:.3})", &seed[..12], label, sim));
        }
    }
    lines.push(String::new());
    lines.push(format!("Type 'approve {}' or 'reject {}'", proposal.id, proposal.id));
    lines.join("\n")
}

fn cmd_approve(brain: &mut Brain, id: &str) -> String {
    match brain.approve_proposal(id.trim()) {
        Some(seed) => format!("Approved and learned. Seed: {}", seed),
        None => format!("No pending proposal: {}", id),
    }
}

fn cmd_reject(brain: &mut Brain, id: &str) -> String {
    if brain.reject_proposal(id.trim()) {
        format!("Proposal {} rejected.", id)
    } else {
        format!("No pending proposal: {}", id)
    }
}

fn cmd_reflect(brain: &mut Brain) -> String {
    let report = brain.self_reflect();
    let mut lines = vec!["SELF-REFLECTION REPORT".into(), "═".repeat(50)];
    lines.push(format!("  Bonds strengthened: {}", report.bonds_strengthened));
    lines.push(format!("  Bonds created:      {}", report.bonds_created));
    if !report.strengthened_pairs.is_empty() {
        lines.push(String::new());
        lines.push("  Strengthened:".into());
        for (a, b, w) in &report.strengthened_pairs {
            lines.push(format!("    {} <-> {} (now {:.2})", a, b, w));
        }
    }
    if !report.novel_associations.is_empty() {
        lines.push(String::new());
        lines.push("  Novel associations discovered:".into());
        for (a, b) in &report.novel_associations {
            lines.push(format!("    {} <-> {}", a, b));
        }
    }
    if report.bonds_strengthened == 0 && report.bonds_created == 0 {
        lines.push("  No changes. Run a /ask or talk first to give the brain something to reflect on.".into());
    }
    lines.join("\n")
}

fn cmd_stats(brain: &Brain) -> String {
    let s = brain.stats();
    [
        "BRAIN STATUS",
        &"═".repeat(40),
        &format!("  Active nodes:    {}", s.active_nodes),
        &format!("  Revoked nodes:   {}", s.revoked_nodes),
        &format!("  Scars:           {}", s.scars),
        &format!("  Active bonds:    {}", s.active_bonds),
        &format!("  Dead bonds:      {}", s.dead_bonds),
    ]
    .join("\n")
}

fn cmd_scars(brain: &Brain) -> String {
    let scars = brain.scars();
    if scars.is_empty() {
        return "No scars. No revocations have occurred.".into();
    }
    let mut lines = vec!["REVOCATION SCARS".into(), "═".repeat(50)];
    for scar in scars {
        lines.push(format!(
            "  [{}] {} — reason: {}",
            scar.revoked_seed.short(),
            scar.label,
            scar.reason
        ));
    }
    lines.join("\n")
}

fn cmd_resize(brain: &mut Brain, arg: &str) -> String {
    let new_size: usize = match arg.trim().parse() {
        Ok(n) if n >= 64 && n <= 1024 => n,
        Ok(_) => return "Dimensions must be between 64 and 1024.".into(),
        Err(_) => return "Usage: /resize <dimensions>  (e.g., /resize 256)".into(),
    };
    let result = brain.resize_dimensions(new_size);
    // Auto-save after resize
    match brain.save() {
        Ok(()) => format!("{}\nBrain saved.", result),
        Err(e) => format!("{}\nWARNING: Save failed: {}", result, e),
    }
}

// ━━━ Thinking Layer Commands ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn cmd_think(brain: &mut Brain, goal: &str) -> String {
    if goal.is_empty() {
        return "Usage: /think <goal>\n  Start a deliberate thinking session.\n  Lu will gather, plan, and propose actions for your approval.".into();
    }

    let config = coordinator::ThinkConfig::default();

    match brain.think_start(goal, config) {
        Ok(state) => {
            let mut lines = vec![
                format!("THINKING: {}", state.goal),
                "━".repeat(50),
                format!("  State:      {:?}", state.state),
                format!("  Iteration:  {}", state.iteration),
                format!("  Coherence:  {:.1}%", state.coherence * 100.0),
                format!("  Sub-goals:  {}", state.sub_goal_count),
            ];

            if !state.pending_proposals.is_empty() {
                lines.push(String::new());
                lines.push(format!("PROPOSALS ({}):", state.pending_proposals.len()));
                for p in &state.pending_proposals {
                    let safe_marker = if p.is_safe() { "○" } else { "●" };
                    lines.push(format!("  {} [{}] {:?}: {}",
                        safe_marker, p.id, p.proposal_type, p.description));
                    if !p.scar_conflicts.is_empty() {
                        for sc in &p.scar_conflicts {
                            lines.push(format!("      ⚠ SCAR: {}", sc));
                        }
                    }
                }
                lines.push(String::new());
                lines.push("Use /think_approve <id> or /think_reject <id>".into());
                lines.push("Use /think_step to advance, /think_stop to end.".into());
            }

            lines.join("\n")
        }
        Err(e) => format!("Cannot start thinking: {}", e),
    }
}

fn cmd_think_step(brain: &mut Brain) -> String {
    match brain.think_step() {
        Ok(state) => {
            let mut lines = vec![
                format!("THINKING: {} [iter {}]", state.goal, state.iteration),
                "━".repeat(50),
                format!("  State:      {:?}", state.state),
                format!("  Coherence:  {:.1}%", state.coherence * 100.0),
                format!("  Sub-goals:  {} ({} resolved)",
                    state.sub_goal_count, state.resolved_count),
            ];

            if let Some(ref obs) = state.last_observation {
                lines.push(String::new());
                lines.push(format!("  Last action:  {}", obs.action));
                lines.push(format!("  Outcome:      {}", obs.outcome));
                lines.push(format!("  Δ coherence:  {:+.2}", obs.coherence_delta));
            }

            if !state.pending_proposals.is_empty() {
                lines.push(String::new());
                lines.push(format!("PROPOSALS ({}):", state.pending_proposals.len()));
                for p in &state.pending_proposals {
                    let safe_marker = if p.is_safe() { "○" } else { "●" };
                    lines.push(format!("  {} [{}] {:?}: {}",
                        safe_marker, p.id, p.proposal_type, p.description));
                }
                lines.push(String::new());
                lines.push("Use /think_approve <id> or /think_reject <id>".into());
            }

            if state.state == coordinator::GoalState::Complete {
                lines.push(String::new());
                lines.push("Thinking complete. Use /think_stop for summary.".into());
            }

            lines.join("\n")
        }
        Err(e) => format!("Think step failed: {}", e),
    }
}

fn cmd_think_approve(brain: &mut Brain, id: &str) -> String {
    let id = id.trim();
    if id.is_empty() {
        return "Usage: /think_approve <proposal_id>".into();
    }
    match brain.think_approve(id) {
        Ok(msg) => msg,
        Err(e) => format!("Approve failed: {}", e),
    }
}

fn cmd_think_reject(brain: &mut Brain, id: &str) -> String {
    let id = id.trim();
    if id.is_empty() {
        return "Usage: /think_reject <proposal_id>".into();
    }
    match brain.think_reject(id) {
        Ok(msg) => msg,
        Err(e) => format!("Reject failed: {}", e),
    }
}

fn cmd_think_status(brain: &Brain) -> String {
    match brain.think_status() {
        Some(state) => {
            let mut lines = vec![
                format!("THINKING: {}", state.goal),
                "━".repeat(50),
                format!("  State:      {:?}", state.state),
                format!("  Iteration:  {}", state.iteration),
                format!("  Coherence:  {:.1}%", state.coherence * 100.0),
                format!("  Sub-goals:  {} ({} resolved)",
                    state.sub_goal_count, state.resolved_count),
                format!("  Dry run:    {}", if state.dry_run { "yes" } else { "no" }),
            ];

            if !state.pending_proposals.is_empty() {
                lines.push(String::new());
                lines.push(format!("PENDING PROPOSALS ({}):", state.pending_proposals.len()));
                for p in &state.pending_proposals {
                    let safe_marker = if p.is_safe() { "○" } else { "●" };
                    lines.push(format!("  {} [{}] {:?}: {}",
                        safe_marker, p.id, p.proposal_type, p.description));
                }
            }

            if let Some(ref obs) = state.last_observation {
                lines.push(String::new());
                lines.push(format!("  Last: {} → {}", obs.action, obs.outcome));
            }

            lines.join("\n")
        }
        None => "No active thinking task. Use /think <goal> to start.".into(),
    }
}

fn cmd_think_simulate(brain: &mut Brain, goal: &str) -> String {
    if goal.is_empty() {
        return "Usage: /think_simulate <goal>\n  Preview what a thinking session would do (no real changes).".into();
    }
    match brain.think_simulate(goal) {
        Ok(summary) => {
            let mut lines = vec![
                format!("SIMULATION (dry-run): {}", summary.goal),
                format!("  Iterations: {}, Coherence: {:.1}%", summary.iterations_completed, summary.final_coherence * 100.0),
                format!("  Would create {} nodes, {} bonds", summary.nodes_created, summary.bonds_formed),
                format!("  Conclusion: {}", summary.conclusion),
            ];
            if !summary.observations.is_empty() {
                lines.push("  Observations:".to_string());
                for obs in &summary.observations {
                    lines.push(format!("    - {:?}", obs));
                }
            }
            lines.join("\n")
        }
        Err(e) => format!("Simulation failed: {}", e),
    }
}

fn cmd_think_stop(brain: &mut Brain) -> String {
    match brain.think_stop() {
        Ok(summary) => {
            let mut lines = vec![
                format!("THINKING COMPLETE: {}", summary.goal),
                "━".repeat(50),
                format!("  Iterations:    {}", summary.iterations_completed),
                format!("  Coherence:     {:.1}%", summary.final_coherence * 100.0),
                format!("  Nodes created: {}", summary.nodes_created),
                format!("  Bonds formed:  {}", summary.bonds_formed),
                format!("  Promoted:      {}", summary.nodes_promoted),
                format!("  Cleaned up:    {}", summary.nodes_cleaned),
                format!("  Duration:      {:.1}s", summary.duration_secs),
            ];

            if summary.dry_run {
                lines.push("  (DRY RUN — nothing was permanently changed)".into());
            }

            if !summary.observations.is_empty() {
                lines.push(String::new());
                lines.push("OBSERVATIONS:".into());
                for obs in &summary.observations {
                    lines.push(format!("  [iter {}] {} → {} (Δ{:+.2})",
                        obs.iteration, obs.action, obs.outcome, obs.coherence_delta));
                }
            }

            lines.push(String::new());
            lines.push(format!("CONCLUSION: {}", summary.conclusion));

            lines.join("\n")
        }
        Err(e) => format!("Think stop failed: {}", e),
    }
}

fn cmd_save(brain: &Brain) -> String {
    match brain.save() {
        Ok(()) => "Brain state saved.".into(),
        Err(e) => format!("Save failed: {}", e),
    }
}

fn cmd_help() -> String {
    [
        "Just type to talk. Use /commands for everything else.",
        "",
        "  TEACH",
        "  /learn <content>          Teach directly (highest trust)",
        "  /ingest <text>            Ingest text as proposals",
        "  /ingest_file <path>       Ingest file (txt/md/pdf/img/audio/video)",
        "  /media_search <query>     Cross-modal search: find media from text",
        "  /approve_all <ids>        Bulk approve proposals",
        "",
        "  MANAGE",
        "  /associate <s1> <s2> <d>  Bond two seeds",
        "  /revoke <seed> <reason>   Revoke a seed, create scar",
        "  /propose <content>        Preview external learning",
        "  /approve <id>             Approve a proposal",
        "  /reject <id>              Reject a proposal",
        "",
        "  TALK",
        "  Just type (no slash)      Lu responds conversationally (narration engine)",
        "  /talk <query>             Same as above, explicit command",
        "  /respond <query>          Old-style response (compose_response)",
        "",
        "  THINK",
        "  /think <goal>             Start a deliberate thinking session",
        "  /think_step (ts)          Advance one thinking iteration",
        "  /think_approve (ta) <id>  Approve a thinking proposal",
        "  /think_reject (tr) <id>   Reject a thinking proposal",
        "  /think_simulate (tsim)    Dry-run: preview what thinking would do",
        "  /think_status             Show current thinking state",
        "  /think_stop               End thinking session, get summary",
        "  /reason <query>           Multi-hop reasoning with chain analysis",
        "  /reflect                  Self-reflect: strengthen co-activated bonds",
        "  /ruminate                 Show what Lu is thinking about in the background",
        "  /resize <n>               Resize pattern engine (e.g., /resize 256)",
        "",
        "  SESSION MEMORY",
        "  /session                  Show Left Brain session memory status",
        "  /session_promote <id>     Propose promoting a session fact to permanent knowledge",
        "  /session_clear            Wipe session memory (Right Brain unaffected)",
        "",
        "  AUTONOMY",
        "  /autonomy                 Show execution tiers, queue, and history",
        "  /elevate <action> <tier>  Change tier for an action type",
        "  /execute                  Run next queued action (if tier allows)",
        "",
        "  CURIOSITY",
        "  /curious                  Show curiosity engine status and gap stats",
        "  /wonder (wq) [n]          Peek at top N wonders (default 10)",
        "  /interests                Show per-domain curiosity profile",
        "",
        "  SELF-IMPROVEMENT",
        "  /improve (si)             Performance metrics and gap trends",
        "  /meta_reflect (mr)        Run meta-reflection, propose improvements",
        "",
        "  INVENTION",
        "  /bridges                  Show cross-domain bridge events",
        "",
        "  KNOWLEDGE HEALTH",
        "  /health                   Knowledge graph health dashboard",
        "  /synthesis (syn)          Run synthesis pass: find missing triangle bonds",
        "",
        "  VARIANTS",
        "  /variants <query> [n]     Generate N competing variants (default 5)",
        "  /variants_status (vs)     Show current arena scores",
        "  /variants_promote (vp) [n] Promote top N variants (default 2)",
        "",
        "  TOOLS",
        "  /tools                    Tool engine status (pending, history, custom)",
        "  /tool_detect (td) <q>     Detect tool intent from a query",
        "  /tool_approve (tap) <id>  Approve a pending tool request",
        "  /tool_reject (trj) <id>   Reject a pending tool request",
        "  /tool_pending (tp)        List awaiting-approval requests",
        "  /tool_ready (tex)         List ready-to-execute requests",
        "",
        "  DEBUG",
        "  /ask <query>              Raw recall (debug mode — shows activations)",
        "  /stats                    Brain statistics",
        "  /seed_report              Seeds from last recall",
        "  /scars                    List revocation scars",
        "  /save                     Save to disk",
    ]
    .join("\n")
}

// ━━━ Session Memory Commands ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn cmd_session(brain: &Brain) -> String {
    let status = brain.session_status();
    let mut lines = vec![
        format!("Left Brain Session Memory (v2)"),
        format!("  Tasks: {} ({} active)", status.task_count, status.active_task_count),
        format!("  Attempts: {}", status.attempt_count),
        format!("  Beliefs: {} active, {} promoted, {} expired", status.active_beliefs, status.promoted_count, status.expired_count),
        format!("  Learnings: {} ({} promotion candidates)", status.learning_count, status.learnings.len()),
        format!("  Log entries: {}", status.log_entries),
    ];
    if let Some(ref goal) = status.active_goal {
        lines.push(format!("  Active goal: {}", goal));
    }
    if !status.tasks.is_empty() {
        lines.push(String::new());
        lines.push("  TASKS:".to_string());
        for t in &status.tasks {
            let parent = t.parent_id.as_deref().map(|p| format!(" (sub of {})", p)).unwrap_or_default();
            lines.push(format!("    [{}] {} — {} ({} attempts){}",
                t.id, t.description, t.status, t.attempt_count, parent));
        }
    }
    if !status.beliefs.is_empty() {
        lines.push(String::new());
        lines.push("  BELIEFS:".to_string());
        for b in &status.beliefs {
            let marker = if b.is_learning_candidate { "★" } else { "·" };
            let task = b.task_id.as_deref().map(|t| format!(" [task:{}]", t)).unwrap_or_default();
            lines.push(format!("    {} [{}] {} (conf:{:.2}, recalls:{}, age:{}s/{}s){}",
                marker, b.id, b.content, b.confidence, b.recall_count, b.age_secs, b.ttl_secs, task));
        }
    }
    if !status.learnings.is_empty() {
        lines.push(String::new());
        lines.push("  LEARNINGS (promotion candidates):".to_string());
        for l in &status.learnings {
            lines.push(format!("    [{}] {} ({})", l.id, l.insight, l.category));
        }
    }
    if !status.recent_log.is_empty() {
        lines.push(String::new());
        lines.push("  RECENT LOG:".to_string());
        for entry in &status.recent_log {
            lines.push(format!("    {}", entry));
        }
    }
    lines.join("\n")
}

fn cmd_session_promote(brain: &mut Brain, arg: &str) -> String {
    let fact_id = arg.trim();
    if fact_id.is_empty() {
        return "Usage: /session_promote <fact_id>\n  Propose promoting a session fact to permanent knowledge.\n  Use /session to see active facts.".into();
    }
    match brain.session_propose_promote(fact_id) {
        Some(proposal) => format!(
            "Promotion proposal created: {}\n  Content: {}\n  Use /approve {} to make permanent.",
            proposal.id, proposal.content, proposal.id
        ),
        None => format!(
            "No promotable session fact '{}'. Use /session to list active facts.",
            fact_id
        ),
    }
}

fn cmd_session_clear(brain: &mut Brain) -> String {
    brain.session_clear();
    "Session memory cleared. Left Brain is empty. Right Brain unaffected.".into()
}

// ━━━ Autonomy Commands ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn cmd_autonomy(brain: &Brain) -> String {
    let status = brain.autonomy_status();
    let mut lines = vec![
        "Autonomy Engine".to_string(),
        format!("  Queued actions: {}", status.queued_actions),
        String::new(),
        "  Execution Policy:".to_string(),
    ];
    for (action, tier) in &status.policy {
        lines.push(format!("    {} → {}", action, tier));
    }
    if !status.queue.is_empty() {
        lines.push(String::new());
        lines.push("  Queue:".to_string());
        for a in &status.queue {
            let approved = if a.human_approved { " ✓" } else { "" };
            lines.push(format!(
                "    [{}] {} (risk:{}, rev:{}{})",
                a.id, a.description, a.risk_level,
                a.reversible, approved
            ));
        }
    }
    if !status.recent_history.is_empty() {
        lines.push(String::new());
        lines.push("  Recent:".to_string());
        for h in &status.recent_history {
            lines.push(format!("    {}", h));
        }
    }
    lines.join("\n")
}

fn cmd_elevate(brain: &mut Brain, arg: &str) -> String {
    let parts: Vec<&str> = arg.trim().split_whitespace().collect();
    if parts.len() != 2 {
        return "Usage: /elevate <action_type> <tier>\n  Action types: Recall, CreateSubGoal, FormHypothesis, PromoteBond, LearnInsight, Revoke, StrengthenBond, WeakenBond, CreateNode, DeleteNode\n  Tiers: Observe, Suggest, Act, Create".into();
    }
    match brain.autonomy_set_tier(parts[0], parts[1]) {
        Ok(prev) => format!(
            "Elevated {} from {} → {}",
            parts[0], prev, parts[1]
        ),
        Err(e) => e,
    }
}

fn cmd_execute(brain: &mut Brain) -> String {
    match brain.autonomy_execute_next() {
        Some(result) => match result {
            brain::autonomy::ExecutionResult::Success { action_id, description } =>
                format!("✓ Executed [{}]: {}", action_id, description),
            brain::autonomy::ExecutionResult::Failed { action_id, reason } =>
                format!("✗ Failed [{}]: {}", action_id, reason),
            brain::autonomy::ExecutionResult::Blocked { action_id, reason } =>
                format!("⊘ Blocked [{}]: {}", action_id, reason),
        },
        None => "No actions in queue.".into(),
    }
}

// ━━━ Media Search Command ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn cmd_media_search(brain: &mut Brain, query: &str) -> String {
    if query.is_empty() {
        return "Usage: /media_search <query>\n  Find images, audio, and video nodes that relate to your text query.\n  Uses cross-modal cosine similarity (threshold 0.70).".into();
    }
    let results = brain.cross_modal_search(query, 10);
    if results.is_empty() {
        return format!("No media nodes matching '{}'. Try ingesting some media files first.", query);
    }
    let mut lines = vec![format!("Cross-modal search: '{}'\n", query)];
    for (seed, label, media_type, sim) in &results {
        lines.push(format!("  [{}] {} ({}) — sim: {:.4}", seed, label, media_type, sim));
    }
    lines.push(format!("\n{} media matches found.", results.len()));
    lines.join("\n")
}


// ━━━ Curiosity Engine Commands ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn cmd_curious(brain: &Brain) -> String {
    let status = brain.curiosity_status();
    let mut lines = vec![
        format!("Curiosity Engine: {}", if status.enabled { "ACTIVE" } else { "DISABLED" }),
        format!("  Pending wonders: {}", status.pending_wonders),
        format!("  Total gaps detected: {}", status.total_detected),
        format!("  Total gaps resolved: {}", status.total_resolved),
    ];

    if !status.top_wonders.is_empty() {
        lines.push(String::new());
        lines.push("  TOP WONDERS:".to_string());
        for w in status.top_wonders.iter().take(5) {
            lines.push(format!(
                "    [{}] ({:.1}) {} — \"{}\"",
                w.gap_type, w.priority, w.domain, w.suggested_query
            ));
        }
    }

    if !status.interests.is_empty() {
        lines.push(String::new());
        lines.push("  ACTIVE INTERESTS:".to_string());
        for i in status.interests.iter().take(10) {
            lines.push(format!(
                "    {} — weight: {:.2}, queries: {}, gaps: {}/{}",
                i.domain, i.weight, i.queries, i.gaps_resolved, i.gaps_found
            ));
        }
    }

    lines.join("\n")
}

fn cmd_wonder(brain: &Brain, arg: &str) -> String {
    let n: usize = arg.trim().parse().unwrap_or(10);
    let wonders = brain.curiosity_top_wonders(n);
    if wonders.is_empty() {
        return "Wonder queue is empty — no knowledge gaps detected yet.\nUse Lu normally and gaps will be detected automatically during recall.".into();
    }
    let mut lines = vec![format!("Top {} wonders:\n", wonders.len())];
    for (i, w) in wonders.iter().enumerate() {
        let gap_desc = match &w.gap {
            brain::curiosity::GapType::SparseActivation { query, activated_count } =>
                format!("sparse ({} nodes for '{}')", activated_count, query),
            brain::curiosity::GapType::DeadEnd { node_label, .. } =>
                format!("dead-end: {}", node_label),
            brain::curiosity::GapType::LowConfidenceCluster { domain, .. } =>
                format!("low-confidence cluster in {}", domain),
            brain::curiosity::GapType::MissingBridge { domain_a, domain_b } =>
                format!("missing bridge: {} <-> {}", domain_a, domain_b),
            brain::curiosity::GapType::UnresolvedScar { scar_reason } =>
                format!("unresolved scar: {}", scar_reason),
            brain::curiosity::GapType::RecurringFailure { query_pattern, failure_count } =>
                format!("recurring failure: '{}' ({}x)", query_pattern, failure_count),
        };
        lines.push(format!(
            "  {}. [{}] (priority {:.1}) {} — \"{}\"",
            i + 1, w.id, w.priority_score, gap_desc, w.suggested_query
        ));
    }
    lines.join("\n")
}

fn cmd_interests(brain: &Brain) -> String {
    let status = brain.curiosity_status();
    if status.interests.is_empty() {
        return "No domain interests tracked yet. Use Lu and interests will develop automatically.".into();
    }
    let mut lines = vec!["Domain Interest Profile:\n".to_string()];
    for i in &status.interests {
        let bar_len = (i.weight * 20.0) as usize;
        let bar: String = std::iter::repeat('#').take(bar_len).collect();
        let empty: String = std::iter::repeat('-').take(20 - bar_len).collect();
        lines.push(format!(
            "  {} [{}{bar}{empty}] {:.2}  (queries: {}, gaps: {}/{})",
            i.domain, "", i.weight, i.queries, i.gaps_resolved, i.gaps_found,
            bar = bar, empty = empty
        ));
    }
    lines.join("\n")
}


// ━━━ Variant Generation Commands ━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn cmd_variants(brain: &mut Brain, arg: &str) -> String {
    let parts: Vec<&str> = arg.rsplitn(2, ' ').collect();
    let (query, count) = if parts.len() == 2 {
        if let Ok(n) = parts[0].parse::<usize>() {
            (parts[1], n)
        } else {
            (arg, 5)
        }
    } else {
        (arg, 5)
    };

    if query.is_empty() {
        return "Usage: /variants <query> [count]\n  Generate competing hypothesis variants.\n  Each variant explores a different region of the knowledge graph.".into();
    }

    let status = brain.variants_generate(query, count);
    let mut lines = vec![
        format!("Variant Arena: '{}' (gen {})\n", status.query, status.generation),
    ];
    for v in &status.variants {
        let medal = if v.combined >= 0.5 { "★" } else { " " };
        lines.push(format!(
            "  {} [{}] {} — score: {:.3} (C={:.2} N={:.2} P={:.2}) nodes: {}",
            medal, v.id, v.strategy, v.combined, v.coherence, v.novelty, v.parsimony, v.activated_count
        ));
    }
    lines.push(format!("\nUse /variants_promote to promote winners, /variants_status for details."));
    lines.join("\n")
}

fn cmd_variants_status(brain: &Brain) -> String {
    match brain.variants_status() {
        Some(status) => {
            let mut lines = vec![
                format!("Variant Arena: '{}' (gen {})\n", status.query, status.generation),
            ];
            for v in &status.variants {
                lines.push(format!(
                    "  [{}] {} ({}) — {:.3}",
                    v.id, v.strategy, v.status, v.combined
                ));
            }
            lines.join("\n")
        }
        None => "No active variant arena. Use /variants <query> to start one.".into(),
    }
}

fn cmd_variants_promote(brain: &mut Brain, arg: &str) -> String {
    let n: usize = arg.trim().parse().unwrap_or(2);
    match brain.variants_promote(n) {
        Some(report) => {
            let mut lines = vec![format!("Promotion Report (gen {}):\n", report.generation)];
            if report.promoted.is_empty() {
                lines.push("  No variants cleared the promotion threshold.".to_string());
            } else {
                lines.push("  PROMOTED:".to_string());
                for p in &report.promoted {
                    lines.push(format!("    {} — {}", p.variant.id, p.explanation));
                }
            }
            if !report.archived.is_empty() {
                lines.push(format!("\n  Archived: {} variants", report.archived.len()));
            }
            lines.join("\n")
        }
        None => "No active variant arena. Use /variants <query> first.".into(),
    }
}


// ━━━ Knowledge Health Commands ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn cmd_health(brain: &Brain) -> String {
    let h = brain.health_report();
    let bar_len = (h.density * 1000.0).min(20.0) as usize;
    let bar: String = std::iter::repeat('#').take(bar_len).collect();
    vec![
        "Knowledge Graph Health Report".to_string(),
        format!("  Nodes: {} active / {} total", h.active_nodes, h.total_nodes),
        format!("  Bonds: {} active / {} total", h.active_bonds, h.total_bonds),
        format!("  Density: {:.6} [{}]", h.density, bar),
        format!("  Avg bonds/node: {:.1}", h.avg_bonds_per_node),
        format!("  Orphans: {} (nodes with no bonds)", h.orphan_count),
        format!("  Scars: {}", h.scars),
        format!("  Domains: ~{}", h.domain_count),
        format!("  Top hub: {} ({} bonds)", h.max_hub_label, h.max_hub_bonds),
        format!("  Coherence (avg bond weight): {:.3}", h.coherence_estimate),
    ].join("\n")
}

fn cmd_cleanup_glossary(brain: &mut Brain) -> String {
    let report = brain.cleanup_glossary_nodes();
    let _ = brain.save();

    if report.nodes_revoked == 0 {
        return "No glossary/appendix noise found. Graph is clean.".to_string();
    }

    let mut lines = vec![
        format!("Glossary Cleanup: {} candidates found, {} revoked", report.candidates_found, report.nodes_revoked),
    ];
    for (label, reason) in report.details.iter().take(20) {
        lines.push(format!("  ✗ \"{}\" — {}", label, reason));
    }
    if report.details.len() > 20 {
        lines.push(format!("  ... and {} more", report.details.len() - 20));
    }
    lines.join("\n")
}

fn cmd_synthesis(brain: &Brain) -> String {
    let report = brain.synthesis_pass();
    if report.proposals.is_empty() {
        return format!(
            "Synthesis pass: scanned {} triangles, no missing bonds found.",
            report.triangles_scanned
        );
    }
    let mut lines = vec![
        format!("Synthesis Pass: scanned {} triangles, {} missing bonds\n",
            report.triangles_scanned, report.missing_bonds_found),
    ];
    for (i, p) in report.proposals.iter().enumerate().take(20) {
        lines.push(format!(
            "  {}. {} → [{}] → {} (weight: {:.3})",
            i + 1, p.node_a_label, p.node_b_label, p.node_c_label, p.inferred_weight
        ));
    }
    if report.proposals.len() > 20 {
        lines.push(format!("  ... and {} more", report.proposals.len() - 20));
    }
    lines.push("\nUse /propose to add any of these as learning proposals.".to_string());
    lines.join("\n")
}


// ━━━ Invention Engine Commands ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn cmd_bridges(brain: &Brain) -> String {
    let status = brain.invention_status();
    let mut lines = vec![
        format!("Cross-Domain Invention Engine: {}", if status.enabled { "ACTIVE" } else { "DISABLED" }),
        format!("  Total bridges detected: {}", status.total_bridges),
        format!("  Unique domain pairs: {}", status.unique_domain_pairs),
    ];

    if !status.recent_bridges.is_empty() {
        lines.push(String::new());
        lines.push("  RECENT BRIDGES:".to_string());
        for b in &status.recent_bridges {
            lines.push(format!(
                "    [{}] {} <-> {} (length: {}, strength: {:.2}, novelty: {:.2})",
                b.id, b.source_domain, b.target_domain, b.bridge_length, b.strength, b.novelty
            ));
        }
    } else {
        lines.push("  No bridge events detected yet. Use Lu more to discover cross-domain connections.".to_string());
    }

    lines.join("\n")
}


// ━━━ Self-Improvement Commands ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn cmd_self_improvement(brain: &Brain) -> String {
    let s = brain.self_improvement_status();
    let trend_dir = if s.gap_trend > 0.1 { "improving" }
        else if s.gap_trend < -0.1 { "degrading" }
        else { "stable" };
    let mut lines = vec![
        format!("Self-Improvement Engine: {}", if s.enabled { "ACTIVE" } else { "DISABLED" }),
        format!("  Queries tracked: {}", s.total_queries),
        format!("  Avg activation: {:.1} nodes/query", s.avg_activation),
        format!("  Avg gaps: {:.1}/query (trend: {} {:.2})", s.avg_gaps, trend_dir, s.gap_trend),
        format!("  Reflections: {}, Module proposals: {}", s.reflections_count, s.module_proposals_count),
    ];

    if !s.hot_domains.is_empty() {
        lines.push(String::new());
        lines.push("  HOT DOMAINS:".to_string());
        for (domain, count) in s.hot_domains.iter().take(5) {
            lines.push(format!("    {} ({} queries)", domain, count));
        }
    }

    if !s.recent_reflections.is_empty() {
        lines.push(String::new());
        lines.push("  RECENT REFLECTIONS:".to_string());
        for r in &s.recent_reflections {
            lines.push(format!("    [{}] {} -> {}", r.id, r.pattern_observed, r.proposed_adjustment));
        }
    }

    lines.join("\n")
}

fn cmd_meta_reflect(brain: &mut Brain) -> String {
    let reflections = brain.meta_reflect();
    if reflections.is_empty() {
        return "Meta-reflection: no improvement patterns detected yet. Need more query data.".into();
    }
    let mut lines = vec![format!("Meta-Reflection: {} proposals\n", reflections.len())];
    for r in &reflections {
        lines.push(format!("  [{}] Observed: {}", r.id, r.pattern_observed));
        lines.push(format!("       Proposal: {} (confidence: {:.2})", r.proposed_adjustment, r.confidence));
        lines.push(String::new());
    }
    lines.join("\n")
}

// ━━━ Tool Integration Commands ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn cmd_tool_status(brain: &Brain) -> String {
    let status = brain.tool_status();
    let mut lines = vec![
        "Tool Integration Engine".into(),
        format!("  Enabled: {}", status.enabled),
        format!("  Pending: {} ({} awaiting approval, {} ready)",
            status.pending_count, status.awaiting_approval, status.ready_to_execute),
        format!("  History: {} executions", status.history_count),
    ];
    if !status.custom_tools.is_empty() {
        lines.push(format!("  Custom tools: {}", status.custom_tools.join(", ")));
    }
    if !status.recent_results.is_empty() {
        lines.push(String::new());
        lines.push("  Recent results:".into());
        for r in &status.recent_results {
            let icon = if r.success { "+" } else { "x" };
            lines.push(format!("    [{}] {} ({}) {}ms: {}",
                icon, r.request_id, r.tool_type, r.duration_ms, r.output_preview));
        }
    }
    lines.join("\n")
}

fn cmd_tool_detect(brain: &mut Brain, arg: &str) -> String {
    if arg.is_empty() {
        return "Usage: /tool_detect <query>".into();
    }
    match brain.tool_detect(arg) {
        Some((id, intent)) => {
            format!(
                "Tool intent detected!\n  Request ID: {}\n  Type: {}\n  Confidence: {:.2}\n  Reason: {}\n  Parameters: {:?}\n  Status: {}",
                id, intent.tool_type, intent.confidence, intent.explanation, intent.parameters,
                if intent.confidence >= 0.6 { "Submitted (check /tool_pending)" } else { "Below threshold" }
            )
        }
        None => "No tool intent detected in query.".into(),
    }
}

fn cmd_tool_approve(brain: &mut Brain, arg: &str) -> String {
    if arg.is_empty() {
        return "Usage: /tool_approve <request_id>".into();
    }
    match brain.tool_approve(arg.trim()) {
        Ok(()) => format!("Tool request {} approved. Check /tool_ready.", arg.trim()),
        Err(e) => format!("Error: {}", e),
    }
}

fn cmd_tool_reject(brain: &mut Brain, arg: &str) -> String {
    let parts: Vec<&str> = arg.splitn(2, ' ').collect();
    if parts.is_empty() || parts[0].is_empty() {
        return "Usage: /tool_reject <request_id> [reason]".into();
    }
    let id = parts[0].trim();
    let reason = if parts.len() > 1 { parts[1] } else { "Rejected by user" };
    match brain.tool_reject(id, reason) {
        Ok(()) => format!("Tool request {} rejected: {}", id, reason),
        Err(e) => format!("Error: {}", e),
    }
}

fn cmd_tool_pending(brain: &Brain) -> String {
    let pending = brain.tool_pending();
    if pending.is_empty() {
        return "No tool requests awaiting approval.".into();
    }
    let mut lines = vec![format!("{} tool requests awaiting approval:", pending.len())];
    for p in &pending {
        lines.push(format!("  [{}] {} (risk: {}) — {}", p.id, p.tool_type, p.risk, p.reason));
        if !p.parameters.is_empty() {
            lines.push(format!("       params: {:?}", p.parameters));
        }
    }
    lines.join("\n")
}

fn cmd_tool_ready(brain: &Brain) -> String {
    let ready = brain.tool_ready();
    if ready.is_empty() {
        return "No tool requests ready to execute.".into();
    }
    let mut lines = vec![format!("{} tool requests ready to execute:", ready.len())];
    for p in &ready {
        lines.push(format!("  [{}] {} (risk: {}) — {}", p.id, p.tool_type, p.risk, p.reason));
    }
    lines.join("\n")
}

// ━━━ Orchestrator (Smart Front Door) ━━━━━━━━━━━━━━━━━━━━━━━━━

fn cmd_orchestrate(brain: &mut Brain, input: &str) -> String {
    if input.is_empty() {
        return "Say something! I'll figure out what to do with it.".into();
    }
    let result = brain.orchestrate(input);
    let _ = brain.save();

    let mut lines = Vec::new();
    lines.push(result.response);

    // Show pending action if any
    if let Some(pending) = &result.pending_action {
        lines.push(String::new());
        lines.push(format!("  [{}]", pending));
    }

    // Show scar warnings
    if !result.scar_warnings.is_empty() {
        for w in &result.scar_warnings {
            lines.push(format!("  Warning: {}", w));
        }
    }

    lines.join("\n")
}
