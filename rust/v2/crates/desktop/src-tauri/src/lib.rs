// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Lu Desktop — Tauri command layer wrapping the Brain v2 crate.
//!
//! All Brain access goes through `BrainState` (a `Mutex<Brain>` held in
//! Tauri's managed state).  Every public command is prefixed `cmd_` to avoid
//! name collisions with the underlying Brain methods.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

use brain::Brain;
use df_core::Seed;
use graph::node::Confidence;
use serde::{Deserialize, Serialize};
use tauri::State;

// ── State ─────────────────────────────────────────────────────────────────────

pub struct BrainState(pub Mutex<Brain>);

// ── DTOs ──────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatResponse {
    pub text: String,
    pub timing_ms: Option<u64>,
    pub nodes_activated: Option<usize>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BrainStatus {
    pub nodes_total: usize,
    pub nodes_active: usize,
    pub bonds_total: usize,
    pub bonds_active: usize,
    pub episodes: usize,
    pub scars: usize,
    pub autonomy_tier: String,
    pub pending_proposals: usize,
    pub recall_hit_rate: f32,
    pub avg_response_quality: f32,
    pub consolidation_effectiveness: f32,
    pub total_queries: u64,
    pub warnings: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FlagItem {
    pub name: String,
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub confidence: String,
    pub ku_type: String,
    pub activation_count: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GraphBond {
    pub id: String,
    pub source: String,
    pub target: String,
    pub bond_type: String,
    pub weight: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub bonds: Vec<GraphBond>,
    pub total_nodes: usize,
    pub total_bonds: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LearnResult {
    pub count: usize,
    pub seeds: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConsolidateResult {
    pub episodes_processed: usize,
    pub bonds_strengthened: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProposalItem {
    pub id: String,
    pub kind: String,
    pub summary: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ObserveData {
    pub active_nodes: usize,
    pub active_bonds: usize,
    pub episodes_total: usize,
    pub pending_consolidation: usize,
    pub curiosity_queue: usize,
    pub wonder_items: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ThinkResult {
    pub answer: String,
    pub timing_ms: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WebSearchResult {
    pub query: String,
    pub pages_visited: usize,
    pub facts_extracted: usize,
    pub sources: Vec<String>,
    pub summary: String,
    pub elapsed_secs: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CuriosityResult {
    pub new_items: usize,
    pub pending: usize,
    pub top_questions: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DaemonToggleResult {
    pub running: bool,
    pub message: String,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn confidence_label(c: &Confidence) -> &'static str {
    match c {
        Confidence::Terminal   => "Terminal",
        Confidence::Verified   => "Verified",
        Confidence::Unverified => "Unverified",
        Confidence::Inferred   => "Inferred",
        Confidence::Contested  => "Contested",
    }
}

fn ku_kind_label(ku: &graph::knowledge_unit::KnowledgeUnit) -> &'static str {
    use graph::knowledge_unit::KuKind;
    match ku.kind() {
        KuKind::Definition  => "Definition",
        KuKind::Mechanism   => "Mechanism",
        KuKind::Composition => "Composition",
        KuKind::Purpose     => "Purpose",
        KuKind::Contrast    => "Contrast",
        KuKind::Causal      => "Causal",
        KuKind::Example     => "Example",
        KuKind::Procedure   => "Procedure",
        KuKind::Code        => "Code",
        _                   => "Raw",
    }
}

// ── Chat commands ─────────────────────────────────────────────────────────────

#[tauri::command]
fn cmd_chat(state: State<BrainState>, message: String) -> Result<ChatResponse, String> {
    let mut brain = state.0.lock().map_err(|e| e.to_string())?;
    let text = brain.respond(&message);
    let timing_ms = brain.last_timing.as_ref().map(|t| t.total_ms);
    Ok(ChatResponse {
        text,
        timing_ms,
        nodes_activated: None,
    })
}

#[tauri::command]
fn cmd_think(state: State<BrainState>, topic: String) -> Result<ThinkResult, String> {
    let mut brain = state.0.lock().map_err(|e| e.to_string())?;
    let t = std::time::Instant::now();
    let answer = brain.think_through(&topic);
    let timing_ms = t.elapsed().as_millis() as u64;
    Ok(ThinkResult { answer, timing_ms })
}

// ── Learning commands ─────────────────────────────────────────────────────────

#[tauri::command]
fn cmd_learn(
    state: State<BrainState>,
    content: String,
    confidence: String,
) -> Result<LearnResult, String> {
    let mut brain = state.0.lock().map_err(|e| e.to_string())?;
    let conf = match confidence.as_str() {
        "Verified"   => Confidence::Verified,
        "Unverified" => Confidence::Unverified,
        "Inferred"   => Confidence::Inferred,
        _            => Confidence::Terminal,
    };
    let seeds = brain.learn_text(&content, conf);
    let seed_strs: Vec<String> = seeds.iter().map(|s| s.short()).collect();
    Ok(LearnResult { count: seeds.len(), seeds: seed_strs })
}

#[tauri::command]
fn cmd_learn_file(state: State<BrainState>, path: String) -> Result<LearnResult, String> {
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Cannot read '{}': {}", path, e))?;
    let mut brain = state.0.lock().map_err(|e| e.to_string())?;
    let seeds = brain.learn_text(&content, Confidence::Terminal);
    let seed_strs: Vec<String> = seeds.iter().map(|s| s.short()).collect();
    Ok(LearnResult { count: seeds.len(), seeds: seed_strs })
}

#[tauri::command]
fn cmd_consolidate(state: State<BrainState>) -> Result<ConsolidateResult, String> {
    let mut brain = state.0.lock().map_err(|e| e.to_string())?;
    let report = brain.consolidate();
    Ok(ConsolidateResult {
        episodes_processed: report.episodes_processed,
        bonds_strengthened: report.bonds_strengthened,
    })
}

// ── Graph commands ────────────────────────────────────────────────────────────

#[tauri::command]
fn cmd_graph_data(
    state: State<BrainState>,
    limit: Option<usize>,
) -> Result<GraphData, String> {
    let brain = state.0.lock().map_err(|e| e.to_string())?;
    let node_limit = limit.unwrap_or(300);
    let bond_limit = limit.unwrap_or(500);

    let all_bonds = brain.network.all_bonds();
    let total_bonds = all_bonds.len();
    let total_nodes = brain.network.node_count();

    // Collect unique seeds from bonds (up to bond_limit bonds)
    let mut seen_seeds: HashSet<String> = HashSet::new();
    let mut node_seeds: Vec<Seed> = Vec::new();

    let bonds_to_use: Vec<_> = all_bonds.iter().take(bond_limit).collect();

    for bond in &bonds_to_use {
        let src_hex = bond.source_seed.to_hex();
        let tgt_hex = bond.target_seed.to_hex();
        if seen_seeds.insert(src_hex) && node_seeds.len() < node_limit {
            node_seeds.push(bond.source_seed.clone());
        }
        if seen_seeds.insert(tgt_hex) && node_seeds.len() < node_limit {
            node_seeds.push(bond.target_seed.clone());
        }
    }

    // Build GraphNode list
    let nodes: Vec<GraphNode> = node_seeds
        .iter()
        .filter_map(|seed| {
            let hex = seed.to_hex();
            let node = brain.network.get_node(seed)?;
            let label: String = node
                .knowledge_unit
                .raw_text()
                .chars()
                .take(80)
                .collect();
            Some(GraphNode {
                id: hex,
                label,
                confidence: confidence_label(&node.confidence).to_string(),
                ku_type: ku_kind_label(&node.knowledge_unit).to_string(),
                activation_count: node.activation_count,
            })
        })
        .collect();

    // Build GraphBond list (only include bonds whose nodes are in our set)
    let bonds: Vec<GraphBond> = bonds_to_use
        .iter()
        .filter_map(|b| {
            let src = b.source_seed.to_hex();
            let tgt = b.target_seed.to_hex();
            if seen_seeds.contains(&src) && seen_seeds.contains(&tgt) {
                Some(GraphBond {
                    id: b.seed.to_hex(),
                    source: src,
                    target: tgt,
                    bond_type: format!("{:?}", b.bond_type),
                    weight: b.weight,
                })
            } else {
                None
            }
        })
        .collect();

    Ok(GraphData { nodes, bonds, total_nodes, total_bonds })
}

// ── Proposal commands ─────────────────────────────────────────────────────────

#[tauri::command]
fn cmd_proposals_list(state: State<BrainState>) -> Result<Vec<ProposalItem>, String> {
    let brain = state.0.lock().map_err(|e| e.to_string())?;
    let items = brain
        .proposals
        .pending()
        .iter()
        .map(|p| ProposalItem {
            id: p.id.clone(),
            kind: p.proposal.kind().to_string(),
            summary: p.proposal.summary(),
            created_at: p.created_at.to_rfc3339(),
        })
        .collect();
    Ok(items)
}

#[tauri::command]
fn cmd_proposal_approve(
    state: State<BrainState>,
    proposal_id: String,
) -> Result<String, String> {
    let mut brain = state.0.lock().map_err(|e| e.to_string())?;
    match brain.proposals.approve(&proposal_id) {
        None => Err(format!("No pending proposal '{}'", &proposal_id[..8.min(proposal_id.len())])),
        Some(pending) => {
            let outcome = apply_proposal(&mut brain, &pending.proposal);
            Ok(format!("Approved '{}': {}", &proposal_id[..8.min(proposal_id.len())], outcome))
        }
    }
}

#[tauri::command]
fn cmd_proposal_reject(
    state: State<BrainState>,
    proposal_id: String,
) -> Result<String, String> {
    let mut brain = state.0.lock().map_err(|e| e.to_string())?;
    if brain.proposals.reject(&proposal_id) {
        Ok(format!("Rejected '{}'", &proposal_id[..8.min(proposal_id.len())]))
    } else {
        Err(format!("No pending proposal '{}'", &proposal_id[..8.min(proposal_id.len())]))
    }
}

// ── Settings commands ─────────────────────────────────────────────────────────

#[tauri::command]
fn cmd_status(state: State<BrainState>) -> Result<BrainStatus, String> {
    let mut brain = state.0.lock().map_err(|e| e.to_string())?;
    let stats = brain.stats();
    let health = brain.health();
    let si = &brain.self_improvement.metrics;
    let ep_count = brain.hippocampus.episodes.len();
    Ok(BrainStatus {
        nodes_total: stats.total_nodes,
        nodes_active: stats.active_nodes,
        bonds_total: stats.total_bonds,
        bonds_active: stats.active_bonds,
        episodes: ep_count,
        scars: health.scar_count,
        autonomy_tier: health.autonomy_tier.label().to_string(),
        pending_proposals: brain.proposals.pending_count(),
        recall_hit_rate: si.recall_hit_rate,
        avg_response_quality: si.avg_response_quality,
        consolidation_effectiveness: si.consolidation_effectiveness,
        total_queries: si.total_queries,
        warnings: health.warnings.clone(),
    })
}

#[tauri::command]
fn cmd_observe(state: State<BrainState>) -> Result<ObserveData, String> {
    let brain = state.0.lock().map_err(|e| e.to_string())?;
    let stats = brain.stats();
    let ep_count = brain.hippocampus.episodes.len();
    let pending = brain.hippocampus.pending_count();
    let wonder_items: Vec<String> = brain
        .hippocampus
        .episodes
        .iter()
        .rev()
        .take(5)
        .map(|e| e.description.clone())
        .collect();
    let curiosity_queue = brain.curiosity.wonder_queue.pending_count();
    Ok(ObserveData {
        active_nodes: stats.active_nodes,
        active_bonds: stats.active_bonds,
        episodes_total: ep_count,
        pending_consolidation: pending,
        curiosity_queue,
        wonder_items,
    })
}

#[tauri::command]
fn cmd_flags_list(state: State<BrainState>) -> Result<Vec<FlagItem>, String> {
    let brain = state.0.lock().map_err(|e| e.to_string())?;
    let items = brain
        .flags
        .list()
        .into_iter()
        .map(|(name, enabled)| FlagItem { name: name.to_string(), enabled })
        .collect();
    Ok(items)
}

#[tauri::command]
fn cmd_flag_set(
    state: State<BrainState>,
    name: String,
    value: bool,
) -> Result<(), String> {
    let mut brain = state.0.lock().map_err(|e| e.to_string())?;
    brain.flags.set(&name, value)
}

#[tauri::command]
fn cmd_configure(
    state: State<BrainState>,
    autonomy_tier: Option<String>,
    storage_path: Option<String>,
) -> Result<String, String> {
    let mut brain = state.0.lock().map_err(|e| e.to_string())?;
    let mut changes: Vec<String> = Vec::new();

    if let Some(tier_str) = autonomy_tier {
        use brain::hypothalamus::AutonomyTier;
        let tier = match tier_str.to_lowercase().as_str() {
            "supervised"    => Some(AutonomyTier::Supervised),
            "assisted"      => Some(AutonomyTier::Assisted),
            "collaborative" => Some(AutonomyTier::Collaborative),
            "autonomous"    => Some(AutonomyTier::Autonomous),
            other => return Err(format!("Unknown tier '{}'. Valid: Supervised, Assisted, Collaborative, Autonomous", other)),
        };
        if let Some(t) = tier {
            brain.hypothalamus.set_autonomy_tier(t);
            changes.push(format!("tier → {}", t.label()));
        }
    }

    if let Some(path_str) = storage_path {
        let p = PathBuf::from(&path_str);
        brain.set_storage(p);
        changes.push(format!("storage → {}", path_str));
    }

    if changes.is_empty() {
        Ok(format!("No changes. Tier: {}", brain.hypothalamus.autonomy_tier.label()))
    } else {
        Ok(changes.join("; "))
    }
}

#[tauri::command]
fn cmd_save(state: State<BrainState>) -> Result<String, String> {
    let mut brain = state.0.lock().map_err(|e| e.to_string())?;
    brain.save()?;
    let stats = brain.stats();
    Ok(format!(
        "Saved. ({} nodes, {} bonds)",
        stats.total_nodes, stats.total_bonds
    ))
}

#[tauri::command]
fn cmd_load(state: State<BrainState>, path: String) -> Result<String, String> {
    let p = PathBuf::from(&path);
    let new_brain = Brain::load(&p)?;
    let stats = new_brain.stats();
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    *guard = new_brain;
    Ok(format!(
        "Loaded '{}'. ({} nodes, {} bonds)",
        path, stats.total_nodes, stats.total_bonds
    ))
}

#[tauri::command]
fn cmd_revoke(
    state: State<BrainState>,
    seed_hex: String,
    reason: String,
) -> Result<String, String> {
    let seed = Seed::from_hex(&seed_hex)
        .map_err(|e| format!("Invalid seed hex: {}", e))?;
    let mut brain = state.0.lock().map_err(|e| e.to_string())?;
    brain.revoke(&seed, &reason);
    Ok(format!("Revoked {}: {}", &seed_hex[..16.min(seed_hex.len())], reason))
}

// ── Web research commands ─────────────────────────────────────────────────────

#[tauri::command]
fn cmd_web_search(state: State<BrainState>, query: String) -> Result<WebSearchResult, String> {
    let mut brain = state.0.lock().map_err(|e| e.to_string())?;
    if !brain.flags.web_research {
        return Err("Web research is disabled. Enable it via feature flags.".to_string());
    }
    let report = brain.web_research(&query);
    Ok(WebSearchResult {
        query: report.query,
        pages_visited: report.pages_visited,
        facts_extracted: report.facts_extracted,
        sources: report.sources,
        summary: report.summary,
        elapsed_secs: report.elapsed_secs,
    })
}

#[tauri::command]
fn cmd_curiosity_trigger(state: State<BrainState>) -> Result<CuriosityResult, String> {
    let mut brain = state.0.lock().map_err(|e| e.to_string())?;
    let new_items = brain.run_curiosity();
    let pending = brain.curiosity.wonder_queue.pending_count();
    let top_questions: Vec<String> = brain
        .curiosity
        .wonder_queue
        .pending()
        .iter()
        .take(5)
        .map(|w| w.question.clone())
        .collect();
    Ok(CuriosityResult { new_items, pending, top_questions })
}

#[tauri::command]
fn cmd_toggle_daemon(state: State<BrainState>) -> Result<DaemonToggleResult, String> {
    let mut brain = state.0.lock().map_err(|e| e.to_string())?;
    if brain.flags.daemon_mode {
        brain.flags.daemon_mode = false;
        brain.daemon.stop();
        Ok(DaemonToggleResult {
            running: false,
            message: "Daemon stopped.".to_string(),
        })
    } else {
        brain.start_daemon();
        Ok(DaemonToggleResult {
            running: true,
            message: "Daemon started. Idle research, rumination and consolidation now run in background.".to_string(),
        })
    }
}

// ── Proposal application (mirrors mcp_server) ─────────────────────────────────

fn apply_proposal(
    brain: &mut Brain,
    proposal: &brain::proposals::ThinkingProposal,
) -> String {
    use brain::proposals::ThinkingProposal;
    use graph::bond::BondType;

    match proposal {
        ThinkingProposal::CreateBond { source_seed, target_seed, bond_type, weight } => {
            let bond_t = match bond_type.to_lowercase().as_str() {
                "isa"         => BondType::IsA,
                "haspart"     => BondType::HasPart,
                "causes"      => BondType::Causes,
                "requires"    => BondType::Requires,
                "opposes"     => BondType::Opposes,
                "exampleof"   => BondType::ExampleOf,
                "sequence"    => BondType::Sequence,
                "derivesfrom" => BondType::DerivesFrom,
                "depicts"     => BondType::Depicts,
                "implements"  => BondType::Implements,
                _             => BondType::Context,
            };
            match (Seed::from_hex(source_seed), Seed::from_hex(target_seed)) {
                (Ok(src), Ok(tgt)) => match brain.associate(&src, &tgt, bond_t, *weight) {
                    Ok(_)  => format!("Created {} bond (w={:.2})", bond_type, weight),
                    Err(e) => format!("Bond creation failed: {}", e),
                },
                _ => "Invalid seed hex in CreateBond proposal".to_string(),
            }
        }

        ThinkingProposal::StrengthenBond { bond_seed, amount } => {
            match Seed::from_hex(bond_seed) {
                Ok(seed) => {
                    if let Some(bond) = brain.network.get_bond_mut(&seed) {
                        bond.strengthen(*amount);
                        format!("Strengthened bond by {:.2}", amount)
                    } else {
                        "Bond not found".to_string()
                    }
                }
                Err(_) => "Invalid bond seed hex".to_string(),
            }
        }

        ThinkingProposal::WeakenBond { bond_seed, amount } => {
            match Seed::from_hex(bond_seed) {
                Ok(seed) => {
                    if let Some(bond) = brain.network.get_bond_mut(&seed) {
                        bond.weaken(*amount);
                        "Bond weakened".to_string()
                    } else {
                        "Bond not found".to_string()
                    }
                }
                Err(_) => "Invalid bond seed hex".to_string(),
            }
        }

        ThinkingProposal::RevokeNode { seed, reason } => {
            match Seed::from_hex(seed) {
                Ok(s) => { brain.revoke(&s, reason); format!("Node revoked: {}", reason) }
                Err(_) => "Invalid node seed hex".to_string(),
            }
        }

        ThinkingProposal::LearnInsight { text, confidence } => {
            let conf = match confidence.to_lowercase().as_str() {
                "verified"   => Confidence::Verified,
                "unverified" => Confidence::Unverified,
                "inferred"   => Confidence::Inferred,
                _            => Confidence::Terminal,
            };
            let seeds = brain.learn_text(text, conf);
            format!("Learned {} KU(s)", seeds.len())
        }

        ThinkingProposal::FormHypothesis { hypothesis, .. } => {
            let seeds = brain.learn_text(hypothesis, Confidence::Inferred);
            format!("Hypothesis: {} inferred KU(s)", seeds.len())
        }
    }
}

// ── App entry point ───────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Brain path resolution (highest priority first):
    //   1. --brain <path> CLI argument
    //   2. LU_BRAIN_PATH environment variable
    //   3. v2\brain\lu_brain.lbr (known default)
    //   4. %APPDATA%\lu-desktop\brain.json (fallback)
    let cli_path = {
        let args: Vec<String> = std::env::args().collect();
        args.windows(2)
            .find(|w| w[0] == "--brain")
            .map(|w| PathBuf::from(&w[1]))
    };

    // 3. Known default location for this machine
    let known_default = PathBuf::from(r"v2\brain\lu_brain.lbr");

    let brain_path = cli_path
        .or_else(|| std::env::var("LU_BRAIN_PATH").ok().map(PathBuf::from))
        .or_else(|| known_default.exists().then_some(known_default))
        .or_else(dirs_path);

    let brain = if let Some(ref p) = brain_path {
        if p.exists() {
            Brain::load(p).unwrap_or_else(|_| Brain::new())
        } else {
            Brain::new()
        }
    } else {
        Brain::new()
    };

    let mut brain = brain;
    if let Some(p) = brain_path {
        brain.set_storage(p);
    }

    tauri::Builder::default()
        .manage(BrainState(Mutex::new(brain)))
        .invoke_handler(tauri::generate_handler![
            cmd_chat,
            cmd_think,
            cmd_learn,
            cmd_learn_file,
            cmd_consolidate,
            cmd_graph_data,
            cmd_proposals_list,
            cmd_proposal_approve,
            cmd_proposal_reject,
            cmd_status,
            cmd_observe,
            cmd_flags_list,
            cmd_flag_set,
            cmd_configure,
            cmd_save,
            cmd_load,
            cmd_revoke,
            cmd_web_search,
            cmd_curiosity_trigger,
            cmd_toggle_daemon,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Lu Desktop");
}

/// Returns the default brain storage path:
/// `%APPDATA%\lu-desktop\brain.json` on Windows,
/// `~/.local/share/lu-desktop/brain.json` on Linux/macOS.
fn dirs_path() -> Option<PathBuf> {
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .ok()?;
    let dir = base.join("lu-desktop");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("brain.json"))
}
