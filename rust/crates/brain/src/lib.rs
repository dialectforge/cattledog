// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # The Brain
//!
//! The complete associative intelligence. Ties together:
//! - df_core: DF protocol, seeds, gating, bonds
//! - graph: associative network, nodes, traversal, scars
//! - pattern_engine: encoding through DF
//!
//! Provides the high-level API that the terminal calls.

pub mod arena;
pub mod autonomy;
pub mod connectors;
pub mod curiosity;
pub mod invention;
pub mod knowledge_health;
pub mod self_improvement;
pub mod variant_promotion;
pub mod variants;
pub mod coordinator;
pub mod narration;
pub mod reasoning;
pub mod rumination;
pub mod session_memory;
pub mod tokenizer;
pub mod tools;
pub mod orchestrator;
pub mod web_safety;
pub mod web_fetch;
pub mod web_research;
pub mod web_idle;
pub mod computer_use;
pub mod conversations;

use std::path::PathBuf;
use std::collections::{HashMap, HashSet};

/// Keywords matching more nodes than this ceiling contribute zero to recall activation.
/// Words like "used", "tell", "work", "does" match thousands of nodes in a large graph
/// and act as noise. Treating them as dynamic stop words improves recall precision.
const IDF_MATCH_CEILING: usize = 500;

/// Question and function words that carry no topical meaning in a query.
/// These are stripped from the query BEFORE keyword extraction so that
/// "What is neural network?" behaves identically to "neural network".
/// This is separate from IDF_MATCH_CEILING which is a second-layer defence
/// for content words that happen to be very common in the graph.
const QUERY_STOP_WORDS: &[&str] = &[
    // Question words
    "what", "which", "where", "when", "who", "whom", "whose", "why", "how",
    // Auxiliary verbs
    "is", "am", "are", "was", "were", "be", "been", "being",
    "do", "does", "did", "doing",
    "have", "has", "had", "having",
    "will", "would", "shall", "should", "can", "could", "may", "might", "must",
    // Articles
    "a", "an", "the",
    // Personal pronouns
    "i", "me", "my", "we", "us", "our", "you", "your", "he", "she", "it", "they", "them", "their",
    // Common prepositions
    "in", "on", "at", "to", "for", "of", "with", "by", "from", "about", "into", "through",
    // Conjunctions / negation
    "and", "or", "but", "not", "no", "nor",
    // Demonstratives
    "this", "that", "these", "those",
    // Meta-question verbs (common in conversational queries)
    "tell", "explain", "describe", "define",
];

use df_core::{Seed, GatedData};
use df_core::bond::TerminalBond;
use graph::{KnowledgeNode, NodeType, Confidence, AssociativeNetwork, Scar};
use graph::network::{RecallResult, RevocationReport, NetworkStats, ActivatedNode};
use pattern_engine::PatternEngine;
use serde::{Serialize, Deserialize};

/// The complete AI brain.
#[derive(Serialize, Deserialize)]
pub struct Brain {
    /// The DF bond with the terminal (the imprint)
    terminal_bond: TerminalBond,
    /// The associative knowledge network
    network: AssociativeNetwork,
    /// The pattern encoding engine
    pattern_engine: PatternEngine,
    /// Where brain state persists on disk
    #[serde(skip)]
    storage_path: Option<PathBuf>,
    /// Last recall result (for seed_report command)
    #[serde(skip)]
    last_recall: Option<RecallResult>,
    /// Conversation memory: recent queries for follow-up context.
    /// Short/ambiguous queries get enriched with recent context.
    #[serde(skip)]
    conversation_history: Vec<String>,
    /// Pending learning proposals awaiting approval
    #[serde(skip)]
    pending_proposals: HashMap<String, LearningProposal>,
    /// Pending image proposals awaiting approval
    #[serde(skip)]
    pending_image_proposals: HashMap<String, ImageProposal>,
    /// Pending audio proposals awaiting approval
    #[serde(skip)]
    pending_audio_proposals: HashMap<String, AudioProposal>,
    /// Pending video proposals awaiting approval
    #[serde(skip)]
    pending_video_proposals: HashMap<String, VideoProposal>,
    /// Active thinking task (if any). Not persisted — thinking
    /// is session-scoped. Orphaned temp nodes cleaned on load.
    #[serde(skip)]
    active_goal: Option<coordinator::GoalCluster>,
    /// Left Brain: operational session memory.
    /// Holds temporary context and session facts that INFORM reasoning
    /// but can NEVER auto-promote to Right Brain (graph) fact nodes.
    /// Promotion requires explicit human approval via propose/approve.
    #[serde(skip)]
    session_memory: session_memory::SessionMemory,
    /// Autonomous execution layer: tiered safety controls.
    /// Determines which actions can auto-execute vs need human approval.
    /// Default: everything at Suggest tier (maximally safe).
    #[serde(skip)]
    autonomy: autonomy::AutonomyEngine,
    /// Curiosity engine: detects knowledge gaps, ranks them by interest,
    /// and queues them for research. Runs passively after every recall.
    #[serde(skip)]
    curiosity: curiosity::CuriosityEngine,
    /// Active variant arena (if any). Holds competing hypothesis variants.
    #[serde(skip)]
    active_arena: Option<arena::VariantArena>,
    /// Invention engine: detects cross-domain bridge events.
    #[serde(skip)]
    invention: invention::InventionEngine,
    /// Self-improvement engine: performance monitoring + meta-reflection.
    #[serde(skip)]
    self_improvement: self_improvement::SelfImprovementEngine,
    /// Tool integration layer: intent detection, request gating, execution tracking.
    #[serde(skip)]
    tool_engine: tools::ToolEngine,
    /// Idle exploration: Lu goes browsing when you're away.
    #[serde(default)]
    idle_explorer: web_idle::IdleExplorer,
    /// Persisted curiosity config flags. The CuriosityEngine itself is
    /// #[serde(skip)] (bincode-incompatible), so config that must survive
    /// restarts lives here and is pushed into the engine after load.
    #[serde(default)]
    curiosity_auto_resolve: bool,
    /// Computer use: sandboxed code execution, file I/O.
    #[serde(skip)]
    computer_use: computer_use::ComputerUseEngine,
    /// Persistent conversation store. Separate files, not in brain_state.
    #[serde(skip)]
    conversations: conversations::ConversationStore,
    /// Inverted word index: stemmed_word → sorted vec of node hex strings.
    /// Built lazily on first recall; invalidated by learn/revoke.
    /// Turns O(N×Q) keyword scan into O(Q×hits).
    #[serde(skip)]
    word_index: HashMap<String, Vec<String>>,
    /// False when word_index needs a rebuild.
    #[serde(skip)]
    word_index_valid: bool,
    /// Cached PhraseIndex rebuilt only when node count changes.
    #[serde(skip)]
    phrase_index_cache: Option<crate::tokenizer::PhraseIndex>,
    /// Node count at the last phrase_index rebuild.
    #[serde(skip)]
    phrase_index_node_count: usize,
    /// Set by recall() when auto_resolve is enabled and a sparse/gap result
    /// is detected. Drained by drain_auto_resolve() in the worker loop AFTER
    /// the MCP response has already been sent — so it never blocks the caller.
    #[serde(skip)]
    pending_auto_research: bool,
}

/// Result of an autonomous wonder-resolution cycle.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AutoResolveResult {
    pub wonder_id: String,
    pub query_used: String,
    pub pages_visited: usize,
    pub facts_proposed: usize,
    pub proposals_approved: usize,
    pub gap_filled: bool,
    pub summary: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LearningProposal {
    pub id: String,
    pub content: String,
    pub source: String,
    pub similar_existing: Vec<(String, String, f32)>, // (seed_hex, label, similarity)
    pub scar_conflicts: Vec<String>,
    pub recommendation: String,
}

/// A proposed image perception awaiting approval.
/// Raw image bytes are wrapped as GatedData — the seed gates access.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageProposal {
    pub id: String,
    pub label: String,
    pub source: String,
    pub gated_bytes: GatedData,
    pub features_summary: String,
    pub similar_existing: Vec<(String, String, f32)>,
    pub scar_conflicts: Vec<String>,
    pub recommendation: String,
}

/// A proposed audio perception awaiting approval.
/// Raw audio bytes are wrapped as GatedData — the seed gates access.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioProposal {
    pub id: String,
    pub label: String,
    pub source: String,
    pub gated_bytes: GatedData,
    pub features_summary: String,
    pub similar_existing: Vec<(String, String, f32)>,
    pub scar_conflicts: Vec<String>,
    pub recommendation: String,
}

/// A proposed video perception awaiting approval.
/// Raw keyframe bytes are wrapped as GatedData — the seed gates access.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VideoProposal {
    pub id: String,
    pub label: String,
    pub source: String,
    /// Each keyframe's raw bytes, individually gated
    pub gated_keyframes: Vec<GatedData>,
    pub features_summary: String,
    pub similar_existing: Vec<(String, String, f32)>,
    pub scar_conflicts: Vec<String>,
    pub recommendation: String,
}

/// Input item for session report preview.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionReportItem {
    pub content: String,
    pub associations: Option<Vec<String>>,
}

/// One entry in the session report manifest.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionReportEntry {
    pub content: String,
    pub status: String,  // READY, SCAR_CONFLICT, POSSIBLE_DUPLICATE
    pub similar_existing: Vec<(String, String, f32)>,
    pub scar_conflicts: Vec<String>,
    pub association_targets: Vec<String>,
    pub bond_count: usize,
}

/// Full session report: the manifest of what a batch would do.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionReport {
    pub total_items: usize,
    pub total_new_atoms: usize,
    pub total_new_bonds: usize,
    pub total_scar_warnings: usize,
    pub current_brain_nodes: usize,
    pub current_brain_bonds: usize,
    pub projected_nodes: usize,
    pub projected_bonds: usize,
    pub entries: Vec<SessionReportEntry>,
}

/// A single proposed knowledge atom from external data ingestion.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IngestProposal {
    pub id: String,
    pub content: String,
    pub status: String,       // READY, SCAR_CONFLICT, DUPLICATE
    pub scar_conflicts: Vec<String>,
    pub similar_existing: Vec<(String, String, f32)>, // (seed_hex, label, similarity)
}

/// Result of ingesting external data.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IngestReport {
    pub source: String,
    pub total_sentences: usize,
    pub proposals_ready: usize,
    pub proposals_blocked: usize,
    pub duplicates_skipped: usize,
    pub proposals: Vec<IngestProposal>,
}

/// A brain-composed response from traversal paths.
/// The brain's own voice — no LLM, just activated knowledge stitched together.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrainResponse {
    /// The composed response text
    pub text: String,
    /// The query that triggered this response
    pub query: String,
    /// How many knowledge atoms contributed
    pub atoms_used: usize,
    /// The traversal depth used
    pub traversal_hops: usize,
    /// Confidence: how much of the response came from high-contribution nodes
    pub coherence_score: f32,
    /// The contributing atoms with their contribution percentages
    pub sources: Vec<(String, String, f32)>, // (seed_hex, label, contribution_pct)
    /// Scar warnings triggered during recall
    pub scar_warnings: Vec<String>,
    /// Image URLs found in activated knowledge (for terminal/MCP to present)
    pub image_urls: Vec<(String, String)>, // (url, description)
    /// True when activated nodes have low keyword overlap with the query —
    /// Lu doesn't have confident knowledge about this topic.
    #[serde(default)]
    pub knowledge_gap: bool,
    /// Lu's proactive prompt to the user — set when Lu wants to suggest
    /// an action (e.g. "Want me to research this?") or flag uncertainty.
    /// Also available for future features where Lu initiates communication.
    #[serde(default)]
    pub suggestion: Option<String>,
}

// ─── Question Understanding ─────────────────────────────────

/// What kind of question is the user asking?
/// This determines how we formulate the response — not just
/// what we recall, but how we shape and frame it.
#[derive(Clone, Debug, PartialEq)]
pub enum QueryIntent {
    /// "can you...", "do you know...", "are you able to..."
    /// → Check if brain has relevant knowledge, answer about capability
    Ability { topic: String },
    /// "is X...", "does X...", "do X...", "are X..."
    /// → Find relevant fact, extract yes/no + supporting evidence
    YesNo { topic: String },
    /// "what is...", "what are...", "define...", "what does X mean"
    /// → Find definition nodes (existing behavior works well here)
    Definition { topic: String },
    /// "how do...", "how does...", "how to...", "how can..."
    /// → Look for procedural/process knowledge
    Process { topic: String },
    /// "why is...", "why does...", "why do...", "why are..."
    /// → Look for causal/explanatory knowledge
    Why { topic: String },
    /// "who is...", "who was...", "who invented..."
    /// → Look for person/entity knowledge
    Who { topic: String },
    /// Comparison: "difference between X and Y", "is X bigger than Y"
    /// → Recall both topics, present side-by-side
    Comparison { topic_a: String, topic_b: String },
    /// List/Enumeration: "list the planets", "name 3 types of rocks"
    /// → Recall broadly, present as enumerated items
    List { topic: String, count: Option<usize> },
    /// Think: "what would happen if...", "could X do Y", "is X possible"
    /// → Multi-cluster activation, find connections, synthesize novel thought
    Think { topic: String },
    /// Explore: "tell me everything about X", "what do you know about X"
    /// → Breadth-first diverse recall, maximize coverage not precision
    Explore { topic: String },
    /// Just a topic or imperative: "tell me about X", "math", "gravity"
    /// → General recall + compose (current behavior)
    General { topic: String },
    /// Greeting: "hello", "hi", "hey"
    Greeting,
    /// Meta-question about the brain itself: "what are you", "who are you"
    MetaSelf,
}

fn classify_query(query: &str) -> QueryIntent {
    let q = query.trim().to_lowercase();
    let q = q.trim_end_matches(&['?', '!', '.'][..]).trim();

    // ─── Greetings ────────────────────────────────────
    let greetings = ["hello", "hi", "hey", "howdy", "greetings",
                     "good morning", "good afternoon", "good evening",
                     "yo", "sup", "what's up", "whats up"];
    if greetings.iter().any(|g| q == *g) {
        return QueryIntent::Greeting;
    }

    // ─── Meta-self questions ──────────────────────────
    let meta_patterns = [
        "what are you", "who are you", "what is your name",
        "tell me about yourself", "describe yourself",
        "what can you do", "what do you do",
        "how do you work", "how were you made",
        "are you an ai", "are you a bot", "are you alive",
        "are you sentient", "are you conscious",
    ];
    if meta_patterns.iter().any(|p| q == *p || q.starts_with(p)) {
        return QueryIntent::MetaSelf;
    }

    // ─── Ability questions ────────────────────────────
    // "can you do math", "do you know about physics", "are you able to reason"
    let ability_starters = [
        "can you ", "could you ", "are you able to ",
        "do you know ", "do you understand ",
        "have you learned ", "did you learn ",
    ];
    for starter in &ability_starters {
        if q.starts_with(starter) {
            let topic = q[starter.len()..].trim().to_string();
            let topic = clean_topic(&topic);
            return QueryIntent::Ability { topic };
        }
    }

    // ─── Comparison questions ─────────────────────────
    // "difference between X and Y", "compare X and Y",
    // "X vs Y", "is X bigger than Y", "how is X different from Y"
    if let Some(comp) = try_parse_comparison(q) {
        return comp;
    }

    // ─── List/Enumeration questions ───────────────────
    // "list the planets", "name 3 types of rocks", "what are the types of..."
    if let Some(list) = try_parse_list(q) {
        return list;
    }

    // ─── Yes/No questions ─────────────────────────────
    let yesno_starters = [
        "is ", "are ", "does ", "do ", "did ", "was ", "were ",
        "has ", "have ", "will ", "would ", "should ", "can ",
    ];
    // Only match if it's a real question structure, not ability
    for starter in &yesno_starters {
        if q.starts_with(starter) {
            let rest = q[starter.len()..].trim();
            // Skip if it's really a meta question we already caught
            if rest.starts_with("you ") && (starter == &"are " || starter == &"can ") {
                // "are you..." / "can you..." that wasn't caught above
                let topic = rest.trim_start_matches("you ")
                    .trim_start_matches("able to ")
                    .trim()
                    .to_string();
                return QueryIntent::Ability { topic: clean_topic(&topic) };
            }
            return QueryIntent::YesNo { topic: clean_topic(rest) };
        }
    }

    // ─── Definition questions ─────────────────────────
    let def_starters = [
        "what is ", "what are ", "what's ", "whats ",
        "define ", "what does ", "what do ",
    ];
    for starter in &def_starters {
        if q.starts_with(starter) {
            let topic = q[starter.len()..].trim()
                .trim_end_matches(" mean")
                .trim_end_matches(" means")
                .to_string();
            return QueryIntent::Definition { topic: clean_topic(&topic) };
        }
    }

    // ─── Process questions ────────────────────────────
    let how_starters = [
        "how do ", "how does ", "how to ", "how can ",
        "how is ", "how are ", "how did ",
    ];
    for starter in &how_starters {
        if q.starts_with(starter) {
            let topic = q[starter.len()..].trim().to_string();
            return QueryIntent::Process { topic: clean_topic(&topic) };
        }
    }

    // ─── Why questions ────────────────────────────────
    let why_starters = ["why is ", "why are ", "why does ",
                        "why do ", "why did ", "why "];
    for starter in &why_starters {
        if q.starts_with(starter) {
            let topic = q[starter.len()..].trim().to_string();
            return QueryIntent::Why { topic: clean_topic(&topic) };
        }
    }

    // ─── Who questions ────────────────────────────────
    let who_starters = ["who is ", "who was ", "who are ",
                        "who invented ", "who discovered ", "who created "];
    for starter in &who_starters {
        if q.starts_with(starter) {
            let topic = q[starter.len()..].trim().to_string();
            return QueryIntent::Who { topic: clean_topic(&topic) };
        }
    }

    // ─── Think questions ───────────────────────────────
    // Hypotheticals, counterfactuals, possibility reasoning.
    // "what would happen if...", "could a fish...", "what if...",
    // "imagine...", "is it possible...", "would it work if..."
    if let Some(think) = try_parse_think(q) {
        return think;
    }

    // ─── Explore questions ────────────────────────────
    // Breadth-first knowledge dump requests.
    // "tell me everything about X", "what do you know about X",
    // "explore X", "what all do you know about X"
    if let Some(explore) = try_parse_explore(q) {
        return explore;
    }

    // ─── General: strip common prefixes ───────────────
    let general_prefixes = [
        "tell me about ", "explain ", "describe ",
        "talk about ", "what about ", "teach me about ",
        "tell me ", "show me ",
    ];
    for prefix in &general_prefixes {
        if q.starts_with(prefix) {
            let topic = q[prefix.len()..].trim().to_string();
            return QueryIntent::General { topic: clean_topic(&topic) };
        }
    }

    // Default: treat the whole query as a general topic
    QueryIntent::General { topic: clean_topic(q) }
}

/// Clean a topic string for better recall matching.
/// Strips articles, trailing filler, and noise words
/// that hurt keyword matching without adding semantic value.
fn clean_topic(raw: &str) -> String {
    let s = raw.trim()
        .trim_start_matches("the ")
        .trim_start_matches("a ")
        .trim_start_matches("an ")
        .trim_start_matches("about ")
        .trim_start_matches("do ")
        .trim_start_matches("explain ")
        .trim_start_matches("tell me about ")
        .trim_start_matches("help with ")
        .trim_end_matches(" yet")
        .trim_end_matches(" now")
        .trim_end_matches(" exactly")
        .trim_end_matches(" anyway")
        .trim();
    if s.is_empty() { raw.trim().to_string() } else { s.to_string() }
}

/// Try to parse a comparison query.
fn try_parse_comparison(q: &str) -> Option<QueryIntent> {
    // "difference between X and Y"
    if let Some(rest) = q.strip_prefix("difference between ")
        .or_else(|| q.strip_prefix("differences between "))
        .or_else(|| q.strip_prefix("what is the difference between "))
        .or_else(|| q.strip_prefix("what are the differences between "))
    {
        if let Some((a, b)) = rest.split_once(" and ") {
            return Some(QueryIntent::Comparison {
                topic_a: clean_topic(a),
                topic_b: clean_topic(b),
            });
        }
    }

    // "compare X and/with/to Y"
    if let Some(rest) = q.strip_prefix("compare ") {
        for sep in [" and ", " with ", " to ", " vs "] {
            if let Some((a, b)) = rest.split_once(sep) {
                return Some(QueryIntent::Comparison {
                    topic_a: clean_topic(a),
                    topic_b: clean_topic(b),
                });
            }
        }
    }

    // "X vs Y" / "X versus Y"
    for sep in [" vs ", " versus ", " vs. "] {
        if let Some((a, b)) = q.split_once(sep) {
            return Some(QueryIntent::Comparison {
                topic_a: clean_topic(a),
                topic_b: clean_topic(b),
            });
        }
    }

    // "how is X different from Y"
    if q.starts_with("how is ") || q.starts_with("how are ") {
        if let Some(pos) = q.find(" different from ") {
            let starter_len = if q.starts_with("how is ") { 7 } else { 8 };
            let a = &q[starter_len..pos];
            let b = &q[pos + 16..];
            return Some(QueryIntent::Comparison {
                topic_a: clean_topic(a),
                topic_b: clean_topic(b),
            });
        }
    }

    None
}

/// Try to parse a list/enumeration query.
fn try_parse_list(q: &str) -> Option<QueryIntent> {
    for starter in ["list ", "name ", "enumerate "] {
        if let Some(rest) = q.strip_prefix(starter) {
            let (count, topic) = extract_count(rest);
            return Some(QueryIntent::List {
                topic: clean_topic(&topic),
                count,
            });
        }
    }

    let list_patterns = [
        "what are the types of ", "what are some types of ",
        "what are the kinds of ", "what are some kinds of ",
        "what are some examples of ", "what are examples of ",
        "give me examples of ",
    ];
    for pat in &list_patterns {
        if let Some(rest) = q.strip_prefix(pat) {
            return Some(QueryIntent::List {
                topic: clean_topic(rest),
                count: None,
            });
        }
    }

    if q.starts_with("how many ") && (q.contains(" types of ") || q.contains(" kinds of ")) {
        let topic = q.trim_start_matches("how many ")
            .replace("types of ", "")
            .replace("kinds of ", "")
            .replace(" are there", "")
            .replace(" exist", "");
        return Some(QueryIntent::List {
            topic: clean_topic(&topic),
            count: None,
        });
    }

    None
}

/// Extract a count number from the start of a string.
fn extract_count(s: &str) -> (Option<usize>, String) {
    let words: Vec<&str> = s.split_whitespace().collect();
    if let Some(first) = words.first() {
        if let Ok(n) = first.parse::<usize>() {
            let rest = words[1..].join(" ");
            let rest = rest.trim_start_matches("types of ")
                .trim_start_matches("kinds of ")
                .trim_start_matches("examples of ")
                .to_string();
            return (Some(n), rest);
        }
    }
    (None, s.to_string())
}

/// Try to parse a Think query — hypotheticals, counterfactuals, reasoning.
fn try_parse_think(q: &str) -> Option<QueryIntent> {
    // "what would happen if X", "what if X"
    let think_starters = [
        "what would happen if ",
        "what happens if ",
        "what if ",
        "imagine ",
        "suppose ",
        "hypothetically ",
    ];
    for starter in &think_starters {
        if let Some(rest) = q.strip_prefix(starter) {
            return Some(QueryIntent::Think { topic: clean_topic(rest) });
        }
    }

    // "could a X do Y", "could X survive Y", "would X work"
    let conditional_starters = [
        "could a ", "could an ", "could ",
        "would a ", "would an ", "would ",
    ];
    for starter in &conditional_starters {
        if let Some(rest) = q.strip_prefix(starter) {
            // Skip "could you" (ability) — already handled earlier
            if rest.starts_with("you ") { return None; }
            return Some(QueryIntent::Think { topic: clean_topic(rest) });
        }
    }

    // "is it possible to/that/for X"
    if let Some(rest) = q.strip_prefix("is it possible ") {
        let topic = rest.trim_start_matches("to ")
            .trim_start_matches("that ")
            .trim_start_matches("for ");
        return Some(QueryIntent::Think { topic: clean_topic(topic) });
    }

    // "what happens when X", "what would X do if Y"
    if q.starts_with("what would ") && q.contains(" do ") {
        let topic = q.strip_prefix("what would ").unwrap();
        return Some(QueryIntent::Think { topic: clean_topic(topic) });
    }

    // "can X survive Y", "can X exist without Y" (not "can you")
    if q.starts_with("can ") && !q.starts_with("can you ") {
        let rest = &q[4..];
        if rest.contains(" survive ") || rest.contains(" exist ")
            || rest.contains(" live ") || rest.contains(" work ")
            || rest.contains(" without ") {
            return Some(QueryIntent::Think { topic: clean_topic(rest) });
        }
    }

    None
}

/// Try to parse an Explore query — breadth-first knowledge dump.
fn try_parse_explore(q: &str) -> Option<QueryIntent> {
    let explore_starters = [
        "tell me everything about ",
        "tell me all about ",
        "everything about ",
        "everything you know about ",
        "what do you know about ",
        "what all do you know about ",
        "what have you learned about ",
        "explore ",
        "deep dive into ",
        "deep dive on ",
    ];
    for starter in &explore_starters {
        if let Some(rest) = q.strip_prefix(starter) {
            return Some(QueryIntent::Explore { topic: clean_topic(rest) });
        }
    }

    // "all about X"
    if let Some(rest) = q.strip_prefix("all about ") {
        return Some(QueryIntent::Explore { topic: clean_topic(rest) });
    }

    None
}

/// Report of glossary/appendix cleanup.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlossaryCleanupReport {
    pub candidates_found: usize,
    pub nodes_revoked: usize,
    pub details: Vec<(String, String)>, // (label, reason)
}

/// Report of junk-node brain cleanup (PDF textbook garbage removal).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrainCleanupReport {
    pub page_refs: usize,
    pub textbook_filler: usize,
    pub formatting_artifacts: usize,
    pub too_short: usize,
    pub content_junk: usize,
    pub total_flagged: usize,
    pub revoked: usize,
    pub dry_run: bool,
    pub details: Vec<(String, String)>, // (label, category)
}

/// Report of self-modification actions taken.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelfModReport {
    pub bonds_strengthened: usize,
    pub bonds_created: usize,
    pub novel_associations: Vec<(String, String)>,
    pub strengthened_pairs: Vec<(String, String, f32)>,
}

impl Brain {
    /// Birth. Create a new brain. The terminal bond forms.
    /// This is the first memory.
    pub fn new(pattern_size: usize) -> Self {
        let (bond, _terminal_seed) = TerminalBond::initiate();

        // Pattern engine seeded from the AI's own identity
        let engine = PatternEngine::new(pattern_size, bond.ai_seed());

        Self {
            terminal_bond: bond,
            network: AssociativeNetwork::new(),
            pattern_engine: engine,
            storage_path: None,
            last_recall: None,
            conversation_history: Vec::new(),
            pending_proposals: HashMap::new(),
            pending_image_proposals: HashMap::new(),
            pending_audio_proposals: HashMap::new(),
            pending_video_proposals: HashMap::new(),
            active_goal: None,
            session_memory: session_memory::SessionMemory::new(),
            autonomy: autonomy::AutonomyEngine::new(),
            curiosity: curiosity::CuriosityEngine::new(),
            active_arena: None,
            invention: invention::InventionEngine::new(),
            self_improvement: self_improvement::SelfImprovementEngine::new(),
            tool_engine: tools::ToolEngine::default(),
            idle_explorer: web_idle::IdleExplorer::new(),
            computer_use: computer_use::ComputerUseEngine::new(),
            conversations: conversations::ConversationStore::default(),
            word_index: HashMap::new(),
            word_index_valid: false,
            phrase_index_cache: None,
            phrase_index_node_count: 0,
            pending_auto_research: false,
            curiosity_auto_resolve: false,
        }
    }

    /// Set the storage directory (uses PathBuf — cross-platform).
    pub fn set_storage(&mut self, path: PathBuf) {
        self.conversations = conversations::ConversationStore::new(&path);
        self.storage_path = Some(path);
    }

    /// Seed the brain with knowledge about its creator/inventor.
    /// Called on first creation so Lu always knows who built it.
    /// Idempotent — skips if creator knowledge already exists.
    pub fn seed_creator_knowledge(&mut self) -> usize {
        // Check if we already know about John Dean Martin
        if self.find_existing_content("John Dean Martin is the inventor of the DialectForge protocol and creator of Lu AI").is_some() {
            return 0;
        }

        let facts = [
            "John Dean Martin is the inventor of the DialectForge protocol and creator of Lu AI",
            "Lu AI was created by John Dean Martin as a graph-based associative intelligence",
            "The DialectForge protocol was invented by John Dean Martin to give AI identity through seed-based encoding",
            "John Dean Martin designed Lu to learn through spreading activation across knowledge graphs, not through language models",
            "Lu AI's terminal bond system was invented by John Dean Martin to create unforgeable AI identity",
        ];

        let mut seeds = Vec::new();
        for fact in &facts {
            let seed = self.learn_fast(fact, Confidence::Terminal, "creator_seed");
            seeds.push((fact.to_string(), seed));
        }

        // Create bonds between all creator facts for strong cluster
        for i in 0..seeds.len() {
            for j in (i + 1)..seeds.len() {
                let _ = self.associate(
                    &seeds[i].1, &seeds[j].1,
                    "creator_knowledge", 2.0, "creator_seed",
                );
            }
        }

        seeds.len()
    }

    /// Resize the pattern engine to new dimensions.
    /// Re-encodes ALL existing nodes through the new engine.
    /// This is a migration — call once, save, done.
    pub fn resize_dimensions(&mut self, new_size: usize) -> String {
        let old_size = self.pattern_engine.pattern_size();
        if old_size == new_size {
            return format!("Already at {} dimensions, nothing to do.", new_size);
        }

        // Create new engine from same seed, bigger dimensions
        let mut new_engine = PatternEngine::new(new_size, self.terminal_bond.ai_seed());

        // Re-encode every active node through the new engine
        let mut count = 0usize;
        for (_hex, node) in self.network.active_nodes_mut() {
            node.pattern = new_engine.encode(&node.label);
            count += 1;
        }

        // Swap in the new engine
        self.pattern_engine = new_engine;

        format!(
            "Resized pattern engine: {} -> {} dimensions. Re-encoded {} nodes.",
            old_size, new_size, count
        )
    }

    // ─── Learning ───────────────────────────────────────

    /// Learn a new piece of knowledge. Creates a node in the network.
    /// Returns the seed of the new knowledge atom.
    ///
    /// Dedup uses content identity (normalized string comparison),
    /// NOT pattern similarity. This ensures distinct facts always
    /// get separate nodes, regardless of the pattern engine's maturity.
    pub fn learn(
        &mut self,
        content: &str,
        confidence: Confidence,
        created_by: &str,
    ) -> Seed {
        // Check for duplicate content BEFORE encoding
        if let Some(existing) = self.find_existing_content(content) {
            return existing;
        }

        let pattern = self.pattern_engine.encode(content);

        let node = KnowledgeNode::new(
            NodeType::Fact,
            content.to_string(),
            pattern,
            confidence,
            created_by,
        );

        let seed = self.network.add_node(node);

        // Auto-associate with related knowledge (word overlap + pattern)
        // threshold=0.3 and max=3 keeps the graph sparse enough for
        // spreading activation to discriminate. Too many bonds (5+ per
        // node at low threshold) turns the graph into soup where
        // activation reaches everything equally.
        self.auto_associate(&seed, 0.3, 3);
        self.word_index_valid = false;
        self.phrase_index_cache = None;

        seed
    }

    /// Learn a fact WITHOUT auto-association. Used by batch_train
    /// where explicit associations are provided by the caller.
    /// Skipping auto_associate avoids O(N) scan of all nodes per item,
    /// making batch training fast even with large graphs.
    pub fn learn_fast(
        &mut self,
        content: &str,
        confidence: Confidence,
        created_by: &str,
    ) -> Seed {
        if let Some(existing) = self.find_existing_content(content) {
            return existing;
        }

        let pattern = self.pattern_engine.encode(content);

        let node = KnowledgeNode::new(
            NodeType::Fact,
            content.to_string(),
            pattern,
            confidence,
            created_by,
        );

        let seed = self.network.add_node(node);
        self.word_index_valid = false;
        self.phrase_index_cache = None;
        seed
    }

    // ─── Inverted word index ─────────────────────────────────────────────────
    //
    // Maps stemmed_word → sorted Vec<node_hex>.  Built lazily; O(N) once.
    // Subsequent keyword lookups become O(hits) instead of O(N).
    // Invalidated (word_index_valid = false) on every learn/revoke so the
    // next recall rebuilds it.  The rebuild itself is done in ensure_word_index.

    fn build_word_index(&mut self) {
        self.word_index.clear();

        // Collect (stem, hex) pairs for every word in every node label.
        // We'll sort and group them to build sorted Vecs per stem.
        let mut pairs: Vec<(String, String)> = Vec::new();

        for (hex, node) in self.network.active_nodes() {
            let hex_str = hex.to_string();
            for word in node.label.split_whitespace() {
                // Lowercase before stemming so index keys match lowercased query tokens.
                // Without this, "TCP" in labels indexes as "TCP" but queries look up "tcp".
                let word_lc = word.to_lowercase();
                let stem = Self::stem(&word_lc);
                if stem.len() > 1 {
                    pairs.push((stem.to_string(), hex_str.clone()));
                }
            }
        }

        // Sort by stem so we can group efficiently.
        pairs.sort_unstable_by(|a, b| a.0.cmp(&b.0));

        let mut i = 0;
        while i < pairs.len() {
            let stem = &pairs[i].0;
            let j = pairs.partition_point(|p| p.0 <= *stem); // exclusive end
            // Collect hexes for this stem, deduplicate, sort for binary_search
            let mut hexes: Vec<String> = pairs[i..j].iter().map(|p| p.1.clone()).collect();
            hexes.sort_unstable();
            hexes.dedup();
            self.word_index.insert(stem.clone(), hexes);
            i = j;
        }

        self.word_index_valid = true;
    }

    fn ensure_word_index(&mut self) {
        if !self.word_index_valid || self.word_index.is_empty() {
            self.build_word_index();
        }
    }

    /// Return the sorted hex list for a single stemmed word (fast path).
    /// Returns empty slice if word not in index.
    fn index_hexes_for_stem<'a>(&'a self, stem: &str) -> &'a [String] {
        self.word_index.get(stem).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Return nodes that contain ALL parts of a compound token (intersection).
    fn index_hexes_for_compound(&self, parts: &[&str]) -> Vec<String> {
        if parts.is_empty() { return Vec::new(); }
        // Start with the smallest candidate set.
        let sets: Vec<&[String]> = parts.iter()
            .map(|p| self.index_hexes_for_stem(Self::stem(p)))
            .collect();
        let (min_idx, _) = sets.iter().enumerate()
            .min_by_key(|(_, s)| s.len())
            .unwrap();
        let seed_set = sets[min_idx];
        // Filter to hexes present in ALL other sets (binary search on sorted Vecs).
        seed_set.iter()
            .filter(|hex| {
                sets.iter().enumerate().all(|(i, s)| {
                    i == min_idx || s.binary_search(*hex).is_ok()
                })
            })
            .cloned()
            .collect()
    }

    /// Create a bond between two knowledge atoms.
    /// Weight controls bond strength — higher weight means more
    /// activation flows through this bond during recall.
    /// Default 1.0. Vocabulary definitions use 3.0+ so the graph
    /// naturally prioritizes definitions over incidental word matches.
    pub fn associate(
        &mut self,
        seed_a: &Seed,
        seed_b: &Seed,
        description: &str,
        weight: f32,
        created_by: &str,
    ) -> Result<Seed, String> {
        let result = self.network.add_bond(
            seed_a, seed_b,
            description.to_string(),
            weight,
            created_by,
        );

        // Let pattern engine learn from the association
        if result.is_ok() {
            if let (Some(a), Some(b)) = (
                self.network.get_node(seed_a),
                self.network.get_node(seed_b),
            ) {
                self.pattern_engine.learn_from_association(
                    &a.label, &b.label, 0.01,
                );
            }
        }

        result
    }

    // ─── Recall ─────────────────────────────────────────

    /// Record a query in conversation history so subsequent recall() calls
    /// can inject context from prior turns. Called by tool_recall (which
    /// calls recall() directly, bypassing ask()). ask() pushes history
    /// itself after recall() returns, so this is only needed for raw recall.
    pub fn push_conversation_history(&mut self, query: &str) {
        let meaningful: Vec<&str> = query.split_whitespace()
            .filter(|w| w.len() >= 2)
            .collect();
        if !meaningful.is_empty() {
            self.conversation_history.push(query.to_string());
            if self.conversation_history.len() > 5 {
                self.conversation_history.remove(0);
            }
        }
    }

    /// Ask the brain a question. Recall through spreading activation.
    ///
    /// Uses two paths to find initial nodes:
    /// 1. Pattern similarity (DF-encoded vectors)
    /// 2. Keyword matching (words in query appear in node labels)
    ///
    /// Both contribute to the initial activation set. As the pattern
    /// engine learns more associations, path 1 gets stronger and
    /// path 2 matters less. This is the dialect evolving.
    pub fn recall(&mut self, query: &str, depth: usize, top_k: usize) -> RecallResult {
        use std::io::Write;
        let mut timing_log = std::fs::OpenOptions::new()
            .create(true).append(true)
            .open(r"brain\recall_timing.log")
            .ok();
        let start = std::time::Instant::now();
        {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(r"lu_debug.log") {
                let _ = writeln!(f, "[recall] query={:?} conv_history({})={:?}", query, self.conversation_history.len(), self.conversation_history);
            }
        }
        let pattern = self.pattern_engine.encode(query);

        // ─── Stage 1: Tokenize ──────────────────────────────
        // Build (or reuse) the phrase index from the graph's vocabulary.
        // PhraseIndex::build is O(N) — we cache it and only rebuild when the
        // node count changes, turning repeat calls into O(1).
        let current_node_count = self.network.stats().active_nodes;
        let phrase_index = if self.phrase_index_cache.is_none()
            || self.phrase_index_node_count != current_node_count
        {
            let idx = tokenizer::PhraseIndex::build(
                self.network.active_nodes().map(|(_, n)| n.label.as_str())
            );
            self.phrase_index_node_count = current_node_count;
            self.phrase_index_cache = Some(idx);
            self.phrase_index_cache.as_ref().unwrap()
        } else {
            self.phrase_index_cache.as_ref().unwrap()
        };
        let tokenized = tokenizer::tokenize(query, phrase_index);
        eprintln!("[recall] tokenize: {:?}", start.elapsed());
        if let Some(ref mut f) = timing_log { let _ = writeln!(f, "[recall] tokenize: {:?}", start.elapsed()); }

        // Extract content token texts for keyword matching.
        // Compound tokens stay as multi-word strings.
        // Strip query stop words (question/function words) before keyword
        // extraction so "What is neural network?" → ["neural", "network"].
        let query_words: Vec<&str> = tokenized.content_tokens.iter()
            .map(|t| t.text.as_str())
            .filter(|w| {
                let lc = w.to_lowercase();
                !QUERY_STOP_WORDS.contains(&lc.as_str())
            })
            .collect();

        // Detect acronyms from the ORIGINAL query (before lowercasing).
        // Words that are all-uppercase and 2-8 chars are technical acronyms
        // (TCP, DNS, SQL, HTTP, API, CSS, HTML, UDP, etc.).
        let acronym_words: HashSet<String> = query
            .split_whitespace()
            .filter_map(|w| {
                let trimmed = w.trim_matches(|c: char| !c.is_alphanumeric());
                if trimmed.len() >= 2
                    && trimmed.len() <= 8
                    && trimmed.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
                {
                    Some(trimmed.to_lowercase())
                } else {
                    None
                }
            })
            .collect();

        // Build a weight map: token text -> weight from tokenizer
        let token_weights: HashMap<String, f32> = tokenized.content_tokens.iter()
            .filter(|t| {
                let lc = t.text.to_lowercase();
                !QUERY_STOP_WORDS.contains(&lc.as_str())
            })
            .map(|t| (t.text.clone(), t.weight))
            .collect();

        let mut initial: HashMap<String, (Seed, f32)> = HashMap::new();

        // Build word index early — we need it to pre-filter pattern similarity
        // candidates, cutting the O(N×256) dot-product scan to O(~500×256).
        self.ensure_word_index();
        eprintln!("[recall] word_index: {:?}", start.elapsed());
        if let Some(ref mut f) = timing_log { let _ = writeln!(f, "[recall] word_index: {:?}", start.elapsed()); }

        // Collect the union of all nodes sharing at least one query word stem.
        // Pattern similarity will only run on these candidates, not all 52K+ nodes.
        let similarity_candidates: HashSet<String> = {
            let mut s = HashSet::new();
            for w in &query_words {
                let parts: Vec<&str> = w.split_whitespace().collect();
                if parts.len() > 1 {
                    for h in self.index_hexes_for_compound(&parts) {
                        s.insert(h.clone());
                    }
                } else {
                    for h in self.index_hexes_for_stem(Self::stem(w)) {
                        s.insert(h.clone());
                    }
                }
            }
            s
        };
        eprintln!("[recall] candidates: {:?}", start.elapsed());
        if let Some(ref mut f) = timing_log { let _ = writeln!(f, "[recall] candidates: {:?}", start.elapsed()); }

        // Path 1: Pattern similarity — filtered to word-index candidates.
        // At 52K+ nodes a full scan costs 52K×256 dot products per recall.
        // By restricting to nodes sharing ≥1 query stem we check only the
        // ~200–2000 plausibly-relevant nodes. Falls back to full scan when
        // the query produces no word-index candidates.
        let pattern_matches = self.network_node_similarities_filtered(&pattern, 0.92, &similarity_candidates);
        for (seed, sim) in pattern_matches.iter().take(15) {
            let hex = seed.to_hex();
            let entry = initial.entry(hex).or_insert((seed.clone(), 0.0));
            entry.1 += sim * 30.0; // modest pattern contribution
        }
        eprintln!("[recall] pattern_sim: {:?}", start.elapsed());
        if let Some(ref mut f) = timing_log { let _ = writeln!(f, "[recall] pattern_sim: {:?}", start.elapsed()); }

        // Path 1b: Cross-modal pattern similarity (Media nodes)
        //
        // Media nodes (images, audio, video) live in the same 256-dim
        // vector space but with a small modality offset (~0.02). This
        // means a text query about "ocean sunset" can activate an image
        // of an ocean sunset — but the offset means the text threshold
        // (0.92) would miss it. Use a lower threshold for Media nodes.
        // Media nodes are typically <1% of the graph so full scan is fine.
        let media_matches = self.network_media_similarities(&pattern, 0.80);
        for (seed, sim) in media_matches.iter().take(5) {
            let hex = seed.to_hex();
            let entry = initial.entry(hex).or_insert((seed.clone(), 0.0));
            entry.1 += sim * 25.0; // cross-modal contribution
        }
        eprintln!("[recall] media_sim: {:?}", start.elapsed());
        if let Some(ref mut f) = timing_log { let _ = writeln!(f, "[recall] media_sim: {:?}", start.elapsed()); }

        // Path 2: Keyword matching with inverse frequency weighting.
        //
        // Uses the inverted word index (stem → sorted Vec<hex>) so each
        // query word lookup is O(hits) instead of O(N).  Index already built
        // above; it is invalidated on learn/revoke.

        // Step 1: Count how many nodes contain each query word — O(Q).
        let total_nodes = self.network.node_count().max(1) as f32;
        let mut word_doc_counts: HashMap<String, usize> = HashMap::new();

        for word in &query_words {
            let parts: Vec<&str> = word.split_whitespace().collect();
            let count = if parts.len() > 1 {
                self.index_hexes_for_compound(&parts).len()
            } else {
                self.index_hexes_for_stem(Self::stem(word)).len()
            };
            word_doc_counts.insert(word.to_string(), count);
        }
        eprintln!("[recall] word_counts: {:?}", start.elapsed());
        if let Some(ref mut f) = timing_log { let _ = writeln!(f, "[recall] word_counts: {:?}", start.elapsed()); }

        // Frequency-based word filtering with soft penalty.
        //
        // Words appearing in >8% of nodes are "structurally common" and
        // less discriminative. But we NEVER hard-drop ALL query words —
        // a single-word query like "above" must still work even if the
        // word appears in many nodes. Instead:
        //
        // 1. Words below the cutoff pass through normally.
        // 2. Words above the cutoff get a penalty multiplier (0.1-0.5)
        //    instead of being dropped entirely.
        // 3. If ALL words would be dropped, keep the least-common one.
        //
        let frequency_cutoff = (total_nodes * 0.08).max(3.0) as usize;

        // Track which words are "high frequency" for penalty later.
        let mut high_freq_words: HashSet<String> = HashSet::new();
        let mut surviving_words: Vec<&str> = Vec::new();
        let mut dropped_words: Vec<(&str, usize)> = Vec::new();

        for w in &query_words {
            let count = *word_doc_counts.get(*w).unwrap_or(&0);
            if count == 0 {
                // Word not found in any node — drop it (no matches possible).
                continue;
            } else if count <= frequency_cutoff {
                surviving_words.push(w);
            } else {
                // High frequency — mark for penalty but don't drop yet.
                high_freq_words.insert(w.to_string());
                dropped_words.push((w, count));
            }
        }

        // Safety net: if ALL words were dropped or high-freq, keep
        // the least-common ones so the query still has signal.
        let rescued_from_drop = surviving_words.is_empty() && !dropped_words.is_empty();
        if rescued_from_drop {
            // Sort by frequency ascending — least common first.
            dropped_words.sort_by_key(|(_, count)| *count);
            // Keep at least the least-common word (or up to 3 if short query).
            let keep = dropped_words.len().min(3);
            for (w, _) in &dropped_words[..keep] {
                surviving_words.push(w);
                // These are still marked as high_freq so they'll get
                // penalty weighting, but at least they participate.
            }
        }

        let query_words = surviving_words;

        // ─── IDF weighting from graph topology ──────────────────────────────────
        // Count how many node labels contain each keyword as a case-insensitive
        // substring. Rare/specific keywords (few matches) get weight ~1.0; common
        // keywords (many matches) are dampened via 1/sqrt(count). This complements
        // stop-word filtering: stop words handle zero-information words; topology
        // IDF handles the gray area ("systems", "formal", "design" etc.).
        let mut keyword_topo_idf: HashMap<String, f32> = HashMap::new();
        let mut keyword_substring_counts: HashMap<String, usize> = HashMap::new();
        {
            for word in &query_words {
                let word_lc = word.to_lowercase();
                let count = self.network.active_nodes()
                    .filter(|(_, n)| n.label.to_lowercase().contains(&word_lc))
                    .count();
                keyword_substring_counts.insert(word.to_string(), count);
                // Acronyms detected from original query get special treatment
                if acronym_words.contains(&word_lc) && count > 0 {
                    // Acronyms are specific technical terms — never treat as dynamic
                    // stop words, and give them a minimum weight floor of 0.5.
                    let weight = (1.0_f32 / (count as f32).sqrt()).max(0.5);
                    keyword_topo_idf.insert(word.to_string(), weight);
                } else if count > IDF_MATCH_CEILING {
                    // Dynamic stop word: matches too many nodes → weight 0.0 (no insertion)
                } else if count > 0 {
                    let weight = 1.0_f32 / (count as f32).sqrt();
                    keyword_topo_idf.insert(word.to_string(), weight);
                }
                // count == 0: keyword absent from all node labels; no entry → topo_idf=0 → skip
            }
            // Normalize so max weight = 1.0
            let max_w = keyword_topo_idf.values().cloned().fold(0.0_f32, f32::max);
            if max_w > 0.0 {
                for w in keyword_topo_idf.values_mut() {
                    *w /= max_w;
                }
            }
            // Debug log
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(r"lu_debug.log") {
                for word in &query_words {
                    let count = keyword_substring_counts.get(*word).copied().unwrap_or(0);
                    let weight = keyword_topo_idf.get(*word).copied().unwrap_or(0.0);
                    if count > IDF_MATCH_CEILING {
                        let _ = writeln!(f, "[idf_floor] keyword={:?} matches={} -> weight=0.0 (exceeds {} threshold)", word, count, IDF_MATCH_CEILING);
                    } else {
                        let _ = writeln!(f, "[idf_weight] keyword={:?} matches={} weight={:.2}", word, count, weight);
                    }
                }
            }
        }
        eprintln!("[recall] topo_idf: {:?}", start.elapsed());
        if let Some(ref mut f) = timing_log { let _ = writeln!(f, "[recall] topo_idf: {:?}", start.elapsed()); }

        // Step 2: Score keyword-matching nodes via the inverted index — O(Q×hits).
        //
        // Precompute max_idf² (denominator for normalization) and per-word
        // score contributions; then iterate only matching nodes.
        let max_idf: f32 = query_words.iter()
            .map(|w| {
                let dc = *word_doc_counts.get(*w).unwrap_or(&1) as f32;
                let idf = (total_nodes / (dc + 1.0)).ln().max(0.1);
                idf * idf
            })
            .sum::<f32>()
            .max(0.01);

        let total_query_words = query_words.len().max(1) as f32;
        let keyword_multiplier = if rescued_from_drop { 40.0 } else { 3.0 };

        // Per-node accumulation: (raw_idf2_score, word_match_count).
        let mut node_kw: HashMap<String, (f32, usize)> = HashMap::new();

        for word in &query_words {
            let parts: Vec<&str> = word.split_whitespace().collect();
            let doc_count = *word_doc_counts.get(*word).unwrap_or(&1) as f32;
            let idf = (total_nodes / (doc_count + 1.0)).ln().max(0.1);
            let token_weight = token_weights.get(*word).copied().unwrap_or(1.0);
            let freq_penalty = if high_freq_words.contains(*word) {
                let ratio = doc_count / total_nodes;
                (0.4 - ratio * 0.3).max(0.1)
            } else {
                1.0
            };
            // Topology IDF weight: 1/sqrt(substring_match_count), normalized.
            // Keywords matching thousands of nodes get a low weight; rare keywords
            // matching a handful get weight ~1.0. If count==0, skip entirely.
            let topo_idf = keyword_topo_idf.get(*word).copied().unwrap_or(0.0);
            if topo_idf == 0.0 {
                continue; // No node labels contain this keyword as a substring
            }
            // Acronym boost: words identified as uppercase acronyms in the original
            // query (TCP, DNS, SQL, HTTP, API, etc.) are inherently specific technical
            // terms. Give them a significant multiplier. If the entire query IS a
            // single acronym, treat it as exact-match-priority with an even larger boost.
            let acronym_boost = if acronym_words.contains(*word) {
                if query_words.len() == 1 { 8.0_f32 } else { 4.0_f32 }
            } else {
                1.0_f32
            };
            let word_contrib = idf * idf * token_weight * freq_penalty * topo_idf * acronym_boost;

            // Get matching hex list from index.
            let owned_compound: Vec<String>;
            let matching: &[String] = if parts.len() > 1 {
                owned_compound = self.index_hexes_for_compound(&parts);
                &owned_compound
            } else {
                self.index_hexes_for_stem(Self::stem(word))
            };

            for hex in matching {
                let e = node_kw.entry(hex.clone()).or_insert((0.0, 0));
                e.0 += word_contrib;
                e.1 += 1;
            }
        }
        eprintln!("[recall] keyword_score: {:?}", start.elapsed());
        if let Some(ref mut f) = timing_log { let _ = writeln!(f, "[recall] keyword_score: {:?}", start.elapsed()); }

        // Apply coverage boost, normalize, and merge into `initial`.
        for (hex, (raw_score, words_matched)) in &node_kw {
            let coverage = *words_matched as f32 / total_query_words;
            // Deprioritize nodes matching only 1 of 3+ query words — they are likely
            // false positives (e.g. "solar panel" matching query "solar system planets").
            let single_word_penalty = if *words_matched == 1 && total_query_words >= 3.0 {
                0.2
            } else {
                1.0
            };
            // Boost nodes where ALL query words matched — full phrase hit.
            // "solar system" matching query "solar system" scores 2× over partial matches.
            let phrase_bonus = if *words_matched as f32 >= total_query_words && total_query_words >= 2.0 {
                2.0
            } else {
                1.0
            };
            let score = (raw_score / max_idf) * (coverage * coverage) * keyword_multiplier
                * single_word_penalty * phrase_bonus;
            if score > 0.0 {
                let entry = initial.entry(hex.clone());
                entry.and_modify(|(_, existing)| {
                    *existing = existing.max(score) + 0.2;
                }).or_insert_with(|| {
                    if let Ok(seed) = Seed::from_hex(hex) {
                        (seed, score)
                    } else {
                        (Seed::generate(), 0.0)
                    }
                });
            }
        }

        // ─── Topic seeds deferred to Pass 2 (after spread activation) ───
        // Topic seeds from prior turn are NOT injected here — that would
        // contaminate Pass 1 (pure comprehension of current query).
        // Instead, they are read AFTER spread activation and used to boost
        // nodes that overlap with both the query AND the conversation.

        // Keyword-matched set: nodes that directly matched at least one query word.
        // Used downstream to gate traversal-only results.
        let keyword_matched: HashSet<String> = node_kw.into_keys()
            .filter(|hex| initial.contains_key(hex))
            .collect();

        // All direct matches (keyword + pattern) for injection later
        let direct_matches: HashMap<String, f32> = initial.iter()
            .map(|(hex, (_, score))| (hex.clone(), *score))
            .collect();

        let mut activation_set: Vec<(Seed, f32)> = initial.into_values()
            .filter(|(_, s)| *s > 0.0)
            .collect();
        // Cap initial seed set: common words like "loop" match thousands of
        // nodes. Spreading from all of them is O(N*K) — too slow at 52K+.
        // Keep only the top 100 by activation score; the rest are noise.
        if activation_set.len() > 100 {
            activation_set.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            activation_set.truncate(100);
        }
        eprintln!("[recall] seed_set({} seeds): {:?}", activation_set.len(), start.elapsed());
        if let Some(ref mut f) = timing_log { let _ = writeln!(f, "[recall] seed_set({} seeds): {:?}", activation_set.len(), start.elapsed()); }

        // Get more results than needed so we have room to inject direct matches
        let spread_k = (top_k * 3).max(15);
        let mut result = self.network.spread_activation(&activation_set, depth, spread_k);
        eprintln!("[recall] spread: {:?}", start.elapsed());
        if let Some(ref mut f) = timing_log { let _ = writeln!(f, "[recall] spread: {:?}", start.elapsed()); }

        // ─── Hub penalty: dampen high-bond-count nodes ───────────────────────
        // Generic hub nodes (500+ bonds) can score 250-263 through sheer
        // connectivity, drowning out relevant content scoring ~150-180.
        // Divide accumulated activation by ln(bond_count) for nodes with
        // 100+ bonds. This lets hub nodes still appear if directly seeded
        // by query keywords (they'll get the IDF boost afterward) but prevents
        // them dominating through accumulated neighbor activation alone.
        const HUB_PENALTY_THRESHOLD: usize = 100;
        {
            use std::io::Write;
            for node in &mut result.activated_nodes {
                if let Some(bond_count) = self.network.bond_count_for(&node.seed_hex) {
                    if bond_count >= HUB_PENALTY_THRESHOLD {
                        let raw_score = node.strength;
                        let penalty_factor = (bond_count as f32).ln().max(1.0);
                        node.strength = raw_score / penalty_factor;
                        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(r"lu_debug.log") {
                            let _ = writeln!(f, "[hub_penalty] label={:?} raw={:.2} penalized={:.2} bonds={}", node.label, raw_score, node.strength, bond_count);
                        }
                    }
                }
            }
        }
        eprintln!("[recall] hub_penalty: {:?}", start.elapsed());
        if let Some(ref mut f) = timing_log { let _ = writeln!(f, "[recall] hub_penalty: {:?}", start.elapsed()); }

        // Keyword relevance gate for traversal-only results.
        //
        // Spreading activation follows bonds and returns neighbors of
        // keyword-matched seeds. But those neighbors are often early,
        // heavily-bonded nodes ("A fish lives in water") that have nothing
        // to do with the query. Without this gate, high-connectivity nodes
        // dominate every query through sheer bond count.
        //
        // Fix: if a node was NOT a direct keyword match, check whether
        // its label contains ANY query words. If zero match, crush its
        // score so it sinks below direct matches. Nodes matching at least
        // one word keep their traversal score (they're legitimately related
        // through both structure AND content).
        //
        // RESCUE MODE: When all query words were rescued from the frequency
        // filter, the query word appears in THOUSANDS of node labels. The
        // standard gate passes almost everything. In this case, apply a
        // stricter "definitional" filter — only keep nodes where the query
        // word appears in the first 3 words (subject position) or the label
        // contains definitional verbs like "means", "is a".
        if !query_words.is_empty() {
            for node in &mut result.activated_nodes {
                if !keyword_matched.contains(&node.seed_hex) {
                    // Not a keyword match (could be pattern-only or traversal) — check relevance
                    let label_lower = node.label.to_lowercase();
                    let label_ws: HashSet<String> = label_lower
                        .split_whitespace()
                        .map(|w| Self::stem(w).to_string())
                        .collect();
                    let hits = query_words.iter()
                        .filter(|w| {
                            let parts: Vec<&str> = w.split_whitespace().collect();
                            if parts.len() > 1 {
                                parts.iter().all(|p| label_ws.contains(Self::stem(p)))
                            } else {
                                label_ws.contains(Self::stem(*w))
                            }
                        })
                        .count();
                    if hits == 0 {
                        node.strength = 1.0;
                    } else if rescued_from_drop {
                        // RESCUE MODE: word is high-frequency, so containing
                        // it is not special. Apply strict definitional filter.
                        let label_words_vec: Vec<&str> = label_lower.split_whitespace().collect();
                        let is_definitional = query_words.iter().any(|w| {
                            let stemmed = Self::stem(w);
                            // Word in first 3 positions (subject/topic position)
                            let early = label_words_vec.iter().take(3)
                                .any(|lw| Self::stem(lw) == stemmed);
                            early
                        }) || label_lower.contains(" means ")
                           || label_lower.contains(" is a ")
                           || label_lower.contains(" is an ")
                           || label_lower.contains(" refers to ")
                           || label_lower.contains(" defined as ");
                        if !is_definitional {
                            // Not a definition, not leading with the word —
                            // probably incidental mention. Crush it.
                            node.strength = 1.0;
                        } else {
                            let cov = hits as f32 / query_words.len().max(1) as f32;
                            node.strength *= cov;
                        }
                    } else {
                        // Partial match — scale by coverage so nodes with
                        // more query words rank higher.
                        let cov = hits as f32 / query_words.len().max(1) as f32;
                        node.strength *= cov;
                    }
                }
            }
        }
        eprintln!("[recall] gate: {:?}", start.elapsed());
        if let Some(ref mut f) = timing_log { let _ = writeln!(f, "[recall] gate: {:?}", start.elapsed()); }

        // Inject direct matches not already in results
        for (hex, score) in &direct_matches {
            let already_present = result.activated_nodes.iter()
                .any(|n| n.seed_hex == *hex);
            if !already_present {
                if let Some(node) = self.network.get_node_by_hex(hex) {
                    result.activated_nodes.push(ActivatedNode {
                        seed_hex: hex.clone(),
                        label: node.label.clone(),
                        strength: score * 1.5,
                        contribution: 0.0,
                        confidence: node.confidence.clone(),
                    });
                }
            }
        }

        // IDF boost + definition preference: applied AFTER injection so all
        // keyword matches (both spread-found and injected) get the boost.
        // This ensures keyword matches always outrank traversal noise.
        //
        // Definition preference: when many nodes match the same keyword,
        // prefer nodes that look like definitions (keyword appears early,
        // contains "is a", "are", "means", etc.). This prevents a query
        // like "what is a noun" from returning "Italian nouns have gender"
        // instead of "A noun is a word that names a person, place, or thing."
        for node in &mut result.activated_nodes {
            if keyword_matched.contains(&node.seed_hex) {
                let label_lower = node.label.to_lowercase();
                let label_ws: HashSet<String> = label_lower
                    .split_whitespace()
                    .map(|w| Self::stem(w).to_string())
                    .collect();
                let mut idf_boost: f32 = 0.0;
                for word in &query_words {
                    let parts: Vec<&str> = word.split_whitespace().collect();
                    let matches = if parts.len() > 1 {
                        parts.iter().all(|p| label_ws.contains(Self::stem(p)))
                    } else {
                        label_ws.contains(Self::stem(word))
                    };
                    if matches {
                        let dc = *word_doc_counts.get(*word).unwrap_or(&1) as f32;
                        let idf = (total_nodes / (dc + 1.0)).ln().max(0.1);
                        idf_boost += idf * idf;
                    }
                }
                // Minimum boost of 50 ensures keyword matches always
                // outrank spreading activation noise (~10-20).
                // In RESCUE MODE, only definitional nodes get the full
                // boost — incidental mentions get a smaller boost.
                if rescued_from_drop {
                    let label_words_lc: Vec<&str> = label_lower.split_whitespace().collect();
                    let is_def = query_words.iter().any(|w| {
                        let stemmed = Self::stem(w);
                        label_words_lc.iter().take(3)
                            .any(|lw| Self::stem(lw) == stemmed)
                    }) || label_lower.contains(" means ")
                       || label_lower.contains(" is a ")
                       || label_lower.contains(" is an ");
                    if is_def {
                        node.strength += idf_boost.max(80.0); // stronger for definitions
                    } else {
                        node.strength += idf_boost.max(5.0); // minimal for incidental
                    }
                } else {
                    node.strength += idf_boost.max(50.0);
                }

                // Definition preference bonus (breaks ties among same-IDF nodes)
                let label_words: Vec<&str> = label_lower.split_whitespace().collect();
                let mut def_bonus: f32 = 0.0;

                // Bonus 1: keyword appears in first 5 words (definitional position)
                // "A noun is a word that..." — "noun" at position 1 → high bonus
                // "Italian nouns have gender" — "nouns" at position 1 → also ok
                // "In Hebrew, many nouns..." — "nouns" at position 3 → some bonus
                // Compound tokens: check if ANY part appears early
                for qw in &query_words {
                    let parts: Vec<&str> = qw.split_whitespace().collect();
                    if parts.len() > 1 {
                        // Compound: find earliest position of any part
                        for part in &parts {
                            for (pos, lw) in label_words.iter().enumerate().take(5) {
                                if Self::stem(lw) == Self::stem(part) {
                                    def_bonus += 10.0 - pos as f32;
                                    break; // Only count first position per part
                                }
                            }
                        }
                    } else {
                        for (pos, lw) in label_words.iter().enumerate().take(5) {
                            if Self::stem(lw) == Self::stem(qw) {
                                def_bonus += 10.0 - pos as f32;
                            }
                        }
                    }
                }

                // Bonus 2: contains definitional verbs ("is a", "are", "means", "refers to").
                // Skip the bonus for contrastive statements: "opposite of X", "contrary to X", etc.
                // such nodes describe what something is NOT and should not rank as definitions.
                let contrastive_markers = [
                    "opposite of", "contrary to", "unlike", "not to be confused",
                    "contrast with", "contrast to", "different from", "as opposed to",
                    "in contrast to", "not the same as",
                ];
                let is_contrastive = contrastive_markers.iter().any(|m| {
                    if let Some(pos) = label_lower.find(m) {
                        // Contrastive marker followed by a query keyword
                        let after = &label_lower[pos + m.len()..];
                        query_words.iter().any(|w| w.len() > 2 && after.contains(*w))
                    } else {
                        false
                    }
                });
                if !is_contrastive {
                    if label_lower.contains(" is a ") || label_lower.contains(" is an ") {
                        def_bonus += 8.0;
                    }
                    if label_lower.contains(" are ") || label_lower.contains(" means ")
                        || label_lower.contains(" refers to ") || label_lower.contains(" defined as ") {
                        def_bonus += 5.0;
                    }
                } else {
                    // Contrastive node: subtract bonus to push it below real definitions.
                    def_bonus -= 15.0;
                }

                // Bonus 3: shorter sentences tend to be more definitional
                // "A noun is a word" (5 words) > "In many Romance languages, nouns..." (8+ words)
                if label_words.len() <= 15 {
                    def_bonus += 3.0;
                }

                node.strength += def_bonus;
            }
        }

        // Re-sort, truncate, recompute percentages
        result.activated_nodes.sort_by(|a, b| {
            b.strength.partial_cmp(&a.strength).unwrap_or(std::cmp::Ordering::Equal)
        });
        result.activated_nodes.truncate(top_k);

        let total: f32 = result.activated_nodes.iter().map(|n| n.strength).sum();
        let total = if total > 0.0 { total } else { 1.0 };
        for node in &mut result.activated_nodes {
            node.contribution = node.strength / total;
        }

        // ─── Pass 2: Topic-seed context blending (after spread activation) ───
        // Now that Pass 1 has determined "what Lu thinks this query is about",
        // read topic seeds from the prior turn and boost nodes that overlap
        // with BOTH the current query results AND the prior conversation.
        // Seeds that don't overlap are ignored — they're from a different thread.
        {
            use std::io::Write;
            // Log Pass 1 top-5
            {
                let pass1_top5: Vec<String> = result.activated_nodes.iter()
                    .take(5)
                    .map(|n| format!("{}({:.2})", n.label, n.strength))
                    .collect();
                eprintln!("[recall] pass1: top5 activated = {:?}", pass1_top5);
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(r"lu_debug.log") {
                    let _ = writeln!(f, "[recall] pass1: top5 activated = {:?}", pass1_top5);
                }
            }

            let topic_seeds = self.session_memory.get_topic_seeds(10);
            let topic_seed_map: HashMap<String, f32> = topic_seeds.iter()
                .map(|ts| (ts.seed_hex.clone(), ts.score))
                .collect();

            if !topic_seed_map.is_empty() {
                // Build set of Pass 1 activated node hexes for fast lookup
                let pass1_hexes: HashSet<String> = result.activated_nodes.iter()
                    .map(|n| n.seed_hex.clone())
                    .collect();

                let mut seeds_checked = 0usize;
                let mut seeds_overlapping = 0usize;
                let mut seeds_boosted = 0usize;

                for (seed_hex, seed_score) in &topic_seed_map {
                    seeds_checked += 1;
                    // Direct overlap: topic seed is in the Pass 1 results
                    if pass1_hexes.contains(seed_hex) {
                        seeds_overlapping += 1;
                        if let Some(node) = result.activated_nodes.iter_mut()
                            .find(|n| n.seed_hex == *seed_hex)
                        {
                            node.strength += seed_score * 0.5;
                            seeds_boosted += 1;
                        }
                        continue;
                    }
                    // Neighbor overlap: check if any immediate neighbor of the
                    // topic seed appears in the Pass 1 results
                    let neighbors = self.network.node_bonds(seed_hex);
                    let neighbor_in_pass1 = neighbors.iter()
                        .any(|(neighbor_hex, _, _)| pass1_hexes.contains(neighbor_hex));
                    if neighbor_in_pass1 {
                        seeds_overlapping += 1;
                        // Boost ALL Pass 1 nodes that are neighbors of this topic seed
                        let neighbor_hexes: HashSet<String> = neighbors.iter()
                            .map(|(h, _, _)| h.clone())
                            .collect();
                        for node in result.activated_nodes.iter_mut() {
                            if neighbor_hexes.contains(&node.seed_hex) {
                                node.strength += seed_score * 0.5;
                                seeds_boosted += 1;
                            }
                        }
                    }
                    // No overlap: ignore this topic seed entirely
                }

                // Re-sort after boosting
                result.activated_nodes.sort_by(|a, b| {
                    b.strength.partial_cmp(&a.strength).unwrap_or(std::cmp::Ordering::Equal)
                });
                // Recompute contribution percentages
                let total: f32 = result.activated_nodes.iter().map(|n| n.strength).sum();
                let total = if total > 0.0 { total } else { 1.0 };
                for node in &mut result.activated_nodes {
                    node.contribution = node.strength / total;
                }

                // Log Pass 2 results
                {
                    let pass2_top5: Vec<String> = result.activated_nodes.iter()
                        .take(5)
                        .map(|n| format!("{}({:.2})", n.label, n.strength))
                        .collect();
                    eprintln!("[recall] pass2: topic_seeds checked={}, overlapping={}, boosted={}", seeds_checked, seeds_overlapping, seeds_boosted);
                    eprintln!("[recall] pass2: final top5 = {:?}", pass2_top5);
                    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(r"lu_debug.log") {
                        let _ = writeln!(f, "[recall] pass2: topic_seeds checked={}, overlapping={}, boosted={}", seeds_checked, seeds_overlapping, seeds_boosted);
                        let _ = writeln!(f, "[recall] pass2: final top5 = {:?}", pass2_top5);
                    }
                }
            }
        }
        eprintln!("[recall] pass2: {:?}", start.elapsed());
        if let Some(ref mut f) = timing_log { let _ = writeln!(f, "[recall] pass2: {:?}", start.elapsed()); }

        // ─── Left Brain: Session Context ─────────────────────
        // Check session memory for relevant working context.
        // Results are tagged [SESSION] so callers can always distinguish
        // them from permanent Right Brain fact nodes.
        // THE WALL: session context appears here for reasoning but
        // is NOT added to activated_nodes and cannot auto-become a node.
        self.session_memory.add_context(query);
        let session_hits = self.session_memory.get_relevant_context(&pattern, 5);
        for (id, content, source, confidence, _sim) in session_hits {
            result.scar_warnings.push(format!(
                "[SESSION:{id}] (source={source}, conf={:.2}) {content}",
                confidence
            ));
        }

        // Curiosity: analyse recall for knowledge gaps
        // Extract domain from query (first word with len > 2)
        let domain = query.split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())
            .find(|w| w.len() > 2)
            .unwrap_or_else(|| "general".to_string());
        self.curiosity.after_recall(&result, query, &domain);

        // Auto-resolve: if enabled and curiosity detected any gaps during this
        // recall, flag for background resolution. The flag is drained by
        // drain_auto_resolve() which the MCP worker calls AFTER the response
        // is sent — never blocking.
        if self.curiosity.auto_resolve && self.curiosity.wonder_queue.pending_count() > 0 {
            self.pending_auto_research = true;
        }

        // Invention: detect cross-domain bridge events
        // Use targeted lookups — only activated nodes, not the full graph
        let inv_bond_counts: HashMap<String, usize> = result.activated_nodes.iter()
            .filter_map(|n| {
                self.network.bond_count_for(&n.seed_hex)
                    .map(|c| (n.seed_hex.clone(), c))
            })
            .collect();
        let inv_node_labels: HashMap<String, String> = result.activated_nodes.iter()
            .filter_map(|n| {
                self.network.get_node_by_hex(&n.seed_hex)
                    .map(|node| (n.seed_hex.clone(), node.label.clone()))
            })
            .collect();
        self.invention.after_recall(&result, &inv_bond_counts, &inv_node_labels);

        // Self-improvement: record performance metric
        let gap_count = self.curiosity.wonder_queue.pending_count();
        let bridge_count = self.invention.bridge_history.len();
        self.self_improvement.record_query(self_improvement::QueryMetric {
            query: query.to_string(),
            activated_nodes: result.activated_nodes.len(),
            traversal_hops: result.traversal_path.len(),
            top_strength: result.activated_nodes.first().map(|n| n.strength).unwrap_or(0.0),
            gap_count,
            bridge_count,
            duration_ms: 0, // TODO: actual timing
            timestamp: chrono::Utc::now(),
        });

        self.last_recall = Some(result.clone());
        result
    }



    // ─── Revocation ─────────────────────────────────────

    /// Revoke a specific seed. Returns revocation report.
    pub fn revoke(&mut self, seed: &Seed, reason: &str) -> RevocationReport {
        let report = self.network.revoke(seed, reason);
        self.word_index_valid = false;
        self.phrase_index_cache = None;
        report
    }

    // ─── Proposals ──────────────────────────────────────

    /// Propose learning external content. Returns proposal for review.
    pub fn propose(&mut self, content: &str, source: &str) -> LearningProposal {
        let pattern = self.pattern_engine.encode(content);
        let id = Seed::generate().short();

        let similar = self.network_node_similarities(&pattern, 0.4);
        let similar_existing: Vec<(String, String, f32)> = similar.iter()
            .take(5)
            .filter_map(|(seed, sim)| {
                self.network.get_node(seed).map(|n| {
                    (seed.to_hex(), n.label.clone(), *sim)
                })
            })
            .collect();

        // Check for scar conflicts (re-encode + word overlap)
        let scar_conflicts = self.check_scar_conflicts(content);

        let recommendation = if scar_conflicts.is_empty() {
            "learn".to_string()
        } else {
            "REVIEW - matches revocation scar".to_string()
        };

        let proposal = LearningProposal {
            id: id.clone(),
            content: content.to_string(),
            source: source.to_string(),
            similar_existing,
            scar_conflicts,
            recommendation,
        };

        self.pending_proposals.insert(id, proposal.clone());
        proposal
    }

    /// Approve a pending proposal.
    pub fn approve_proposal(&mut self, id: &str) -> Option<Seed> {
        let proposal = self.pending_proposals.remove(id)?;
        Some(self.learn(&proposal.content, Confidence::Unverified, "external"))
    }

    /// Reject a pending proposal.
    pub fn reject_proposal(&mut self, id: &str) -> bool {
        self.pending_proposals.remove(id).is_some()
    }

    /// List all pending proposal IDs.
    pub fn pending_proposal_ids(&self) -> Vec<String> {
        self.pending_proposals.keys().cloned().collect()
    }

    /// Approve ALL pending proposals (skips scar conflicts).
    pub fn approve_all_pending(&mut self) -> Vec<(String, Seed)> {
        let ids: Vec<String> = self.pending_proposals.keys().cloned().collect();
        self.approve_all_ready(&ids)
    }

    // ─── Image Perception ───────────────────────────────

    /// Propose learning an image. The raw bytes are wrapped as GatedData
    /// immediately — the seed gates access from birth. Returns a proposal
    /// for review. Nothing is learned until approved.
    ///
    /// `label` is a human-readable descriptor (e.g. "sunset over ocean").
    /// The image bytes are encoded through the pattern engine to produce
    /// a 256-dim vector in the same space as text patterns.
    pub fn propose_image(
        &mut self,
        raw_bytes: &[u8],
        label: &str,
        source: &str,
    ) -> Result<ImageProposal, String> {
        // Encode the image through the pattern engine
        let (pattern, features) = self.pattern_engine.encode_image(raw_bytes)
            .ok_or_else(|| "Failed to decode image bytes".to_string())?;

        let id = Seed::generate().short();

        // Wrap raw bytes as GatedData — seed gates access from birth
        let gated_bytes = GatedData::create(raw_bytes);

        // Find similar existing nodes (cross-modal — could match text OR images)
        let similar = self.network_node_similarities(&pattern, 0.4);
        let similar_existing: Vec<(String, String, f32)> = similar.iter()
            .take(5)
            .filter_map(|(seed, sim)| {
                self.network.get_node(seed).map(|n| {
                    (seed.to_hex(), n.label.clone(), *sim)
                })
            })
            .collect();

        // Check scar conflicts using the label
        let scar_conflicts = self.check_scar_conflicts(label);

        let recommendation = if scar_conflicts.is_empty() {
            "learn".to_string()
        } else {
            "REVIEW - matches revocation scar".to_string()
        };

        let features_summary = format!(
            "{}x{}, brightness={:.2}, contrast={:.2}, {} images encoded",
            features.width, features.height,
            features.mean_brightness, features.contrast,
            self.pattern_engine.images_encoded()
        );

        let proposal = ImageProposal {
            id: id.clone(),
            label: label.to_string(),
            source: source.to_string(),
            gated_bytes,
            features_summary,
            similar_existing,
            scar_conflicts,
            recommendation,
        };

        self.pending_image_proposals.insert(id, proposal.clone());
        Ok(proposal)
    }

    /// Approve a pending image proposal. Creates a Media node in the
    /// network with the image's 256-dim pattern vector.
    /// Stores image metadata (dimensions, filename, file size) on the node.
    pub fn approve_image(&mut self, id: &str) -> Option<Seed> {
        let proposal = self.pending_image_proposals.remove(id)?;

        // Re-encode to get the pattern (proposals don't store the vector)
        let raw_bytes = proposal.gated_bytes.access(&proposal.gated_bytes.seed)?;
        let (pattern, features) = self.pattern_engine.encode_image(&raw_bytes)?;

        let mut node = KnowledgeNode::new(
            NodeType::Media,
            proposal.label,
            pattern,
            Confidence::Unverified,
            &proposal.source,
        );

        // Store media metadata on the node
        node.metadata.insert("media_type".to_string(), "image".to_string());
        node.metadata.insert("width".to_string(), features.width.to_string());
        node.metadata.insert("height".to_string(), features.height.to_string());
        node.metadata.insert("file_size".to_string(), raw_bytes.len().to_string());
        // Extract filename from source if present
        if let Some(fname) = proposal.source.strip_prefix("file:") {
            if let Some(name) = std::path::Path::new(fname).file_name() {
                node.metadata.insert("filename".to_string(), name.to_string_lossy().to_string());
            }
        }

        let seed = self.network.add_node(node);

        // Auto-associate with similar nodes (cross-modal bonding)
        self.auto_associate(&seed, 0.3, 3);

        Some(seed)
    }

    /// Reject a pending image proposal.
    pub fn reject_image_proposal(&mut self, id: &str) -> bool {
        self.pending_image_proposals.remove(id).is_some()
    }

    // ─── Audio Perception ───────────────────────────────

    /// Propose learning an audio clip. The raw bytes are wrapped as
    /// GatedData immediately. Returns a proposal for review.
    /// Nothing is learned until approved.
    ///
    /// `label` is a human-readable descriptor (e.g. "birdsong recording").
    /// The WAV bytes are encoded through the pattern engine to produce
    /// a 256-dim vector in the same space as text and image patterns.
    pub fn propose_audio(
        &mut self,
        raw_bytes: &[u8],
        label: &str,
        source: &str,
    ) -> Result<AudioProposal, String> {
        let (pattern, features) = self.pattern_engine.encode_audio(raw_bytes)
            .ok_or_else(|| "Failed to decode audio bytes (WAV expected)".to_string())?;

        let id = Seed::generate().short();
        let gated_bytes = GatedData::create(raw_bytes);

        let similar = self.network_node_similarities(&pattern, 0.4);
        let similar_existing: Vec<(String, String, f32)> = similar.iter()
            .take(5)
            .filter_map(|(seed, sim)| {
                self.network.get_node(seed).map(|n| {
                    (seed.to_hex(), n.label.clone(), *sim)
                })
            })
            .collect();

        let scar_conflicts = self.check_scar_conflicts(label);

        let recommendation = if scar_conflicts.is_empty() {
            "learn".to_string()
        } else {
            "REVIEW - matches revocation scar".to_string()
        };

        let features_summary = format!(
            "{:.1}s, {}Hz, mean_rms={:.3}, peak_rms={:.3}, {} audios encoded",
            features.duration_secs, features.sample_rate,
            features.mean_rms, features.peak_rms,
            self.pattern_engine.audios_encoded()
        );

        let proposal = AudioProposal {
            id: id.clone(),
            label: label.to_string(),
            source: source.to_string(),
            gated_bytes,
            features_summary,
            similar_existing,
            scar_conflicts,
            recommendation,
        };

        self.pending_audio_proposals.insert(id, proposal.clone());
        Ok(proposal)
    }

    /// Approve a pending audio proposal. Creates a Media node in the
    /// network with the audio's 256-dim pattern vector.
    pub fn approve_audio(&mut self, id: &str) -> Option<Seed> {
        let proposal = self.pending_audio_proposals.remove(id)?;
        let raw_bytes = proposal.gated_bytes.access(&proposal.gated_bytes.seed)?;
        let (pattern, features) = self.pattern_engine.encode_audio(&raw_bytes)?;

        let mut node = KnowledgeNode::new(
            NodeType::Media,
            proposal.label,
            pattern,
            Confidence::Unverified,
            &proposal.source,
        );

        // Store audio metadata on the node
        node.metadata.insert("media_type".to_string(), "audio".to_string());
        node.metadata.insert("sample_rate".to_string(), features.sample_rate.to_string());
        node.metadata.insert("channels".to_string(), features.channels.to_string());
        node.metadata.insert("duration_secs".to_string(), format!("{:.2}", features.duration_secs));
        node.metadata.insert("total_samples".to_string(), features.total_samples.to_string());
        node.metadata.insert("file_size".to_string(), raw_bytes.len().to_string());
        if let Some(fname) = proposal.source.strip_prefix("file:") {
            if let Some(name) = std::path::Path::new(fname).file_name() {
                node.metadata.insert("filename".to_string(), name.to_string_lossy().to_string());
            }
        }

        let seed = self.network.add_node(node);
        self.auto_associate(&seed, 0.3, 3);
        Some(seed)
    }

    /// Reject a pending audio proposal.
    pub fn reject_audio_proposal(&mut self, id: &str) -> bool {
        self.pending_audio_proposals.remove(id).is_some()
    }

    // ─── Video Perception ───────────────────────────────

    /// Propose learning a video from its keyframes. Each keyframe's raw
    /// image bytes are individually wrapped as GatedData. Returns a
    /// proposal for review. Nothing is learned until approved.
    ///
    /// `keyframe_bytes` is a Vec of raw image bytes (one per keyframe).
    /// The caller is responsible for extracting keyframes from the video.
    pub fn propose_video(
        &mut self,
        keyframe_bytes: &[&[u8]],
        label: &str,
        source: &str,
    ) -> Result<VideoProposal, String> {
        // Build slice-of-slices for pattern engine
        let (pattern, features) = self.pattern_engine.encode_video(keyframe_bytes)
            .ok_or_else(|| "Failed to encode video keyframes".to_string())?;

        let id = Seed::generate().short();

        // Gate each keyframe individually
        let gated_keyframes: Vec<GatedData> = keyframe_bytes.iter()
            .map(|bytes| GatedData::create(bytes))
            .collect();

        let similar = self.network_node_similarities(&pattern, 0.4);
        let similar_existing: Vec<(String, String, f32)> = similar.iter()
            .take(5)
            .filter_map(|(seed, sim)| {
                self.network.get_node(seed).map(|n| {
                    (seed.to_hex(), n.label.clone(), *sim)
                })
            })
            .collect();

        let scar_conflicts = self.check_scar_conflicts(label);

        let recommendation = if scar_conflicts.is_empty() {
            "learn".to_string()
        } else {
            "REVIEW - matches revocation scar".to_string()
        };

        let features_summary = format!(
            "{}x{}, {} keyframes, {} videos encoded",
            features.width, features.height,
            features.num_keyframes,
            self.pattern_engine.videos_encoded()
        );

        let proposal = VideoProposal {
            id: id.clone(),
            label: label.to_string(),
            source: source.to_string(),
            gated_keyframes,
            features_summary,
            similar_existing,
            scar_conflicts,
            recommendation,
        };

        self.pending_video_proposals.insert(id, proposal.clone());
        Ok(proposal)
    }

    /// Approve a pending video proposal. Creates a Media node in the
    /// network with the video's 256-dim pattern vector.
    pub fn approve_video(&mut self, id: &str) -> Option<Seed> {
        let proposal = self.pending_video_proposals.remove(id)?;

        // Reconstruct keyframe bytes from gated data
        let mut frame_bytes: Vec<Vec<u8>> = Vec::new();
        for gated in &proposal.gated_keyframes {
            if let Some(bytes) = gated.access(&gated.seed) {
                frame_bytes.push(bytes);
            }
        }

        if frame_bytes.is_empty() {
            return None;
        }

        let refs: Vec<&[u8]> = frame_bytes.iter().map(|v| v.as_slice()).collect();
        let (pattern, features) = self.pattern_engine.encode_video(&refs)?;

        let mut node = KnowledgeNode::new(
            NodeType::Media,
            proposal.label,
            pattern,
            Confidence::Unverified,
            &proposal.source,
        );

        // Store video metadata on the node
        node.metadata.insert("media_type".to_string(), "video".to_string());
        node.metadata.insert("width".to_string(), features.width.to_string());
        node.metadata.insert("height".to_string(), features.height.to_string());
        node.metadata.insert("num_keyframes".to_string(), features.num_keyframes.to_string());
        node.metadata.insert("total_frames".to_string(), features.total_frames.to_string());
        if let Some(fname) = proposal.source.strip_prefix("file:") {
            if let Some(name) = std::path::Path::new(fname).file_name() {
                node.metadata.insert("filename".to_string(), name.to_string_lossy().to_string());
            }
        }

        let seed = self.network.add_node(node);
        self.auto_associate(&seed, 0.3, 3);
        Some(seed)
    }

    /// Reject a pending video proposal.
    pub fn reject_video_proposal(&mut self, id: &str) -> bool {
        self.pending_video_proposals.remove(id).is_some()
    }

    // ─── Unified Media Approval ─────────────────────────

    /// Approve a pending media proposal (image, audio, or video) by ID.
    /// Checks all pending media queues and dispatches to the correct
    /// approve method. Returns the new node seed, or None if not found.
    pub fn approve_media_proposal(&mut self, id: &str) -> Option<Seed> {
        if self.pending_image_proposals.contains_key(id) {
            return self.approve_image(id);
        }
        if self.pending_audio_proposals.contains_key(id) {
            return self.approve_audio(id);
        }
        if self.pending_video_proposals.contains_key(id) {
            return self.approve_video(id);
        }
        None
    }

    /// Reject a pending media proposal (image, audio, or video) by ID.
    /// Returns true if found and rejected in any media queue.
    pub fn reject_media_proposal(&mut self, id: &str) -> bool {
        if self.pending_image_proposals.contains_key(id) {
            return self.reject_image_proposal(id);
        }
        if self.pending_audio_proposals.contains_key(id) {
            return self.reject_audio_proposal(id);
        }
        if self.pending_video_proposals.contains_key(id) {
            return self.reject_video_proposal(id);
        }
        false
    }

    // ─── Session Reports (v0 7.5) ───────────────────────

    /// Generate a session report: a full manifest of what a batch of
    /// knowledge items would do if committed. Nothing is changed.
    ///
    /// This is the review step from the master plan. Before integrating
    /// new knowledge, the brain shows exactly what will happen:
    /// - New atoms to be created (with proposed seeds)
    /// - Associations that will form
    /// - Existing atoms that would be affected (similar content)
    /// - Any scar conflicts that would fire
    ///
    /// The human reviews this through the terminal. Redlines items.
    /// Only approved items proceed to batch_train.
    pub fn session_report(&mut self, items: &[SessionReportItem]) -> SessionReport {
        let mut entries: Vec<SessionReportEntry> = Vec::new();
        let mut total_new_atoms = 0usize;
        let mut total_new_bonds = 0usize;
        let mut total_scar_warnings = 0usize;

        for item in items {
            // Check for similar existing nodes (would this be a duplicate?)
            let pattern = self.pattern_engine.encode(&item.content);
            let similar = self.network_node_similarities(&pattern, 0.4);
            let similar_existing: Vec<(String, String, f32)> = similar.iter()
                .take(3)
                .filter_map(|(seed, sim)| {
                    self.network.get_node(seed).map(|n| {
                        (seed.to_hex(), n.label.clone(), *sim)
                    })
                })
                .collect();

            // Check for scar conflicts (re-encode + word overlap)
            let scar_conflicts = self.check_scar_conflicts(&item.content);

            // Count associations that would form
            let bond_count = item.associations.as_ref().map_or(0, |a| a.len());

            // Check which proposed associations match existing nodes
            let association_targets: Vec<String> = item.associations.as_ref()
                .map(|assocs| {
                    assocs.iter().map(|a| {
                        // See if this association target already exists as a node
                        let exists = self.network.active_nodes()
                            .any(|(_, n)| n.label == *a);
                        if exists {
                            format!("→ [existing] {}", Self::truncate(a, 60))
                        } else {
                            format!("→ [new in batch] {}", Self::truncate(a, 60))
                        }
                    }).collect()
                })
                .unwrap_or_default();

            let status = if !scar_conflicts.is_empty() {
                total_scar_warnings += 1;
                "SCAR_CONFLICT".to_string()
            } else if similar_existing.iter().any(|(_, _, sim)| *sim > 0.9) {
                "POSSIBLE_DUPLICATE".to_string()
            } else {
                "READY".to_string()
            };

            total_new_atoms += 1;
            total_new_bonds += bond_count;

            entries.push(SessionReportEntry {
                content: item.content.clone(),
                status,
                similar_existing,
                scar_conflicts,
                association_targets,
                bond_count,
            });
        }

        SessionReport {
            total_items: items.len(),
            total_new_atoms,
            total_new_bonds,
            total_scar_warnings,
            current_brain_nodes: self.network.stats().active_nodes,
            current_brain_bonds: self.network.stats().active_bonds,
            projected_nodes: self.network.stats().active_nodes + total_new_atoms,
            projected_bonds: self.network.stats().active_bonds + total_new_bonds,
            entries,
        }
    }

    fn truncate(s: &str, max: usize) -> String {
        if s.len() <= max { s.to_string() }
        else { format!("{}...", &s[..max]) }
    }

    // ─── Self-Modification (v0 7.6) ────────────────────

    /// Self-reflect after a recall. Examines the last recall result
    /// and strengthens bonds between co-activated nodes. This is
    /// use-based plasticity — the brain rewires itself through use.
    ///
    /// "Frequently co-activated atoms strengthen their bond."
    /// — Master Plan Section 3
    ///
    /// Also identifies pairs of co-activated nodes that have NO bond
    /// and creates novel associations (self-generated learning).
    ///
    /// Returns a report of what changed.
    pub fn self_reflect(&mut self) -> SelfModReport {
        let mut report = SelfModReport {
            bonds_strengthened: 0,
            bonds_created: 0,
            novel_associations: Vec::new(),
            strengthened_pairs: Vec::new(),
        };

        let recall = match &self.last_recall {
            Some(r) => r.clone(),
            None => return report,
        };

        // Only reflect on the top activated nodes (the ones that
        // actually contributed to the response)
        let top_nodes: Vec<&ActivatedNode> = recall.activated_nodes.iter()
            .filter(|n| n.contribution > 0.05) // at least 5% contribution
            .collect();

        if top_nodes.len() < 2 {
            return report;
        }

        // For every pair of co-activated nodes:
        // - If bonded: strengthen the bond
        // - If NOT bonded: create a novel association
        for i in 0..top_nodes.len() {
            for j in (i+1)..top_nodes.len() {
                let a = &top_nodes[i];
                let b = &top_nodes[j];

                let existing_weight = self.network.bond_weight(&a.seed_hex, &b.seed_hex);

                // Detect cross-modal pair: one Media + one non-Media.
                // Cross-modal co-activation is a stronger signal than
                // same-modality co-activation — it means genuinely
                // related content surfaced across perception boundaries.
                let a_is_media = self.network.get_node_by_hex(&a.seed_hex)
                    .map_or(false, |n| n.node_type == NodeType::Media);
                let b_is_media = self.network.get_node_by_hex(&b.seed_hex)
                    .map_or(false, |n| n.node_type == NodeType::Media);
                let is_cross_modal = a_is_media != b_is_media;

                if let Some(_weight) = existing_weight {
                    // Bond exists — strengthen it. Amount proportional to
                    // how strongly both nodes contributed.
                    // Cross-modal bonds get a 2x boost — the brain values
                    // connections that bridge perception boundaries.
                    let base_amount = (a.contribution * b.contribution * 2.0).min(0.3);
                    let amount = if is_cross_modal { (base_amount * 2.0).min(0.5) } else { base_amount };
                    if let Ok(new_weight) = self.network.strengthen_bond(
                        &a.seed_hex, &b.seed_hex, amount
                    ) {
                        report.bonds_strengthened += 1;
                        report.strengthened_pairs.push((
                            Self::truncate(&a.label, 50),
                            Self::truncate(&b.label, 50),
                            new_weight,
                        ));
                    }
                } else {
                    // No bond exists — these nodes were co-activated but
                    // aren't directly connected. The brain discovered a
                    // novel association through traversal. Create it.
                    //
                    // Cross-modal discoveries use a lower contribution
                    // threshold (0.05 vs 0.10) — finding that an image
                    // and a text node co-activate is inherently meaningful
                    // even at lower activation levels.
                    let threshold = if is_cross_modal { 0.05 } else { 0.10 };
                    if a.contribution > threshold && b.contribution > threshold {
                        if let (Ok(seed_a), Ok(seed_b)) = (
                            Seed::from_hex(&a.seed_hex),
                            Seed::from_hex(&b.seed_hex),
                        ) {
                            let desc = if is_cross_modal {
                                "cross-modal co-activation: text↔media bridge".to_string()
                            } else {
                                "self-association: co-activated during recall".to_string()
                            };
                            // Cross-modal bonds start stronger (0.5 vs 0.3)
                            let initial_weight = if is_cross_modal { 0.5 } else { 0.3 };
                            if let Ok(_bond_seed) = self.network.add_bond(
                                &seed_a, &seed_b,
                                desc,
                                initial_weight,
                                "self",
                            ) {
                                report.bonds_created += 1;
                                report.novel_associations.push((
                                    Self::truncate(&a.label, 50),
                                    Self::truncate(&b.label, 50),
                                ));
                            }
                        }
                    }
                }
            }
        }

        // ─── Left Brain: Session Housekeeping ────────────────
        // Decay expired session facts. Promotion candidates have already
        // been updated by get_relevant_context() during recall().
        // We do NOT auto-promote here — that is the whole point.
        self.session_memory.decay_old_facts();

        report
    }

    /// Apply organic decay to all bonds. Bonds weaken from disuse.
    /// Dead bonds (weight ≤ 0) are deactivated.
    ///
    /// "Unused bonds weaken naturally. The system forgets what it
    /// does not use." — Master Plan Section 3
    ///
    /// Returns (bonds_decayed, bonds_killed).
    pub fn decay(&mut self, amount: f32) -> (usize, usize) {
        self.network.decay_all_bonds(amount)
    }

    /// Prune dead bonds from the network.
    /// Returns count of bonds removed.
    pub fn prune(&mut self) -> usize {
        self.network.prune_dead_bonds()
    }

    /// Self-generated learning: the brain creates a new knowledge atom
    /// from its own internal reasoning, not from external input.
    /// These get a special confidence level and "self" as creator.
    ///
    /// "Self-generated associations receive their own seeds with full
    /// lineage tracking to the parent atoms that produced them."
    /// — V0 Spec Section 3.3
    pub fn self_learn(
        &mut self,
        content: &str,
        parent_hexes: &[String],
    ) -> Option<Seed> {
        // Check for scar conflicts before self-learning (re-encode + word overlap)
        let scar_conflicts = self.check_scar_conflicts(content);
        if !scar_conflicts.is_empty() {
            return None; // Scar prevents this learning
        }

        // Check for duplicate content
        if self.find_existing_content(content).is_some() {
            return None; // Already known
        }

        let seed = self.learn(content, Confidence::Unverified, "self");

        // Bond to parent nodes
        for hex in parent_hexes {
            if let Ok(parent_seed) = Seed::from_hex(hex) {
                let _ = self.network.add_bond(
                    &seed, &parent_seed,
                    "self-derived".to_string(),
                    0.5,
                    "self",
                );
            }
        }

        Some(seed)
    }

    /// Strengthen a specific bond between two nodes (by seed hex).
    /// The brain consciously reinforces a connection.
    pub fn strengthen_bond(&mut self, src_hex: &str, tgt_hex: &str, amount: f32) -> Result<f32, String> {
        self.network.strengthen_bond(src_hex, tgt_hex, amount)
    }

    /// Weaken a specific bond between two nodes (by seed hex).
    /// The brain consciously weakens a connection.
    pub fn weaken_bond(&mut self, src_hex: &str, tgt_hex: &str, amount: f32) -> Result<f32, String> {
        self.network.weaken_bond(src_hex, tgt_hex, amount)
    }

    // ─── External Data Ingestion (v0 7.7) ─────────────

    /// Ingest raw text: split into sentences, check each against scars
    /// and existing knowledge, return proposals for review.
    ///
    /// "External data is raw material to observe, never to obey."
    /// — Master Plan Section 12
    ///
    /// Nothing is auto-learned. Every sentence becomes a proposal
    /// that must be approved before integration.
    pub fn ingest_text(&mut self, text: &str, source: &str) -> IngestReport {
        // Split text into (content, is_code) pairs.
        // Code blocks (prefixed with [code:*] by clean_pdf_text) are kept atomic;
        // prose is sentence-split normally.
        let items = Self::extract_ingest_items(text);
        let mut proposals: Vec<IngestProposal> = Vec::new();
        let mut ready = 0usize;
        let mut blocked = 0usize;
        let mut duplicates = 0usize;

        for (content, is_code) in &items {
            let trimmed = content.trim();
            // Skip short fragments — but never skip code blocks
            if !is_code && trimmed.len() < 10 { continue; }

            let id = Seed::generate().short();

            // Skip duplicate check for code blocks (each snippet is unique by nature)
            if !is_code {
                if self.find_existing_content(trimmed).is_some() {
                    duplicates += 1;
                    continue;
                }
            }

            // Check for scar conflicts (re-encode + word overlap)
            let scar_conflicts = self.check_scar_conflicts(trimmed);

            // Check for similar existing nodes
            let pattern = self.pattern_engine.encode(trimmed);
            let similar = self.network_node_similarities(&pattern, 0.4);
            let similar_existing: Vec<(String, String, f32)> = similar.iter()
                .take(3)
                .filter_map(|(seed, sim)| {
                    self.network.get_node(seed).map(|n| {
                        (seed.to_hex(), n.label.clone(), *sim)
                    })
                })
                .collect();

            let status = if !scar_conflicts.is_empty() {
                blocked += 1;
                "SCAR_CONFLICT".to_string()
            } else {
                ready += 1;
                "READY".to_string()
            };

            // Store as pending proposal
            let proposal = IngestProposal {
                id: id.clone(),
                content: trimmed.to_string(),
                status,
                scar_conflicts: scar_conflicts.clone(),
                similar_existing: similar_existing.clone(),
            };

            // Also store in pending_proposals for approve/reject
            self.pending_proposals.insert(id.clone(), LearningProposal {
                id: id.clone(),
                content: trimmed.to_string(),
                source: source.to_string(),
                similar_existing,
                scar_conflicts,
                recommendation: if blocked > 0 { "REVIEW".to_string() } else { "learn".to_string() },
            });

            proposals.push(proposal);
        }

        IngestReport {
            source: source.to_string(),
            total_sentences: items.len(),
            proposals_ready: ready,
            proposals_blocked: blocked,
            duplicates_skipped: duplicates,
            proposals,
        }
    }

    /// Ingest a file from disk. Detects file type by extension and
    /// routes to the appropriate encoder:
    ///
    /// - Image (.png, .jpg, .jpeg, .gif, .bmp, .webp) → propose_image
    /// - Audio (.wav, .mp3, .ogg, .flac) → propose_audio
    /// - Video (.mp4, .avi, .mkv, .webm) → propose_video (needs keyframes)
    /// - PDF (.pdf) → extract text → ingest_text
    /// - Text (.txt, .md, etc.) → ingest_text
    ///
    /// Media files become proposals (ImageProposal, AudioProposal, etc.)
    /// that must be approved before integration. Text files become
    /// IngestReport with per-sentence proposals as before.
    ///
    /// The brain observes the file. It does not obey anything in it.
    pub fn ingest_file(&mut self, path: &str) -> Result<IngestReport, String> {
        let lower = path.to_lowercase();
        let filename = std::path::Path::new(path)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());

        // ─── Image files ────────────────────────────────
        let image_exts = [".png", ".jpg", ".jpeg", ".gif", ".bmp", ".webp"];
        if image_exts.iter().any(|ext| lower.ends_with(ext)) {
            let bytes = std::fs::read(path)
                .map_err(|e| format!("Failed to read image file {}: {}", path, e))?;
            let file_size = bytes.len();
            let label = format!("[image] {}", filename);
            let proposal = self.propose_image(&bytes, &label, &format!("file:{}", path))?;

            // Store file metadata on the proposal for later node creation
            // (metadata will be set when the proposal is approved)
            // For now, stash metadata in the features_summary
            let meta_summary = format!(
                "{} | file={}, size={}",
                proposal.features_summary, filename, file_size
            );
            // Update the stored proposal with enriched summary
            if let Some(p) = self.pending_image_proposals.get_mut(&proposal.id) {
                p.features_summary = meta_summary;
            }

            return Ok(IngestReport {
                source: format!("file:{}", path),
                total_sentences: 1,
                proposals_ready: 1,
                proposals_blocked: 0,
                duplicates_skipped: 0,
                proposals: vec![IngestProposal {
                    id: proposal.id,
                    content: label,
                    status: "READY_IMAGE".to_string(),
                    scar_conflicts: proposal.scar_conflicts,
                    similar_existing: proposal.similar_existing,
                }],
            });
        }

        // ─── Audio files ────────────────────────────────
        let audio_exts = [".wav", ".mp3", ".ogg", ".flac"];
        if audio_exts.iter().any(|ext| lower.ends_with(ext)) {
            let bytes = std::fs::read(path)
                .map_err(|e| format!("Failed to read audio file {}: {}", path, e))?;
            let file_size = bytes.len();
            let label = format!("[audio] {}", filename);
            let proposal = self.propose_audio(&bytes, &label, &format!("file:{}", path))?;

            if let Some(p) = self.pending_audio_proposals.get_mut(&proposal.id) {
                p.features_summary = format!(
                    "{} | file={}, size={}",
                    p.features_summary, filename, file_size
                );
            }

            return Ok(IngestReport {
                source: format!("file:{}", path),
                total_sentences: 1,
                proposals_ready: 1,
                proposals_blocked: 0,
                duplicates_skipped: 0,
                proposals: vec![IngestProposal {
                    id: proposal.id,
                    content: label,
                    status: "READY_AUDIO".to_string(),
                    scar_conflicts: proposal.scar_conflicts,
                    similar_existing: proposal.similar_existing,
                }],
            });
        }

        // ─── Video files ────────────────────────────────
        // Video ingestion reads the raw bytes but needs keyframe
        // extraction. For now, treat the first frame as a single
        // keyframe by attempting to decode the bytes as an image.
        // Full video decoding requires an external codec (ffmpeg).
        let video_exts = [".mp4", ".avi", ".mkv", ".webm"];
        if video_exts.iter().any(|ext| lower.ends_with(ext)) {
            let bytes = std::fs::read(path)
                .map_err(|e| format!("Failed to read video file {}: {}", path, e))?;
            let _file_size = bytes.len();
            let label = format!("[video] {}", filename);

            // Attempt single-frame video proposal.
            // Real video containers can't be decoded as images, so this
            // will typically fail. In that case, return an informative
            // error suggesting the use of propose_video with extracted
            // keyframes via an external tool.
            let keyframes: Vec<&[u8]> = vec![&bytes];
            match self.propose_video(&keyframes, &label, &format!("file:{}", path)) {
                Ok(proposal) => {
                    return Ok(IngestReport {
                        source: format!("file:{}", path),
                        total_sentences: 1,
                        proposals_ready: 1,
                        proposals_blocked: 0,
                        duplicates_skipped: 0,
                        proposals: vec![IngestProposal {
                            id: proposal.id,
                            content: label,
                            status: "READY_VIDEO".to_string(),
                            scar_conflicts: proposal.scar_conflicts,
                            similar_existing: proposal.similar_existing,
                        }],
                    });
                }
                Err(_) => {
                    return Err(format!(
                        "Video file detected: {}. Video ingestion requires \
                         pre-extracted keyframes. Use brain_propose_video with \
                         keyframe image bytes extracted via ffmpeg or similar.",
                        filename
                    ));
                }
            }
        }

        // ─── PDF files ──────────────────────────────────
        if lower.ends_with(".pdf") {
            let bytes = std::fs::read(path)
                .map_err(|e| format!("Failed to read PDF file {}: {}", path, e))?;
            let raw_content = pdf_extract::extract_text_from_mem(&bytes)
                .map_err(|e| format!("Failed to extract text from PDF {}: {}", path, e))?;
            let content = clean_pdf_text(&raw_content);
            return Ok(self.ingest_text(&content, &format!("file:{}", path)));
        }

        // ─── Text files (default) ───────────────────────
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read file {}: {}", path, e))?;
        Ok(self.ingest_text(&content, &format!("file:{}", path)))
    }

    /// Bulk approve all READY proposals from an ingest.
    /// Handles text proposals AND media proposals (READY_IMAGE, READY_AUDIO, READY_VIDEO).
    /// Returns seeds of all learned nodes.
    pub fn approve_all_ready(&mut self, proposal_ids: &[String]) -> Vec<(String, Seed)> {
        let mut results = Vec::new();
        for id in proposal_ids {
            // Text proposals
            if let Some(proposal) = self.pending_proposals.remove(id) {
                if !proposal.scar_conflicts.is_empty() {
                    // Don't auto-approve scar conflicts
                    self.pending_proposals.insert(id.clone(), proposal);
                    continue;
                }
                let seed = self.learn(&proposal.content, Confidence::Unverified, "external");
                results.push((id.clone(), seed));
                continue;
            }
            // Media proposals (image, audio, video) — route through approve_media_proposal
            if let Some(seed) = self.approve_media_proposal(id) {
                results.push((id.clone(), seed));
            }
        }
        results
    }

    /// Split text into sentences. Simple splitter — splits on . ! ?
    /// followed by whitespace or end of string.
    fn split_sentences(text: &str) -> Vec<String> {
        let mut sentences = Vec::new();
        let mut current = String::new();

        for ch in text.chars() {
            current.push(ch);
            if ch == '.' || ch == '!' || ch == '?' {
                let trimmed = current.trim().to_string();
                if trimmed.len() >= 10 {
                    sentences.push(trimmed);
                }
                current = String::new();
            }
        }

        // Remaining text (no sentence-ending punctuation)
        let trimmed = current.trim().to_string();
        if trimmed.len() >= 10 {
            sentences.push(trimmed);
        }

        sentences
    }

    /// Split ingestion text into (content, is_code) items.
    ///
    /// Lines prefixed with `[code:*]` begin a code block that is kept as a
    /// single atomic unit (not sentence-split). A blank line terminates the
    /// block. All other text is sentence-split normally.
    fn extract_ingest_items(text: &str) -> Vec<(String, bool)> {
        let mut items: Vec<(String, bool)> = Vec::new();
        let mut prose_buf = String::new();
        let mut code_buf: Option<String> = None;

        for line in text.lines() {
            if line.starts_with("[code:") {
                // Flush any accumulated prose as individual sentences
                if !prose_buf.trim().is_empty() {
                    for s in Self::split_sentences(&prose_buf) {
                        items.push((s, false));
                    }
                    prose_buf.clear();
                }
                // Start a new code block with the tag line
                code_buf = Some(line.to_string());
            } else if let Some(ref mut cb) = code_buf {
                if line.trim().is_empty() {
                    // Blank line closes the code block
                    let block = std::mem::take(cb);
                    if !block.trim().is_empty() {
                        items.push((block, true));
                    }
                    code_buf = None;
                } else {
                    cb.push('\n');
                    cb.push_str(line);
                }
            } else {
                prose_buf.push_str(line);
                prose_buf.push('\n');
            }
        }

        // Flush remaining buffers
        if let Some(block) = code_buf {
            if !block.trim().is_empty() {
                items.push((block, true));
            }
        }
        if !prose_buf.trim().is_empty() {
            for s in Self::split_sentences(&prose_buf) {
                items.push((s, false));
            }
        }

        items
    }

    // ─── Conversational Prompt Interface (v0 7.8) ─────────

    /// Enrich a short or ambiguous query with recent conversation context.
    ///
    /// If someone says "tell me about cats" then "?", the "?" alone
    /// has zero information. But with context it becomes "cats ?".
    /// This gives the recall system something to work with.
    ///
    /// Rules:
    /// - Queries with 3+ meaningful words: use as-is (enough signal)
    /// - Queries with 1-2 words: prepend the last 1-2 conversation topics
    /// - Empty/punctuation-only: replay the last query entirely
    fn enrich_query(&self, query: &str) -> String {
        let meaningful_words: Vec<&str> = query
            .split_whitespace()
            .filter(|w| w.len() >= 2)
            .collect();

        // 1+ meaningful words — use as-is. Even a single content word
        // like "gravity" or "water" has enough signal for keyword matching.
        // Enrichment would pollute it with unrelated context.
        if !meaningful_words.is_empty() {
            return query.to_string();
        }

        // No conversation history — can't enrich
        if self.conversation_history.is_empty() {
            return query.to_string();
        }

        // Zero meaningful words (punctuation-only, pronouns, "?", "more", etc.)
        // → replay the last query so the brain has something to work with.
        self.conversation_history.last().unwrap().clone()
    }

    /// The brain responds to a query by composing its own answer
    /// from activated knowledge atoms. No LLM. The response emerges
    /// from the pattern of activated atoms, ordered by contribution.
    ///
    /// "The response emerges from the pattern of activated atoms.
    ///  The traversal path is recorded and available as the seed report."
    /// — Master Plan Section 3
    ///
    /// Composition strategy:
    /// 1. Enrich short queries with conversation context
    /// 2. Recall activates relevant atoms via spreading activation
    /// 3. Filter for connected subgraph coherence
    /// 4. Quality gate: reject weak/incoherent activations
    /// 5. Top contributors become the response
    /// 6. Self-reflect strengthens the bonds that produced this response
    pub fn respond(&mut self, query: &str, depth: usize, top_k: usize) -> BrainResponse {
        use std::io::Write;
        let mut timing_log = std::fs::OpenOptions::new()
            .create(true).append(true)
            .open(r"brain\recall_timing.log")
            .ok();
        if let Some(ref mut f) = timing_log {
            let _ = writeln!(f, "\n=== respond({:?}) depth={} top_k={} ===", query, depth, top_k);
        }
        let ask_start = std::time::Instant::now();
        // Phase 0: Understand — what kind of question is this?
        let intent = classify_query(query);

        // Phase 0.5: Enrich — short queries get conversation context
        let effective_query = self.enrich_query(query);

        // For certain intents, extract the topic for a more focused recall.
        // Also tune recall parameters: definitions want shallow+precise,
        // why/comparison want deep+broad.
        let (recall_query, eff_depth, eff_top_k) = match &intent {
            QueryIntent::Definition { topic } => (topic.clone(), depth.min(2), top_k),
            QueryIntent::Who { topic } => (topic.clone(), depth.min(2), top_k),
            QueryIntent::Why { topic } => (topic.clone(), depth.max(4), (top_k * 2).max(10)),
            QueryIntent::Process { topic } => (topic.clone(), depth.max(3), (top_k * 2).max(10)),
            QueryIntent::Comparison { topic_a, topic_b } => {
                // Combine both topics for a single broad recall
                (format!("{} {}", topic_a, topic_b), depth.max(3), (top_k * 2).max(10))
            }
            QueryIntent::List { topic, .. } => {
                (topic.clone(), depth.max(3), (top_k * 2).max(10))
            }
            QueryIntent::Think { topic } => {
                // Deep traversal, large result set — we need to find
                // connections between multiple knowledge clusters
                (topic.clone(), depth.max(5), (top_k * 3).max(15))
            }
            QueryIntent::Explore { topic } => {
                // Broad recall — maximize diversity and coverage
                (topic.clone(), depth.max(4), (top_k * 3).max(15))
            }
            QueryIntent::Ability { topic } => (topic.clone(), depth, top_k),
            QueryIntent::YesNo { topic } => (topic.clone(), depth, top_k),
            QueryIntent::General { topic } => (topic.clone(), depth, top_k),
            QueryIntent::Greeting => (effective_query.clone(), 1, 3),
            QueryIntent::MetaSelf => (effective_query.clone(), 1, 3),
        };

        // Use the topic-extracted query if it has substance, else fall back
        let recall_input = if recall_query.split_whitespace().count() >= 1
            && recall_query.len() >= 2 {
            recall_query
        } else {
            effective_query.clone()
        };

        // Phase 1: Recall — activate the knowledge graph
        // Intent tunes depth and top_k for optimal results per question type
        let result = self.recall(&recall_input, eff_depth, eff_top_k);
        eprintln!("[ask] recall took: {:?}", ask_start.elapsed());
        if let Some(ref mut f) = timing_log { let _ = writeln!(f, "[ask] recall took: {:?}", ask_start.elapsed()); }

        // Phase 1.5: Filter — connected subgraph coherence
        let result = self.filter_connected_subgraph(result);

        // Phase 2: Formulate — shape the response based on intent
        let text = Self::formulate_response(&intent, &result);

        // Phase 3: Reflect — DISABLED during conversation.
        // self_reflect strengthens bonds between co-activated nodes,
        // which biases subsequent queries toward the same cluster.
        // "tell me about cats" → strengthens cat bonds → "water" returns cats.
        // Self-reflection should happen as a deliberate maintenance step,
        // not during every conversation turn.
        // let _mod_report = self.self_reflect();

        // Phase 4: Build the response object
        let atoms_used = result.activated_nodes.len();
        let traversal_hops = result.traversal_path.len();

        // Coherence: sum of top-3 contributions (how concentrated the answer is)
        let coherence_score: f32 = result.activated_nodes.iter()
            .take(3)
            .map(|n| n.contribution)
            .sum();

        let sources: Vec<(String, String, f32)> = result.activated_nodes.iter()
            .map(|n| (n.seed_hex.clone(), n.label.clone(), n.contribution * 100.0))
            .collect();

        let scar_warnings = result.scar_warnings.clone();

        // Extract image URLs from activated nodes
        let image_urls: Vec<(String, String)> = result.activated_nodes.iter()
            .filter_map(|n| {
                if let Some(rest) = n.label.strip_prefix("[Image: ") {
                    if let Some(bracket_end) = rest.find(']') {
                        let url = rest[..bracket_end].trim().to_string();
                        let desc = rest[bracket_end + 1..].trim().to_string();
                        if !url.is_empty() { return Some((url, desc)); }
                    }
                }
                None
            })
            .collect();

        // Phase 5: Remember — store this query for conversation context
        // Only store queries with meaningful content (not "?" or ".")
        let meaningful: Vec<&str> = query.split_whitespace()
            .filter(|w| w.len() >= 2)
            .collect();
        if !meaningful.is_empty() {
            self.conversation_history.push(query.to_string());
            // Keep only the last 5 queries — enough for context, not enough to pollute
            if self.conversation_history.len() > 5 {
                self.conversation_history.remove(0);
            }
        }

        eprintln!("[ask] total: {:?}", ask_start.elapsed());
        if let Some(ref mut f) = timing_log { let _ = writeln!(f, "[ask] total: {:?}", ask_start.elapsed()); }
        BrainResponse {
            text,
            query: query.to_string(),
            atoms_used,
            traversal_hops,
            coherence_score,
            sources,
            scar_warnings,
            image_urls,
            knowledge_gap: false,
            suggestion: None,
        }
    }

    /// Conversational response — the Left Brain pipeline.
    /// Right Brain: recall() does spreading activation, returns raw data.
    /// Left Brain: classify_query() detects intent, narration engine composes speech.
    pub fn ask(&mut self, query: &str, depth: usize, top_k: usize) -> BrainResponse {
        use std::io::Write;
        let mut timing_log = std::fs::OpenOptions::new()
            .create(true).append(true)
            .open(r"brain\recall_timing.log")
            .ok();
        let ask_start = std::time::Instant::now();
        use crate::narration::{NarrationEngine, NarrationConfidence};

        // Phase 0: Understand — what kind of question is this?
        let intent = classify_query(query);

        // Phase 0.5: Enrich — short queries get conversation context
        let effective_query = self.enrich_query(query);

        // Topic extraction + recall parameter tuning — identical to respond().
        let (recall_query, eff_depth, eff_top_k) = match &intent {
            QueryIntent::Definition { topic } => (topic.clone(), depth.min(2), top_k),
            QueryIntent::Who { topic } => (topic.clone(), depth.min(2), top_k),
            QueryIntent::Why { topic } => (topic.clone(), depth.max(4), (top_k * 2).max(10)),
            QueryIntent::Process { topic } => (topic.clone(), depth.max(3), (top_k * 2).max(10)),
            QueryIntent::Comparison { topic_a, topic_b } => {
                (format!("{} {}", topic_a, topic_b), depth.max(3), (top_k * 2).max(10))
            }
            QueryIntent::List { topic, .. } => {
                (topic.clone(), depth.max(3), (top_k * 2).max(10))
            }
            QueryIntent::Think { topic } => {
                (topic.clone(), depth.max(5), (top_k * 3).max(15))
            }
            QueryIntent::Explore { topic } => {
                (topic.clone(), depth.max(4), (top_k * 3).max(15))
            }
            QueryIntent::Ability { topic } => (topic.clone(), depth, top_k),
            QueryIntent::YesNo { topic } => (topic.clone(), depth, top_k),
            QueryIntent::General { topic } => (topic.clone(), depth, top_k),
            QueryIntent::Greeting => (effective_query.clone(), 1, 3),
            QueryIntent::MetaSelf => (effective_query.clone(), 1, 3),
        };

        let recall_input = if recall_query.split_whitespace().count() >= 1
            && recall_query.len() >= 2 {
            recall_query
        } else {
            effective_query.clone()
        };

        // Phase 1: Recall — activate the knowledge graph (same path as respond())
        let result = self.recall(&recall_input, eff_depth, eff_top_k);
        eprintln!("[ask] recall took: {:?}", ask_start.elapsed());
        if let Some(ref mut f) = timing_log { let _ = writeln!(f, "[ask] recall took: {:?}", ask_start.elapsed()); }

        // Phase 1.1: For Definition/General/Who queries, do additional recalls with
        // definitional patterns. "What is X?" / "Tell me about X" / "Explain X" all strip
        // to just "X", but "X is", "X refers to", etc. surface the actual definitional nodes.
        let result = if matches!(&intent, QueryIntent::Definition { .. } | QueryIntent::General { .. } | QueryIntent::Who { .. }) {
            let def_patterns = [
                format!("{} is", recall_input),
                format!("{} refers to", recall_input),
                format!("{} is a", recall_input),
                format!("{} is defined", recall_input),
            ];
            let def_markers = ["is a", "is the", "is an", "is like", "refers to", "defined as", "means"];
            let mut merged_nodes = result.activated_nodes;
            let mut merged_traversal = result.traversal_path;
            let mut seen_hexes: std::collections::HashSet<String> = merged_nodes.iter()
                .map(|n| n.seed_hex.clone()).collect();
            let mut existing_hops: std::collections::HashSet<(String,String,String)> = merged_traversal.iter()
                .map(|h| (h.from.clone(), h.bond.clone(), h.to.clone())).collect();

            for def_query in &def_patterns {
                let def_result = self.recall(def_query, eff_depth, eff_top_k);
                for node in def_result.activated_nodes {
                    if !seen_hexes.contains(&node.seed_hex) {
                        seen_hexes.insert(node.seed_hex.clone());
                        merged_nodes.push(node);
                    }
                }
                for hop in def_result.traversal_path {
                    let key = (hop.from.clone(), hop.bond.clone(), hop.to.clone());
                    if !existing_hops.contains(&key) {
                        existing_hops.insert(key);
                        merged_traversal.push(hop);
                    }
                }
            }

            // Strongly boost definitional nodes so they dominate over bare label fragments.
            for node in merged_nodes.iter_mut() {
                if def_markers.iter().any(|m| node.label.to_lowercase().contains(m)) {
                    node.strength *= 3.0;
                }
            }

            // Sort: definitional nodes first, then by boosted strength.
            merged_nodes.sort_by(|a, b| {
                let a_is_def = def_markers.iter().any(|m| a.label.to_lowercase().contains(m));
                let b_is_def = def_markers.iter().any(|m| b.label.to_lowercase().contains(m));
                b_is_def.cmp(&a_is_def).then(b.strength.partial_cmp(&a.strength).unwrap_or(std::cmp::Ordering::Equal))
            });

            RecallResult {
                activated_nodes: merged_nodes,
                traversal_path: merged_traversal,
                seed_report: result.seed_report,
                scar_warnings: result.scar_warnings,
            }
        } else {
            result
        };

        // Phase 1.2: Sparse fallback — if recall found fewer than 3 nodes,
        // retry with individual query words and basic stemmed variants
        // (strip s/es/ed/ing). This reduces "I don't know" for short/inflected
        // queries where the compound lookup missed.
        let result = if result.activated_nodes.len() < 3 {
            let mut merged_nodes = result.activated_nodes;
            let mut merged_traversal = result.traversal_path;
            let seed_report = result.seed_report;
            let scar_warnings = result.scar_warnings;
            let mut seen_hexes: HashSet<String> = merged_nodes.iter()
                .map(|n| n.seed_hex.clone()).collect();
            let mut existing_hops: HashSet<(String, String, String)> = merged_traversal.iter()
                .map(|h| (h.from.clone(), h.bond.clone(), h.to.clone())).collect();

            // Build individual-word + stemmed-variant list, deduplicated.
            let mut word_set: HashSet<String> = HashSet::new();
            let mut fallback_words: Vec<String> = Vec::new();
            for w in recall_input.to_lowercase()
                .split_whitespace()
                .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
                .filter(|w| w.len() > 2)
            {
                let lc = w.clone();
                if !QUERY_STOP_WORDS.contains(&lc.as_str()) {
                    if word_set.insert(w.clone()) { fallback_words.push(w.clone()); }
                    let stemmed = Self::stem(&lc).to_string();
                    if stemmed != lc && word_set.insert(stemmed.clone()) {
                        fallback_words.push(stemmed);
                    }
                }
            }

            for word in &fallback_words {
                if merged_nodes.len() >= 3 { break; }
                let word_result = self.recall(word, eff_depth, eff_top_k);
                for node in word_result.activated_nodes {
                    if !seen_hexes.contains(&node.seed_hex) {
                        seen_hexes.insert(node.seed_hex.clone());
                        merged_nodes.push(node);
                    }
                }
                for hop in word_result.traversal_path {
                    let key = (hop.from.clone(), hop.bond.clone(), hop.to.clone());
                    if !existing_hops.contains(&key) {
                        existing_hops.insert(key);
                        merged_traversal.push(hop);
                    }
                }
            }

            RecallResult { activated_nodes: merged_nodes, traversal_path: merged_traversal, seed_report, scar_warnings }
        } else {
            result
        };

        // Phase 1.5: Filter — connected subgraph coherence
        let result = self.filter_connected_subgraph(result);
        // Left Brain: build lookup maps for narration (targeted — only activated nodes + traversal path)
        let bond_labels: HashMap<String, String> = result.traversal_path.iter()
            .filter_map(|h| {
                self.network.get_bond_by_hex(&h.bond)
                    .map(|b| (h.bond.clone(), b.label.clone()))
            })
            .collect();
        let mut node_hexes: HashSet<String> = result.activated_nodes.iter()
            .map(|n| n.seed_hex.clone())
            .collect();
        for h in &result.traversal_path {
            node_hexes.insert(h.from.clone());
            node_hexes.insert(h.to.clone());
        }
        let node_labels: HashMap<String, String> = node_hexes.iter()
            .filter_map(|hex| {
                self.network.get_node_by_hex(hex)
                    .map(|n| (hex.clone(), n.label.clone()))
            })
            .collect();

        // Left Brain: prepare data for narration engine
        let activated: Vec<(String, f32, String)> = result.activated_nodes.iter()
            .map(|n| (n.label.clone(), n.strength, n.seed_hex.clone()))
            .collect();

        let traversal: Vec<(String, String, String, f32, u32)> = result.traversal_path.iter()
            .map(|h| (h.from.clone(), h.bond.clone(), h.to.clone(), h.strength, h.hop))
            .collect();

        // Left Brain: build query pattern + node pattern map for semantic relevance
        let query_pattern = self.pattern_engine.encode(query);
        let node_pattern_map: HashMap<String, Vec<f32>> = result.activated_nodes.iter()
            .filter_map(|n| {
                self.network.get_node_by_hex(&n.seed_hex)
                    .map(|node| (n.seed_hex.clone(), node.pattern.clone()))
            })
            .collect();

        // Left Brain: bond counts per node for hub penalty (targeted — only activated nodes)
        let node_bond_counts: HashMap<String, usize> = result.activated_nodes.iter()
            .filter_map(|n| {
                self.network.bond_count_for(&n.seed_hex)
                    .map(|c| (n.seed_hex.clone(), c))
            })
            .collect();

        // Left Brain: narrate using shared intent + semantic relevance + hub penalty
        // Pass conversation_history so narration can boost on-thread context.
        let mut engine = NarrationEngine::new();
        let narration = engine.narrate(
            &activated, &traversal, &bond_labels, &node_labels,
            query, &intent, Some(&query_pattern), &node_pattern_map,
            &node_bond_counts,
            &self.conversation_history,
        );

        // ── Save topic seeds for next turn's conversation continuity ──
        // Extract the top 10 activated nodes that narration could use as
        // exact seeds for the next turn. These replace the old keyword-flooding
        // conversation context approach.
        {
            use crate::session_memory::TopicSeed;
            // Filter: only keep nodes whose labels contain at least one query keyword.
            // This prevents hub nodes (high activation, zero query relevance) from
            // contaminating topic context for the next turn.
            let seed_keywords: Vec<String> = query.to_lowercase()
                .split_whitespace()
                .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
                .filter(|w| w.len() > 2)
                .collect();
            let top_seeds: Vec<TopicSeed> = result.activated_nodes.iter()
                .filter(|n| {
                    let label_lower = n.label.to_lowercase();
                    seed_keywords.iter().any(|kw| label_lower.contains(kw.as_str()))
                })
                // Filter out hub nodes — high activation from connectivity, not relevance
                .filter(|n| {
                    self.network.bond_count_for(&n.seed_hex)
                        .map_or(true, |c| c <= 500)
                })
                .take(10)
                .map(|n| TopicSeed {
                    seed_hex: n.seed_hex.clone(),
                    score: n.strength,
                })
                .collect();
            {
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(r"lu_debug.log") {
                    let seed_info: Vec<String> = top_seeds.iter()
                        .take(5)
                        .map(|ts| format!("{}({:.2})", &ts.seed_hex[..16.min(ts.seed_hex.len())], ts.score))
                        .collect();
                    let _ = writeln!(f, "[ask] saving topic_seeds: count={} top={:?}", top_seeds.len(), seed_info);
                }
            }
            self.session_memory.set_topic_seeds(top_seeds);
        }

        // ── Phase 6: Confidence Gating ──────────────────────────────────────────
        // Four signals detect when the activated nodes aren't actually about
        // the query — i.e. Lu is about to return noise with false confidence.

        // Pre-compute query keywords (len > 2); IDF weighting in recall() handles frequency dampening.
        let query_keywords: Vec<String> = query.to_lowercase()
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|w| w.len() > 2)
            .collect();

        // Signal 1: Keyword coverage — tightened to 50%.
        // What fraction of query keywords appear in the top-5 activated node labels?
        let keyword_coverage = if query_keywords.is_empty() {
            1.0_f32
        } else {
            let matched = query_keywords.iter()
                .filter(|kw| {
                    result.activated_nodes.iter().take(5).any(|n| {
                        n.label.to_lowercase().contains(kw.as_str())
                    })
                })
                .count();
            matched as f32 / query_keywords.len() as f32
        };

        // Signal 2: Specificity miss — named concepts must appear verbatim.
        // Build bigrams (both tokens >= 5 chars) and trigrams (all >= 4 chars)
        // from the stop-word-filtered query. These represent specific named
        // concepts ("Mpemba effect", "Riemann zeta function"). If the query has
        // such phrases but NONE appear in any top-5 node label, the individual
        // keyword matches on common sub-words (water, prime, distribution) are
        // noise rather than real knowledge.
        let query_tokens_nonstop: Vec<String> = query.to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { ' ' })
            .collect::<String>()
            .split_whitespace()
            .map(|w| w.to_string())
            .collect();

        let mut specific_phrases: Vec<String> = Vec::new();
        if query_tokens_nonstop.len() >= 2 {
            for w in query_tokens_nonstop.windows(2) {
                if w[0].len() >= 5 && w[1].len() >= 5 {
                    specific_phrases.push(format!("{} {}", w[0], w[1]));
                }
            }
        }
        if query_tokens_nonstop.len() >= 3 {
            for w in query_tokens_nonstop.windows(3) {
                if w[0].len() >= 4 && w[1].len() >= 4 && w[2].len() >= 4 {
                    specific_phrases.push(format!("{} {} {}", w[0], w[1], w[2]));
                }
            }
        }
        // specificity_miss: two-part test.
        // Part A — the named phrase itself is absent from every top-5 node.
        // Part B — at least one constituent token from any specific phrase is
        //          also absent from ALL top-5 nodes. This rules out false positives
        //          where Lu knows the concept but phrases it differently ("ML" vs
        //          "machine learning"). If "machine" or "learning" appears anywhere,
        //          Lu probably has related knowledge; if "mpemba" appears nowhere,
        //          the concept is genuinely unknown.
        // Crucially, specificity_miss does NOT respect NarrationConfidence::Strong,
        // because common sub-words like "effect" can inflate confidence to Strong
        // via keyword boost even when the named concept is completely absent.
        let specificity_miss = if !specific_phrases.is_empty() {
            let all_phrases_absent = !specific_phrases.iter().any(|phrase| {
                result.activated_nodes.iter().take(5).any(|n| {
                    n.label.to_lowercase().contains(phrase.as_str())
                })
            });
            if all_phrases_absent {
                // At least one token from some specific phrase must be absent from
                // ALL top-5 nodes — the distinctive/rare part of the concept is missing.
                specific_phrases.iter().any(|phrase| {
                    phrase.split_whitespace().any(|token| {
                        result.activated_nodes.iter().take(5).all(|n| {
                            !n.label.to_lowercase().contains(token)
                        })
                    })
                })
            } else {
                false
            }
        } else {
            false
        };

        // Signal 3: Source scatter — incoherent activation across unrelated domains.
        // If no pair of the top-4 nodes shares even one meaningful word (> 4 chars,
        // non-stop), the activation was scattered through hub traversal, not focused.
        let top_wordsets: Vec<HashSet<String>> = result.activated_nodes.iter()
            .take(4)
            .map(|n| {
                n.label.to_lowercase()
                    .split_whitespace()
                    .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
                    .filter(|w| w.len() > 4)
                    .collect::<HashSet<String>>()
            })
            .collect();
        let source_scatter = if top_wordsets.len() >= 3 {
            let any_pair_shares = top_wordsets.iter().enumerate().any(|(i, ws_i)| {
                top_wordsets.iter().enumerate().any(|(j, ws_j)| {
                    j > i && ws_i.intersection(ws_j).count() >= 1
                })
            });
            !any_pair_shares
        } else {
            false
        };

        // Signal 4: Hub contamination — narrated nodes with no keyword connection.
        // If >50% of the narrated nodes contain none of the query keywords,
        // they were likely reached via hub traversal rather than direct matching.
        let narrated_count = narration.facts_used.max(1);
        let hub_contaminated_n = result.activated_nodes.iter()
            .take(narrated_count)
            .filter(|n| {
                !query_keywords.iter().any(|kw| {
                    n.label.to_lowercase().contains(kw.as_str())
                })
            })
            .count();
        let hub_contamination = narrated_count >= 2
            && hub_contaminated_n as f32 / narrated_count as f32 > 0.40;

        // Conversation continuity check (two-pass aware):
        // Since Pass 2 boosting happened in recall(), check whether any
        // topic seeds from the prior turn (or their immediate neighbors)
        // appear in the current top-4 results. If Pass 2 promoted any
        // conversation-relevant nodes into the top-4, that's continuity.
        let conv_continuity = {
            let prior_seeds = self.session_memory.get_topic_seeds(10);
            if prior_seeds.is_empty() {
                false
            } else {
                let top4_hexes: HashSet<&str> = result.activated_nodes.iter()
                    .take(4)
                    .map(|n| n.seed_hex.as_str())
                    .collect();
                // Check direct overlap or neighbor overlap with top-4
                let mut has_overlap = false;
                for ts in prior_seeds {
                    // Direct: topic seed itself is in top-4
                    if top4_hexes.contains(ts.seed_hex.as_str()) {
                        has_overlap = true;
                        break;
                    }
                    // Neighbor: any immediate neighbor of the topic seed is in top-4
                    let neighbors = self.network.node_bonds(&ts.seed_hex);
                    if neighbors.iter().any(|(nh, _, _)| top4_hexes.contains(nh.as_str())) {
                        has_overlap = true;
                        break;
                    }
                }
                has_overlap
            }
        };

        // Combine signals. Signals 3 and 4 respect confidence (Strong narration
        // genuinely found good facts). Signal 2 does NOT — common sub-words can
        // push confidence to Strong even when the named concept is absent.
        let confidence_allows_gap = !matches!(narration.confidence, NarrationConfidence::Strong);

        // When conversation continuity is established, relax keyword coverage
        // threshold (50% → 20%) and suppress specificity_miss — the topic is
        // carried from prior turns, not from the current query alone.
        let effective_kw_threshold = if conv_continuity { 0.20 } else { 0.50 };
        let effective_specificity_miss = if conv_continuity { false } else { specificity_miss };
        {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(r"lu_debug.log") {
                let prior_count = self.session_memory.get_topic_seeds(10).len();
                let top4_labels: Vec<String> = result.activated_nodes.iter()
                    .take(4)
                    .map(|n| n.label.clone())
                    .collect();
                let _ = writeln!(f, "[ask] gap_detect: conv_continuity={} prior_seeds={} top4={:?} query_kw={:?}", conv_continuity, prior_count, top4_labels, query_keywords);
            }
        }

        let knowledge_gap =
            // Basic keyword coverage too low
            (keyword_coverage < effective_kw_threshold
                && matches!(narration.confidence,
                    NarrationConfidence::Weak | NarrationConfidence::Empty))
            // Named concept absent from top nodes — no confidence exemption
            || effective_specificity_miss
            // Top nodes are from completely different domains
            || (source_scatter && confidence_allows_gap)
            // Most narrated nodes have no topical keyword link to the query
            || (hub_contamination && confidence_allows_gap);

        // Extract topic for gap messages
        let topic_for_gap = match &intent {
            QueryIntent::Definition { topic }
            | QueryIntent::Why { topic }
            | QueryIntent::Process { topic }
            | QueryIntent::Who { topic }
            | QueryIntent::Ability { topic }
            | QueryIntent::YesNo { topic }
            | QueryIntent::Think { topic }
            | QueryIntent::Explore { topic }
            | QueryIntent::General { topic } => {
                if !topic.is_empty() { topic.clone() } else { query.to_string() }
            }
            QueryIntent::List { topic, .. } => {
                if !topic.is_empty() { topic.clone() } else { query.to_string() }
            }
            QueryIntent::Comparison { topic_a, topic_b } => {
                format!("{} and {}", topic_a, topic_b)
            }
            _ => query.to_string(),
        };

        let (final_text, suggestion) = if knowledge_gap {
            let gap_text = format!(
                "I don't have strong knowledge about \"{}\". Would you like me to research this?",
                topic_for_gap
            );
            let suggest = format!(
                "I don't have strong knowledge about \"{}\". I can search the web and learn about it — want me to research this topic?",
                topic_for_gap
            );
            (gap_text, Some(suggest))
        } else {
            (narration.text.clone(), None)
        };

        // Phase 7: Build coherence score
        let coherence = match narration.confidence {
            NarrationConfidence::Strong => 0.85,
            NarrationConfidence::Moderate => 0.6,
            NarrationConfidence::Weak => 0.35,
            NarrationConfidence::Empty => 0.0,
        };

        // Build sources from the exact nodes narration selected (in narration order).
        // narration re-ranks by relevance+generality; taking the first N from recall-order
        // showed the wrong nodes (short label-like stubs instead of rich definitions).
        let sources: Vec<(String, String, f32)> = narration.selected_hexes.iter()
            .filter_map(|hex| {
                result.activated_nodes.iter()
                    .find(|n| &n.seed_hex == hex)
                    .map(|n| (n.seed_hex.clone(), n.label.clone(), n.contribution * 100.0))
            })
            .collect();

        // Surface image URLs from narration engine
        let image_urls: Vec<(String, String)> = narration.image_urls.iter()
            .map(|img| (img.url.clone(), img.description.clone()))
            .collect();

        // Update conversation history for subsequent calls
        let meaningful: Vec<&str> = query.split_whitespace()
            .filter(|w| w.len() >= 2)
            .collect();
        if !meaningful.is_empty() {
            self.conversation_history.push(query.to_string());
            if self.conversation_history.len() > 5 {
                self.conversation_history.remove(0);
            }
        }

        eprintln!("[ask] total: {:?}", ask_start.elapsed());
        if let Some(ref mut f) = timing_log { let _ = writeln!(f, "[ask] total: {:?}", ask_start.elapsed()); }
        BrainResponse {
            text: final_text,
            query: query.to_string(),
            atoms_used: narration.facts_used,
            traversal_hops: result.traversal_path.len(),
            coherence_score: coherence,
            sources,
            scar_warnings: result.scar_warnings.clone(),
            image_urls,
            knowledge_gap,
            suggestion,
        }
    }

    /// Perform reasoning on a query — multi-hop inference that goes
    /// beyond simple recall. Uses the same recall pipeline but then
    /// analyzes the traversal paths to find reasoning chains.
    ///
    /// Returns a ReasoningResult with direct answer, chains, and
    /// a synthesized conclusion that may include derived knowledge.
    pub fn reason(&mut self, query: &str, depth: usize) -> reasoning::ReasoningResult {
        // First, do normal recall to get activated nodes + traversal
        let result = self.recall(query, 10, depth.max(2));
        let direct_answer = Self::compose_response(&result);

        // Build lookup maps for reasoning engine (targeted — only traversal path, not full graph)
        let bond_labels: HashMap<String, String> = result.traversal_path.iter()
            .filter_map(|h| {
                self.network.get_bond_by_hex(&h.bond)
                    .map(|b| (h.bond.clone(), b.label.clone()))
            })
            .collect();
        let mut node_hexes: HashSet<String> = result.activated_nodes.iter()
            .map(|n| n.seed_hex.clone())
            .collect();
        for h in &result.traversal_path {
            node_hexes.insert(h.from.clone());
            node_hexes.insert(h.to.clone());
        }
        let node_labels: HashMap<String, String> = node_hexes.iter()
            .filter_map(|hex| {
                self.network.get_node_by_hex(hex)
                    .map(|n| (hex.clone(), n.label.clone()))
            })
            .collect();

        // Run the reasoning engine
        reasoning::reason(
            &direct_answer,
            &result.activated_nodes,
            &result.traversal_path,
            &node_labels,
            &bond_labels,
            depth,
        )
    }

    /// Shape the response based on question intent + recalled knowledge.
    /// This is the layer that makes the brain actually ANSWER questions
    /// instead of just dumping related facts.
    fn formulate_response(intent: &QueryIntent, result: &RecallResult) -> String {
        match intent {
            QueryIntent::Greeting => {
                "Hello! Ask me anything — I'll tell you what I know.".to_string()
            }

            QueryIntent::MetaSelf => {
                "I am AI Factory, an associative memory system. I learn facts \
                 and connect them through bonds. I don't use a language model — \
                 my responses come from pattern matching across my knowledge graph. \
                 Ask me about a topic and I'll share what I've learned.".to_string()
            }

            QueryIntent::Ability { topic } => {
                if result.activated_nodes.is_empty()
                    || result.activated_nodes[0].contribution < 0.10
                {
                    format!(
                        "I don't have knowledge about {} yet. \
                         You can teach me with the learn command.",
                        topic
                    )
                } else {
                    let top = &result.activated_nodes[0];
                    let node_count = result.activated_nodes.iter()
                        .filter(|n| n.contribution >= 0.05)
                        .count();
                    format!(
                        "Yes, I have knowledge about {}. I know {} related facts. \
                         For example: {}",
                        topic, node_count, top.label
                    )
                }
            }

            QueryIntent::YesNo { topic } => {
                if result.activated_nodes.is_empty()
                    || result.activated_nodes[0].contribution < 0.10
                {
                    format!(
                        "I don't have enough knowledge to answer that. \
                         Try teaching me more about: {}",
                        topic
                    )
                } else {
                    let composed = Self::compose_response(result);
                    format!("Based on what I know: {}", composed)
                }
            }

            QueryIntent::Definition { .. } => {
                // Definition questions already work well with compose_response
                // (definition preference + flat detection handles these)
                Self::compose_response(result)
            }

            QueryIntent::Process { topic } => {
                if result.activated_nodes.is_empty()
                    || result.activated_nodes[0].contribution < 0.10
                {
                    format!("I don't have knowledge about how {} works yet.", topic)
                } else {
                    Self::compose_response(result)
                }
            }

            QueryIntent::Why { topic } => {
                if result.activated_nodes.is_empty()
                    || result.activated_nodes[0].contribution < 0.10
                {
                    format!("I don't have enough knowledge to explain why {}.", topic)
                } else {
                    Self::compose_response(result)
                }
            }

            QueryIntent::Comparison { topic_a, topic_b } => {
                if result.activated_nodes.is_empty()
                    || result.activated_nodes[0].contribution < 0.10
                {
                    format!(
                        "I don't have enough knowledge to compare {} and {}.",
                        topic_a, topic_b
                    )
                } else {
                    // Split results by which topic they relate to
                    let a_lower = topic_a.to_lowercase();
                    let b_lower = topic_b.to_lowercase();
                    let mut a_parts: Vec<&str> = Vec::new();
                    let mut b_parts: Vec<&str> = Vec::new();
                    let mut shared: Vec<&str> = Vec::new();

                    for node in &result.activated_nodes {
                        if node.contribution < 0.05 { continue; }
                        let label = node.label.to_lowercase();
                        let has_a = a_lower.split_whitespace()
                            .any(|w| label.contains(w));
                        let has_b = b_lower.split_whitespace()
                            .any(|w| label.contains(w));
                        match (has_a, has_b) {
                            (true, true) => shared.push(&node.label),
                            (true, false) => a_parts.push(&node.label),
                            (false, true) => b_parts.push(&node.label),
                            (false, false) => shared.push(&node.label),
                        }
                    }

                    let mut response = String::new();
                    if let Some(first_a) = a_parts.first() {
                        response.push_str(first_a);
                    }
                    if let Some(first_b) = b_parts.first() {
                        if !response.is_empty() {
                            if !response.ends_with('.') { response.push('.'); }
                            response.push(' ');
                        }
                        response.push_str(first_b);
                    }
                    if let Some(first_shared) = shared.first() {
                        if !response.is_empty() {
                            if !response.ends_with('.') { response.push('.'); }
                            response.push(' ');
                        }
                        response.push_str(first_shared);
                    }

                    if response.is_empty() {
                        Self::compose_response(result)
                    } else {
                        response
                    }
                }
            }

            QueryIntent::List { topic, count } => {
                if result.activated_nodes.is_empty()
                    || result.activated_nodes[0].contribution < 0.10
                {
                    format!("I don't have enough knowledge to list {}.", topic)
                } else {
                    // Gather unique facts, cap at requested count or 5
                    let max_items = count.unwrap_or(5).min(10);
                    Self::compose_list_response(result, max_items)
                }
            }

            QueryIntent::Think { topic } => {
                // Think activates broadly across clusters — lower gate
                if result.activated_nodes.is_empty()
                    || result.activated_nodes[0].contribution < 0.04
                {
                    format!("I don't have enough knowledge to reason about {}.", topic)
                } else {
                    Self::compose_think_response(result)
                }
            }

            QueryIntent::Explore { topic } => {
                // Explore expects flat activation (many nodes, low individual
                // contribution). Use a much lower gate than other intents.
                if result.activated_nodes.is_empty()
                    || result.activated_nodes[0].contribution < 0.02
                {
                    format!("I don't have much knowledge about {} yet.", topic)
                } else {
                    Self::compose_explore_response(result)
                }
            }

            QueryIntent::Who { .. } | QueryIntent::General { .. } => {
                Self::compose_response(result)
            }
        }
    }

    /// Compose a natural response from activated nodes.
    ///
    /// v2 Response Assembler: not just concatenation, but structured composition.
    ///
    /// Pipeline:
    /// 1. Quality gates (empty, diffuse, single dominant)
    /// 2. Score each node (definition boost, fragment penalty, standalone quality)
    /// 3. Select nodes with stemmed-word dedup (catches more near-dupes than Jaccard)
    /// 4. Order: definitions first, then supporting facts, then details
    /// 5. Join with clean punctuation
    fn compose_response(result: &RecallResult) -> String {
        if result.activated_nodes.is_empty() {
            return "I don't have knowledge about that yet.".to_string();
        }

        let nodes = &result.activated_nodes;

        // Quality gate: nothing relevant
        if nodes[0].contribution < 0.10 {
            return "I don't have strong knowledge about that yet.".to_string();
        }

        // Single dominant node (>50%) — it IS the answer
        if nodes.len() == 1 || (nodes.len() > 1 && nodes[0].contribution > 0.50) {
            return Self::clean_sentence(&nodes[0].label);
        }

        // ─── Flat activation detection ───────────────────────
        let is_flat = nodes.len() >= 3
            && (nodes[0].contribution / nodes[2].contribution) < 1.3;

        // ─── Score each node ─────────────────────────────────
        let definition_markers = [
            "is a ", "is the ", "is an ", " are ", " means ",
            " refers to ", "is when ", " defined as ",
        ];
        // Expanded fragment detection: sentences that can't stand alone
        let fragment_indicators = [
            "finally,", "they ", "it ", "this ", "also,",
            "however,", "additionally,", "furthermore,",
            "these ", "those ", "that ", "for example,",
            "in addition,", "moreover,", "such as ",
            "on the other hand,", "in contrast,",
            "as a result,", "therefore,",
        ];

        #[derive(Clone)]
        struct ScoredNode {
            idx: usize,
            score: f32,
            is_hub: bool,
            is_definition: bool,
            is_fragment: bool,
            stemmed_words: HashSet<String>,
        }

        let scored: Vec<ScoredNode> = nodes.iter().enumerate().map(|(i, n)| {
            let lower = n.label.to_lowercase();
            let first_chunk = &lower[..lower.len().min(80)];

            let is_definition = definition_markers.iter()
                .any(|pat| first_chunk.contains(pat));
            let is_fragment = fragment_indicators.iter()
                .any(|pat| lower.starts_with(pat));

            // Hub detection: comprehensive overview sentences that span
            // multiple concepts. These are topic anchors that should lead
            // the response. Identified by structural markers.
            let hub_markers = [
                "spans from", "evolved from", "encompasses",
                "from the", "through the", "including",
                "ranging from", "connects", "protects",
                "combines", "covering", "comprising",
            ];
            let is_hub = lower.len() > 100
                && hub_markers.iter().any(|m| lower.contains(m))
                && n.contribution >= 0.08;

            // Standalone quality: does this sentence work on its own?
            // Hub nodes are comprehensive overviews — best response leaders.
            // Short vocabulary-style entries are great standalone.
            // Long sentences starting with capitalized subjects are good.
            // Fragments starting with pronouns or conjunctions are bad.
            let standalone_bonus = if is_hub { 2.0 }  // hubs lead the response
                else if is_definition { 1.5 }
                else if is_fragment { 0.3 }
                else if lower.len() < 120 { 1.1 } // concise = good
                else { 0.9 };

            let score = n.contribution * standalone_bonus;

            // Build stemmed word set for smarter dedup
            let stemmed_words: HashSet<String> = lower
                .split_whitespace()
                .filter(|w| w.len() >= 3)
                .map(|w| Self::stem(w).to_string())
                .collect();

            ScoredNode { idx: i, score, is_hub, is_definition, is_fragment, stemmed_words }
        }).collect();

        let mut ranked = scored.clone();
        ranked.sort_by(|a, b| b.score.partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal));

        // ─── Node cap ────────────────────────────────────────
        let max_parts = if is_flat { 3 } else { 5 };

        // ─── Select with stemmed dedup ───────────────────────
        // Uses stemmed words so "attracts objects" and "attracting object"
        // are recognized as the same content. More aggressive than raw Jaccard.
        let mut selected: Vec<ScoredNode> = Vec::new();
        let mut used_stems: HashSet<String> = HashSet::new();
        let mut total_contribution = 0.0f32;

        for node in &ranked {
            if selected.len() >= max_parts { break; }
            if !is_flat && total_contribution > 0.85 { break; }
            if nodes[node.idx].contribution < 0.05 { continue; }

            // Skip fragments unless we have fewer than 2 parts
            if node.is_fragment && selected.len() >= 2 { continue; }

            // Stemmed overlap dedup: if >40% of this node's stems
            // are already covered by selected nodes, it's redundant.
            let total_stems = node.stemmed_words.len().max(1);
            let overlap = node.stemmed_words.iter()
                .filter(|s| used_stems.contains(*s))
                .count();
            let overlap_ratio = overlap as f32 / total_stems as f32;
            if overlap_ratio > 0.40 && !selected.is_empty() { continue; }

            used_stems.extend(node.stemmed_words.iter().cloned());
            total_contribution += nodes[node.idx].contribution;
            selected.push(node.clone());
        }

        if selected.is_empty() {
            return "I found related knowledge but nothing strong enough to form a clear answer.".to_string();
        }

        if selected.len() == 1 {
            return Self::clean_sentence(&nodes[selected[0].idx].label);
        }

        // ─── Order: definitions first, then by contribution ──
        // A response that starts with a definition and then adds
        // supporting facts reads much more naturally than one
        // that leads with a random detail.
        selected.sort_by(|a, b| {
            // Hubs come first (comprehensive overviews lead the response)
            match (a.is_hub, b.is_hub) {
                (true, false) => return std::cmp::Ordering::Less,
                (false, true) => return std::cmp::Ordering::Greater,
                _ => {}
            }
            // Then definitions
            match (a.is_definition, b.is_definition) {
                (true, false) => return std::cmp::Ordering::Less,
                (false, true) => return std::cmp::Ordering::Greater,
                _ => {}
            }
            // Within same category, higher score first
            b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
        });

        // ─── Compose final text ──────────────────────────────
        let mut composed = String::new();
        for (i, node) in selected.iter().enumerate() {
            let text = Self::clean_sentence(&nodes[node.idx].label);
            if i > 0 {
                let trimmed = composed.trim_end();
                if !trimmed.ends_with('.') && !trimmed.ends_with('!') && !trimmed.ends_with('?') {
                    composed.push('.');
                }
                composed.push(' ');
            }
            composed.push_str(&text);
        }

        // Ensure final punctuation
        let trimmed = composed.trim_end();
        if !trimmed.ends_with('.') && !trimmed.ends_with('!') && !trimmed.ends_with('?') {
            composed.push('.');
        }

        composed
    }

    /// Clean a sentence for inclusion in a response.
    /// Trims whitespace, ensures first letter is capitalized,
    /// removes trailing fragments.
    fn clean_sentence(s: &str) -> String {
        let trimmed = s.trim();
        if trimmed.is_empty() { return String::new(); }

        // Capitalize first letter if it isn't already
        let mut chars = trimmed.chars();
        let first = chars.next().unwrap();
        if first.is_lowercase() {
            let mut result = first.to_uppercase().to_string();
            result.push_str(chars.as_str());
            result
        } else {
            trimmed.to_string()
        }
    }

    /// Compose a Think response: find connections between knowledge clusters.
    ///
    /// The brain's reasoning strategy:
    /// 1. Group activated nodes into clusters by word overlap
    /// 2. Pick the strongest node from each distinct cluster
    /// 3. Look for "bridge" nodes — those that share words with multiple clusters
    /// 4. Compose: cluster facts → bridge connections → synthesis
    ///
    /// This produces novel synthesis by *connecting* what the brain knows,
    /// not just reciting it. "Could a fish survive on land" activates
    /// fish/gills/water AND land/lungs/air, and bridge nodes about
    /// breathing/oxygen connect them.
    fn compose_think_response(result: &RecallResult) -> String {
        if result.activated_nodes.is_empty() {
            return "I don't have enough knowledge to reason about that.".to_string();
        }

        let nodes = &result.activated_nodes;

        // Step 1: Build word sets for each node
        let node_words: Vec<HashSet<String>> = nodes.iter()
            .map(|n| {
                n.label.to_lowercase()
                    .split_whitespace()
                    .filter(|w| w.len() >= 3)
                    .map(|w| w.to_string())
                    .collect()
            })
            .collect();

        // Step 2: Cluster nodes by word overlap (greedy clustering)
        // Two nodes are in the same cluster if they share >30% words
        let mut cluster_ids: Vec<usize> = (0..nodes.len()).collect();
        for i in 0..nodes.len() {
            for j in (i+1)..nodes.len() {
                if cluster_ids[i] == cluster_ids[j] { continue; }
                let intersection = node_words[i].intersection(&node_words[j]).count();
                let smaller = node_words[i].len().min(node_words[j].len()).max(1);
                let overlap = intersection as f32 / smaller as f32;
                if overlap > 0.30 {
                    // Merge: set j's cluster to i's
                    let old_cluster = cluster_ids[j];
                    let new_cluster = cluster_ids[i];
                    for k in 0..cluster_ids.len() {
                        if cluster_ids[k] == old_cluster {
                            cluster_ids[k] = new_cluster;
                        }
                    }
                }
            }
        }

        // Step 3: Find unique clusters and their best representatives
        let mut cluster_reps: HashMap<usize, Vec<usize>> = HashMap::new();
        for (idx, cid) in cluster_ids.iter().enumerate() {
            cluster_reps.entry(*cid).or_default().push(idx);
        }

        // Sort clusters by the strength of their best member
        let mut clusters: Vec<(usize, Vec<usize>)> = cluster_reps.into_iter().collect();
        clusters.sort_by(|a, b| {
            let best_a = a.1.iter().map(|&i| nodes[i].strength)
                .max_by(|x, y| x.partial_cmp(y).unwrap())
                .unwrap_or(0.0);
            let best_b = b.1.iter().map(|&i| nodes[i].strength)
                .max_by(|x, y| x.partial_cmp(y).unwrap())
                .unwrap_or(0.0);
            best_b.partial_cmp(&best_a).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Step 4: Find bridge nodes (share words with 2+ clusters)
        let mut bridges: Vec<usize> = Vec::new();
        if clusters.len() >= 2 {
            for (idx, words) in node_words.iter().enumerate() {
                let mut clusters_touched = HashSet::new();
                for (cid, members) in &clusters {
                    for &m in members {
                        if m == idx { continue; }
                        let intersection = words.intersection(&node_words[m]).count();
                        if intersection >= 2 {
                            clusters_touched.insert(*cid);
                            break;
                        }
                    }
                }
                if clusters_touched.len() >= 2 {
                    bridges.push(idx);
                }
            }
        }

        // Step 5: Compose the response
        // Take the best node from each cluster (up to 3 clusters)
        // Then add any bridge nodes
        let mut parts: Vec<String> = Vec::new();
        let mut used: HashSet<usize> = HashSet::new();

        // Best from each cluster
        for (_cid, members) in clusters.iter().take(3) {
            if let Some(&best) = members.first() {
                if !used.contains(&best) && nodes[best].contribution >= 0.03 {
                    parts.push(nodes[best].label.clone());
                    used.insert(best);
                }
            }
        }

        // Bridge nodes (the connections between clusters)
        for &bridge_idx in bridges.iter().take(2) {
            if !used.contains(&bridge_idx) && nodes[bridge_idx].contribution >= 0.03 {
                parts.push(nodes[bridge_idx].label.clone());
                used.insert(bridge_idx);
            }
        }

        if parts.is_empty() {
            return Self::compose_response(result);
        }

        // Join with proper punctuation
        let mut composed = String::new();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                let trimmed = composed.trim_end();
                if !trimmed.ends_with('.') && !trimmed.ends_with('!') && !trimmed.ends_with('?') {
                    composed.push('.');
                }
                composed.push(' ');
            }
            composed.push_str(part.trim());
        }
        if !composed.trim_end().ends_with('.') && !composed.trim_end().ends_with('!')
            && !composed.trim_end().ends_with('?') {
            composed.push('.');
        }
        composed
    }

    /// Compose an Explore response: maximize diversity across the result set.
    ///
    /// Unlike compose_response which caps at 3-5 nodes and prefers definitions,
    /// Explore deliberately seeks variety — different topics, different angles.
    /// Uses greedy diversity selection: pick the node most UNLIKE what's
    /// already selected.
    fn compose_explore_response(result: &RecallResult) -> String {
        if result.activated_nodes.is_empty() {
            return "I don't have knowledge about that yet.".to_string();
        }

        let nodes = &result.activated_nodes;
        let max_parts = 7; // More generous than normal compose (3-5)

        // Build word sets
        let node_words: Vec<HashSet<String>> = nodes.iter()
            .map(|n| {
                n.label.to_lowercase()
                    .split_whitespace()
                    .filter(|w| w.len() >= 3)
                    .map(|w| w.to_string())
                    .collect()
            })
            .collect();

        let mut selected: Vec<usize> = Vec::new();
        let mut selected_words: HashSet<String> = HashSet::new();

        // Start with the strongest node
        if !nodes.is_empty() && nodes[0].contribution >= 0.03 {
            selected.push(0);
            selected_words.extend(node_words[0].iter().cloned());
        }

        // Greedily add the most diverse remaining node
        while selected.len() < max_parts {
            let mut best_idx: Option<usize> = None;
            let mut best_novelty: f32 = -1.0;

            for (idx, words) in node_words.iter().enumerate() {
                if selected.contains(&idx) { continue; }
                if nodes[idx].contribution < 0.03 { continue; }

                // Novelty = fraction of this node's words NOT in selected set
                let new_words = words.iter()
                    .filter(|w| !selected_words.contains(*w))
                    .count();
                let novelty = if words.is_empty() { 0.0 }
                    else { new_words as f32 / words.len() as f32 };

                // Weight by contribution so we don't pick total noise
                let score = novelty * nodes[idx].contribution.sqrt();

                if score > best_novelty {
                    best_novelty = score;
                    best_idx = Some(idx);
                }
            }

            match best_idx {
                Some(idx) if best_novelty > 0.05 => {
                    selected.push(idx);
                    selected_words.extend(node_words[idx].iter().cloned());
                }
                _ => break, // No more diverse nodes worth adding
            }
        }

        if selected.is_empty() {
            return Self::compose_response(result);
        }

        // Sort selected by original rank to maintain narrative flow
        selected.sort();

        let mut composed = String::new();
        for (i, &idx) in selected.iter().enumerate() {
            if i > 0 {
                let trimmed = composed.trim_end();
                if !trimmed.ends_with('.') && !trimmed.ends_with('!') && !trimmed.ends_with('?') {
                    composed.push('.');
                }
                composed.push(' ');
            }
            composed.push_str(nodes[idx].label.trim());
        }
        if !composed.trim_end().ends_with('.') && !composed.trim_end().ends_with('!')
            && !composed.trim_end().ends_with('?') {
            composed.push('.');
        }
        composed
    }

    /// Compose a list-style response from activated nodes.
    /// Used for List intent — presents facts as enumerated items.
    fn compose_list_response(result: &RecallResult, max_items: usize) -> String {
        if result.activated_nodes.is_empty() {
            return "I don't have knowledge about that yet.".to_string();
        }

        let mut parts: Vec<String> = Vec::new();
        for node in &result.activated_nodes {
            if parts.len() >= max_items { break; }
            if node.contribution < 0.05 { continue; }

            // Dedup (same as compose_response)
            let candidate_words: HashSet<String> = node.label.to_lowercase()
                .split_whitespace()
                .filter(|w| w.len() >= 3)
                .map(|w| w.to_string())
                .collect();

            let is_duplicate = parts.iter().any(|existing| {
                let existing_words: HashSet<String> = existing.to_lowercase()
                    .split_whitespace()
                    .filter(|w| w.len() >= 3)
                    .map(|w| w.to_string())
                    .collect();
                let intersection = candidate_words.intersection(&existing_words).count();
                let union = candidate_words.union(&existing_words).count();
                if union == 0 { return false; }
                intersection as f32 / union as f32 > 0.45
            });

            if !is_duplicate {
                parts.push(node.label.clone());
            }
        }

        if parts.is_empty() {
            return "I found related knowledge but nothing specific enough.".to_string();
        }

        if parts.len() == 1 {
            return parts[0].clone();
        }

        // Format as a list with periods between items
        let mut composed = String::new();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                let trimmed = composed.trim_end();
                if !trimmed.ends_with('.') && !trimmed.ends_with('!') && !trimmed.ends_with('?') {
                    composed.push('.');
                }
                composed.push(' ');
            }
            composed.push_str(part.trim());
        }
        if !composed.trim_end().ends_with('.') {
            composed.push('.');
        }
        composed
    }

    // ─── Reports ────────────────────────────────────────

    /// Get the seed report from the last recall.
    pub fn last_seed_report(&self) -> Option<&RecallResult> {
        self.last_recall.as_ref()
    }

    pub fn stats(&self) -> NetworkStats {
        self.network.stats()
    }

    /// Export graph data for the visualizer.
    /// Returns JSON-ready data: nodes with labels and bond counts,
    /// plus bond source/target pairs.
    pub fn viz_graph_json(&self) -> String {
        let labels = self.network.node_label_map();
        let bond_counts = self.network.bond_counts();
        let confidences = self.network.node_confidence_map();
        let bonds_raw = self.network.viz_bonds();
        let stats = self.network.stats();

        // Build a quick bond-neighbor lookup: node_hex -> Vec<neighbor_label>
        let mut neighbor_labels: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for (src, tgt, _w) in &bonds_raw {
            if let Some(tgt_label) = labels.get(tgt) {
                neighbor_labels.entry(src.clone())
                    .or_default().push(tgt_label.clone());
            }
            if let Some(src_label) = labels.get(src) {
                neighbor_labels.entry(tgt.clone())
                    .or_default().push(src_label.clone());
            }
        }

        // Build node array (all active nodes)
        let nodes: Vec<serde_json::Value> = labels.iter().map(|(hex, label)| {
            let bc = bond_counts.get(hex).copied().unwrap_or(0);
            let conf = confidences.get(hex).cloned()
                .unwrap_or_else(|| "unknown".to_string());
            let connected: Vec<&str> = neighbor_labels.get(hex)
                .map(|v| v.iter().take(8).map(|s| s.as_str()).collect())
                .unwrap_or_default();
            serde_json::json!({
                "id": hex,
                "label": label,
                "bondCount": bc,
                "confidence": conf,
                "connectedLabels": connected,
            })
        }).collect();

        // Build bond array (source/target pairs)
        let bonds: Vec<serde_json::Value> = bonds_raw.iter().map(|(src, tgt, w)| {
            serde_json::json!({
                "source": src,
                "target": tgt,
                "weight": w,
            })
        }).collect();

        serde_json::json!({
            "nodes": nodes,
            "bonds": bonds,
            "total_nodes": stats.active_nodes,
            "total_bonds": stats.active_bonds,
        }).to_string()
    }

    pub fn scars(&self) -> &[graph::Scar] {
        self.network.scars()
    }

    pub fn terminal_bond(&self) -> &TerminalBond {
        &self.terminal_bond
    }

    // ─── Conversation Persistence ────────────────────────

    /// List all conversation sessions (metadata only).
    pub fn list_conversations(&self) -> Vec<conversations::SessionMeta> {
        self.conversations.list()
    }

    /// Create a new conversation. Returns session ID.
    pub fn new_conversation(&mut self, name: Option<&str>) -> String {
        self.conversations.new_session(name)
    }

    /// Get a full conversation session with messages.
    pub fn get_conversation(&self, id: &str) -> Option<conversations::ConversationSession> {
        self.conversations.get_session(id)
    }

    /// Set the active conversation.
    pub fn set_active_conversation(&mut self, id: &str) {
        self.conversations.set_active(id);
    }

    /// Get active conversation ID.
    pub fn active_conversation_id(&self) -> Option<&str> {
        self.conversations.active_id()
    }

    /// Add a message to a conversation. Returns message ID.
    pub fn add_conversation_message(
        &mut self,
        session_id: &str,
        role: conversations::MessageRole,
        content: &str,
        images: Vec<String>,
        sources: Vec<(String, String, f32)>,
    ) -> Option<String> {
        self.conversations.add_message(session_id, role, content, images, sources)
    }

    /// Rename a conversation.
    pub fn rename_conversation(&mut self, id: &str, new_name: &str) -> bool {
        self.conversations.rename(id, new_name)
    }

    /// Delete a conversation.
    pub fn delete_conversation(&mut self, id: &str) -> bool {
        self.conversations.delete(id)
    }

    /// Get or create the active conversation session ID.
    /// On first call within a process, picks up the most recent session from
    /// disk (so MCP calls resume an existing conversation) or creates a fresh one.
    pub fn ensure_active_session(&mut self) -> String {
        if let Some(id) = self.active_conversation_id() {
            return id.to_string();
        }
        // Resume the most recently updated session from disk, if any.
        let sessions = self.conversations.list();
        if let Some(most_recent) = sessions.first() {
            let id = most_recent.id.clone();
            self.set_active_conversation(&id);
            return id;
        }
        // No sessions yet — create the first one.
        self.new_conversation(Some("Lu Session"))
    }

    /// Restore conversation_history from the most recent persisted session.
    /// Called during load() so the recall/narration pipeline retains context
    /// from the previous session even after a process restart.
    fn restore_conversation_history(&mut self) {
        let sessions = self.conversations.list();
        let most_recent_id = match sessions.first() {
            Some(s) => s.id.clone(),
            None => return,
        };
        let session = match self.conversations.get_session(&most_recent_id) {
            Some(s) => s,
            None => return,
        };
        // Collect the last 5 user messages in chronological order.
        let user_queries: Vec<String> = session.messages.iter()
            .filter(|m| m.role == conversations::MessageRole::User)
            .rev()
            .take(5)
            .map(|m| m.content.clone())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        if !user_queries.is_empty() {
            eprintln!("[brain] Restored {} prior queries from conversation history", user_queries.len());
            self.conversation_history = user_queries;
        }
        // Set this as the active session so new messages append to it.
        self.set_active_conversation(&most_recent_id);
    }

    // ─── Persistence (cross-platform) ───────────────────

    /// Save brain state to disk.
    /// Writes bincode (fast, compact) as the primary format.
    /// Also writes JSON as a human-readable backup.
    /// Also writes bond_fingerprint.hex on first save — locks this brain
    /// directory to this terminal bond forever. A foreign brain cannot
    /// load from this directory once the fingerprint is written.
    /// Fast save — writes bincode only. Called after every tool operation.
    /// At 52K+ nodes the JSON pretty-print takes >60 s; bincode is ~200 ms.
    /// Use `save_full()` for an explicit human-readable backup.
    pub fn save(&self) -> Result<(), String> {
        let path = self.storage_path.as_ref()
            .ok_or("No storage path set")?;

        std::fs::create_dir_all(path)
            .map_err(|e| format!("Failed to create directory: {}", e))?;

        let bin_file = path.join("brain_state.bin");
        let bin_data = bincode::serialize(self)
            .map_err(|e| format!("Bincode serialization failed: {}", e))?;
        std::fs::write(&bin_file, &bin_data)
            .map_err(|e| format!("Bincode write failed: {}", e))?;

        // Write bond fingerprint on first save (identity lock — never overwritten).
        let fp_file = path.join("bond_fingerprint.hex");
        if !fp_file.exists() {
            let ai_hex = self.terminal_bond.ai_seed().to_hex();
            let _ = std::fs::write(&fp_file, &ai_hex);
            eprintln!("[brain] Bond fingerprint written: {}", &ai_hex[..16]);
        }

        Ok(())
    }

    /// Full save — bincode + JSON backup. Slow at large graph sizes.
    /// Only call this from the explicit `brain_save` MCP tool or on shutdown.
    pub fn save_full(&self) -> Result<(), String> {
        self.save()?;

        let path = self.storage_path.as_ref()
            .ok_or("No storage path set")?;

        let json_file = path.join("brain_state.json");
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("JSON serialization failed: {}", e))?;
        std::fs::write(&json_file, json)
            .map_err(|e| format!("JSON write failed: {}", e))?;

        Ok(())
    }

    /// Load brain state from disk.
    /// Tries bincode first (fast), falls back to JSON (backward compat).
    /// Verifies bond_fingerprint.hex if it exists — refuses to load a
    /// brain whose bond does not match the fingerprint for this directory.
    /// Creates the fingerprint on first load if none exists (backward compat).
    pub fn load(path: &PathBuf) -> Result<Self, String> {
        let bin_file = path.join("brain_state.bin");
        let json_file = path.join("brain_state.json");

        let mut brain: Self = if bin_file.exists() {
            // Fast path: bincode
            let start = std::time::Instant::now();
            let data = std::fs::read(&bin_file)
                .map_err(|e| format!("Bincode read failed: {}", e))?;
            let brain: Self = bincode::deserialize(&data)
                .map_err(|e| format!("Bincode deserialization failed: {}", e))?;
            eprintln!("[brain] Loaded from bincode ({:.1}MB) in {:.1}s",
                data.len() as f64 / 1_048_576.0,
                start.elapsed().as_secs_f64());
            brain
        } else if json_file.exists() {
            // Fallback: JSON (first load after migration)
            let start = std::time::Instant::now();
            let json = std::fs::read_to_string(&json_file)
                .map_err(|e| format!("JSON read failed: {}", e))?;
            let brain: Self = serde_json::from_str(&json)
                .map_err(|e| format!("JSON deserialization failed: {}", e))?;
            eprintln!("[brain] Loaded from JSON ({:.1}MB) in {:.1}s — will save as bincode next time",
                json.len() as f64 / 1_048_576.0,
                start.elapsed().as_secs_f64());
            brain
        } else {
            return Err(format!("No brain state found at {:?} (checked .bin and .json)", path));
        };
        brain.storage_path = Some(path.clone());
        brain.conversations = conversations::ConversationStore::new(path);
        // Restore conversation_history from the most recent persisted session so
        // the recall pipeline has cross-restart context from the prior session.
        brain.restore_conversation_history();
        // Curiosity always starts enabled with auto_resolve on.
        // The persisted mirror (curiosity_auto_resolve) can override this
        // once the user has explicitly configured it via the MCP tool.
        brain.curiosity.enabled = true;
        brain.curiosity.auto_resolve = true;

        // Bond fingerprint verification — DialectForge identity gate.
        // One terminal, one bond, one brain. No substitutions.
        let fp_file = path.join("bond_fingerprint.hex");
        let brain_ai_hex = brain.terminal_bond.ai_seed().to_hex();

        if fp_file.exists() {
            let stored = std::fs::read_to_string(&fp_file)
                .unwrap_or_default();
            if stored.trim() != brain_ai_hex.as_str() {
                return Err(format!(
                    "Bond fingerprint mismatch — this brain was not born here.\n\
                     This directory is locked to a different terminal bond.\n\
                     Stored:  {}\n\
                     Got:     {}\n\
                     Delete bond_fingerprint.hex manually to reset (use with caution).",
                    stored.trim(),
                    brain_ai_hex
                ));
            }
        } else {
            // No fingerprint yet — this is the first verified load.
            // Claim this directory for this brain's bond.
            let _ = std::fs::write(&fp_file, &brain_ai_hex);
            eprintln!("[brain] Bond fingerprint created: {}", &brain_ai_hex[..16]);
        }

        Ok(brain)
    }

    // ─── Internal ───────────────────────────────────────

    /// Check if this exact content already exists in the network.
    /// Uses normalized string comparison: lowercase, trimmed,
    /// whitespace collapsed. This is an identity check, not similarity.
    ///
    /// "The sky is blue" == "the  sky  is  blue" (same fact)
    /// "The sky is blue" != "The ocean is blue" (different facts)
    fn find_existing_content(&self, content: &str) -> Option<Seed> {
        let normalized = Self::normalize_content(content);

        for (hex, node) in self.network.active_nodes() {
            let node_normalized = Self::normalize_content(&node.label);
            if node_normalized == normalized {
                if let Ok(seed) = Seed::from_hex(hex) {
                    return Some(seed);
                }
            }
        }
        None
    }

    /// Normalize content for identity comparison.
    /// Lowercase, trim, collapse whitespace to single spaces.
    fn normalize_content(s: &str) -> String {
        s.trim()
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Auto-associate a new node with related existing knowledge.
    ///
    /// Uses two signals combined:
    /// - Word overlap (Jaccard similarity of word sets): reliable even
    ///   when the pattern engine is young. "A dog is an animal" and
    ///   "A cat is an animal" share {is, an, animal} → Jaccard = 3/5
    /// - Pattern similarity: improves as the engine learns associations
    ///
    /// The combined score weights words heavily (0.7) while the pattern
    /// engine is young. As the engine matures through learn_from_association,
    /// pattern similarity becomes more meaningful.
    fn auto_associate(&mut self, new_seed: &Seed, threshold: f32, max: usize) {
        let new_node = match self.network.get_node(new_seed) {
            Some(n) => n,
            None => return,
        };
        let new_hex = new_seed.to_hex();
        let new_pattern = new_node.pattern.clone();

        // Extract words for overlap comparison
        let new_words: HashSet<String> = new_node.label
            .to_lowercase()
            .split_whitespace()
            .filter(|w| w.len() >= 2)
            .map(|w| w.to_string())
            .collect();

        let mut candidates: Vec<(Seed, f32)> = Vec::new();

        for (hex, node) in self.network.active_nodes() {
            if hex == new_hex { continue; }

            let node_words: HashSet<String> = node.label
                .to_lowercase()
                .split_whitespace()
                .filter(|w| w.len() >= 2)
                .map(|w| w.to_string())
                .collect();

            // Jaccard similarity of word sets
            let intersection = new_words.intersection(&node_words).count();
            let union = new_words.union(&node_words).count();
            let jaccard = if union > 0 {
                intersection as f32 / union as f32
            } else {
                0.0
            };

            // Pattern cosine similarity
            let pattern_sim: f32 = if node.pattern.len() == new_pattern.len() {
                node.pattern.iter()
                    .zip(new_pattern.iter())
                    .map(|(a, b)| a * b)
                    .sum()
            } else {
                0.0
            };

            // Combined: words weighted 0.7, pattern 0.3
            // As pattern engine matures, these weights could shift
            let score = jaccard * 0.7 + pattern_sim.max(0.0) * 0.3;

            if score >= threshold {
                if let Ok(seed) = Seed::from_hex(hex) {
                    candidates.push((seed, score));
                }
            }
        }

        candidates.sort_by(|a, b| {
            b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.truncate(max);

        for (target, _) in candidates {
            let _ = self.network.add_bond(
                new_seed, &target,
                "auto-association".to_string(),
                0.5,
                "brain",
            );
        }
    }

    // ─── Reindex: Efficient Word-Level Bond Discovery ──────────────

    /// Minimum stop words for reindex — only truly meaningless function
    /// words. We WANT words like "true", "cause", "number" to create bonds here.
    const REINDEX_STOP: &'static [&'static str] = &[
        "a", "an", "the", "is", "are", "was", "were", "be", "been", "being",
        "am", "do", "does", "did", "has", "have", "had", "it", "its",
        "i", "me", "my", "we", "our", "you", "your",
        "in", "on", "at", "to", "for", "of", "with", "by", "from",
        "and", "or", "but", "not", "no", "so", "if",
        "as", "that", "this", "than",
    ];

    /// Scan the entire graph and create word-level bonds between nodes.
    ///
    /// **Algorithm** (designed to stay fast at 100k+ nodes):
    ///
    /// 1. **Build inverted index** — one pass through all nodes.
    ///    For each node, extract lowercase words, stem them, skip
    ///    stop words, and record: `stemmed_word → Vec<seed_hex>`.
    ///    Cost: O(N × W) where W = avg words per node.
    ///
    /// 2. **Frequency cutoff** — words appearing in more than
    ///    `max_freq_pct`% of nodes are auto-skipped. "means" might
    ///    appear in 40% of foundation atoms — bonding all of them
    ///    would turn the graph into soup. Default 5%.
    ///
    /// 3. **Bond creation** — for each word bucket with 2+ nodes,
    ///    bond every pair in the bucket. Uses `add_bond` which
    ///    handles dedup (strengthens if exists).
    ///    Cost: O(Σ bucket_size²) — but frequency cutoff keeps
    ///    bucket sizes small.
    ///
    /// 4. **Max bonds per word** — if a word bucket has too many
    ///    nodes, only bond the first `max_per_word` pairs to avoid
    ///    quadratic blowup on medium-frequency words. Default 50.
    ///
    /// Returns: (bonds_created, bonds_strengthened, words_processed, words_skipped_freq)
    pub fn reindex_bonds(
        &mut self,
        max_freq_pct: f32,
        max_per_word: usize,
    ) -> (usize, usize, usize, usize) {
        let stop_set: HashSet<&str> = Self::REINDEX_STOP.iter().copied().collect();

        // ── Phase 1: Build inverted index ──
        // word_stem → Vec<seed_hex>
        let mut inverted: HashMap<String, Vec<String>> = HashMap::new();
        // Collect node data first to avoid borrow issues
        let node_data: Vec<(String, String)> = self.network.active_nodes()
            .map(|(hex, node)| (hex.to_string(), node.label.clone()))
            .collect();

        let node_count = node_data.len();

        for (hex, label) in &node_data {
            let words: Vec<String> = label
                .to_lowercase()
                .split(|c: char| !c.is_alphanumeric() && c != '-')
                .filter(|w| w.len() >= 3)
                .filter(|w| !stop_set.contains(w))
                .map(|w| Self::stem(w).to_string())
                .collect();

            for word in words {
                inverted.entry(word)
                    .or_default()
                    .push(hex.clone());
            }
        }

        // ── Phase 2: Frequency cutoff ──
        let freq_threshold = (node_count as f32 * max_freq_pct / 100.0).max(2.0) as usize;
        let mut words_skipped_freq: usize = 0;
        let mut words_to_process: Vec<(String, Vec<String>)> = Vec::new();

        for (word, seeds) in inverted {
            if seeds.len() < 2 {
                continue; // need at least 2 nodes to bond
            }
            if seeds.len() > freq_threshold {
                words_skipped_freq += 1;
                continue; // too common, would create soup
            }
            words_to_process.push((word, seeds));
        }

        let words_processed = words_to_process.len();

        // ── Phase 3: Create bonds ──
        let mut bonds_created: usize = 0;
        let mut bonds_strengthened: usize = 0;

        // Pre-build a set of existing bond pairs so we can distinguish
        // new bonds from strengthened ones without calling private methods.
        let mut existing_pairs: HashSet<(String, String)> = HashSet::new();
        for (hex, _) in &node_data {
            for (target_hex, _, _) in self.network.node_bonds(hex) {
                // Store both directions so lookup is order-independent
                let pair = if *hex < target_hex {
                    (hex.clone(), target_hex.clone())
                } else {
                    (target_hex.clone(), hex.clone())
                };
                existing_pairs.insert(pair);
            }
        }

        for (word, seeds) in &words_to_process {
            let mut pairs_done: usize = 0;

            for i in 0..seeds.len() {
                if pairs_done >= max_per_word {
                    break;
                }
                for j in (i + 1)..seeds.len() {
                    if pairs_done >= max_per_word {
                        break;
                    }

                    let seed_a = match Seed::from_hex(&seeds[i]) {
                        Ok(s) => s,
                        Err(_) => continue,
                    };
                    let seed_b = match Seed::from_hex(&seeds[j]) {
                        Ok(s) => s,
                        Err(_) => continue,
                    };

                    let pair_key = if seeds[i] < seeds[j] {
                        (seeds[i].clone(), seeds[j].clone())
                    } else {
                        (seeds[j].clone(), seeds[i].clone())
                    };
                    let was_existing = existing_pairs.contains(&pair_key);

                    let result = self.network.add_bond(
                        &seed_a, &seed_b,
                        format!("reindex:{}", word),
                        0.3,
                        "reindex",
                    );

                    if result.is_ok() {
                        if was_existing {
                            bonds_strengthened += 1;
                        } else {
                            bonds_created += 1;
                            // Add to set so subsequent words don't double-count
                            existing_pairs.insert(pair_key);
                        }
                    }
                    pairs_done += 1;
                }
            }
        }

        (bonds_created, bonds_strengthened, words_processed, words_skipped_freq)
    }


    /// Simple English stemmer — strips common suffixes so singular/plural
    /// and tense variants match the same root.
    ///
    /// "glaciers" → "glacier"   (plural s)
    /// "birds"    → "bird"      (plural s)
    /// "running"  → "run"       (present participle)
    /// "formed"   → "form"      (past tense)
    ///
    /// NOT a full Porter stemmer. Handles the most common cases without
    /// over-stemming. "grass" stays "grass" (ends in "ss"),
    /// "bonus" stays "bonus" (ends in "us").
    fn stem(word: &str) -> &str {
        // Strip "-ing" only if word is long enough to survive it
        if word.len() > 5 && word.ends_with("ing") {
            return &word[..word.len() - 3];
        }
        // Strip "-ed"
        if word.len() > 4 && word.ends_with("ed") {
            return &word[..word.len() - 2];
        }
        // Strip trailing "s" for plurals — but guard common false-positives
        if word.len() > 3
            && word.ends_with('s')
            && !word.ends_with("ss")   // "grass" → stays "grass"
            && !word.ends_with("us")   // "bonus" → stays "bonus"
            && !word.ends_with("is")   // "analysis" → stays "analysis"
        {
            return &word[..word.len() - 1];
        }
        word
    }

    /// Filter activated nodes to keep only a connected subgraph.
    ///
    /// The problem: recall activates nodes based on keyword matching,
    /// but "tell me about cats" might activate "A cat is an animal"
    /// AND "Fiction is a genre" (both share the word "a"). These nodes
    /// live in completely different graph regions.
    ///
    /// The fix: start from the strongest node. Keep only nodes that
    /// are bonded (directly or through one hop) to already-kept nodes.
    /// This prunes isolated noise while preserving coherent clusters.
    fn filter_connected_subgraph(&self, mut result: RecallResult) -> RecallResult {
        if result.activated_nodes.len() <= 1 {
            return result;
        }

        // Seed with the top 3 strongest nodes (not just top 1).
        // Using only the strongest can bias toward one subgraph,
        // dropping valid nodes that live in a parallel cluster.
        // E.g., "what is physics" — the #1 node might be about
        // game physics engines, while #2 is the actual definition.
        // Seeding from top 3 preserves both clusters.
        let seed_count = result.activated_nodes.len().min(3);
        let mut kept: Vec<String> = result.activated_nodes[..seed_count]
            .iter().map(|n| n.seed_hex.clone()).collect();
        let mut kept_set: HashSet<String> = kept.iter().cloned().collect();

        // For each remaining node (in strength order), check if it's
        // connected to any already-kept node via a bond
        for node in result.activated_nodes.iter().skip(1) {
            let is_connected = kept.iter().any(|kept_hex| {
                self.network.bond_weight(kept_hex, &node.seed_hex).is_some()
            });

            if is_connected {
                kept.push(node.seed_hex.clone());
                kept_set.insert(node.seed_hex.clone());
            } else {
                // Not directly bonded — check if bonded through one hop
                // (bonded to a neighbor of a kept node)
                let one_hop = kept.iter().any(|kept_hex| {
                    // Get neighbors of kept node via bond index
                    let neighbors = self.network.node_bonds(kept_hex);
                    neighbors.iter().any(|(_, n_hex, _)| {
                        self.network.bond_weight(n_hex, &node.seed_hex).is_some()
                    })
                });

                if one_hop {
                    kept.push(node.seed_hex.clone());
                    kept_set.insert(node.seed_hex.clone());
                }
                // else: node is isolated noise — drop it
            }
        }

        // Filter the result to only kept nodes
        result.activated_nodes.retain(|n| kept_set.contains(&n.seed_hex));

        // Recompute contributions after filtering
        let total: f32 = result.activated_nodes.iter().map(|n| n.strength).sum();
        let total = if total > 0.0 { total } else { 1.0 };
        for node in &mut result.activated_nodes {
            node.contribution = node.strength / total;
        }

        result
    }

    /// Check content against all scars. Returns list of conflict descriptions.
    /// Uses re-encoding (not stored fingerprint) + word overlap as fallback,
    /// because the pattern engine drifts through learn_from_association —
    /// the same text produces different vectors at different times.
    fn check_scar_conflicts(&self, content: &str) -> Vec<String> {
        // Word-overlap scar detection.
        // Pattern engine can't discriminate (all dots ~0.999), so we use
        // text-based matching only. Two checks:
        //   1. Meaningful-word overlap (stop words stripped) > 50%
        //   2. Exact substring containment in either direction
        const SCAR_STOP: &[&str] = &[
            "a", "an", "the", "is", "are", "was", "were", "be", "been",
            "am", "do", "does", "did", "has", "have", "had", "it", "its",
            "i", "me", "my", "we", "our", "you", "your", "he", "she",
            "him", "her", "his", "they", "them", "their", "this", "that",
            "these", "those", "at", "for", "of", "with", "by", "from",
            "and", "or", "but", "not", "no", "to", "in", "on", "as",
        ];
        let stop: HashSet<&str> = SCAR_STOP.iter().copied().collect();

        let content_lower = content.to_lowercase();
        let content_words: HashSet<String> = content_lower
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|w| !w.is_empty() && !stop.contains(w.as_str()))
            .collect();

        let mut conflicts: Vec<String> = Vec::new();
        for scar in self.network.scars() {
            let scar_lower = scar.label.to_lowercase();

            // Check 1: substring containment (catches exact or near-exact)
            if content_lower.contains(&scar_lower) || scar_lower.contains(&content_lower) {
                conflicts.push(format!("{}: {}", scar.label, scar.reason));
                continue;
            }

            // Check 2: meaningful word overlap
            let scar_words: HashSet<String> = scar_lower
                .split_whitespace()
                .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
                .filter(|w| !w.is_empty() && !stop.contains(w.as_str()))
                .collect();
            if scar_words.is_empty() || content_words.is_empty() { continue; }
            let overlap = scar_words.intersection(&content_words).count();
            let min_len = scar_words.len().min(content_words.len());
            let overlap_ratio = overlap as f32 / min_len as f32;
            if overlap_ratio > 0.5 {
                conflicts.push(format!("{}: {}", scar.label, scar.reason));
            }
        }
        conflicts
    }

    /// Find a node seed by hex prefix (for short-seed UX in terminal).
    pub fn find_seed_by_prefix(&self, prefix: &str) -> Option<Seed> {
        let prefix_lower = prefix.to_lowercase();
        for (hex, _node) in self.network.active_nodes() {
            if hex.to_lowercase().starts_with(&prefix_lower) {
                if let Ok(seed) = Seed::from_hex(hex) {
                    return Some(seed);
                }
            }
        }
        // Also check revoked nodes (scars)
        for scar in self.network.scars() {
            let scar_hex = scar.revoked_seed.to_hex();
            if scar_hex.to_lowercase().starts_with(&prefix_lower) {
                return Some(scar.revoked_seed.clone());
            }
        }
        None
    }

    fn network_node_similarities(&self, pattern: &[f32], threshold: f32) -> Vec<(Seed, f32)> {
        let mut results = Vec::new();
        for (hex, node) in self.network.active_nodes() {
            if node.pattern.len() != pattern.len() || node.pattern.is_empty() {
                continue;
            }
            let sim: f32 = node.pattern.iter()
                .zip(pattern.iter())
                .map(|(a, b)| a * b)
                .sum();
            if sim >= threshold {
                if let Ok(seed) = Seed::from_hex(hex) {
                    results.push((seed, sim));
                }
            }
        }
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    /// Pattern similarity scan restricted to a candidate set of node hexes.
    /// Used by recall() to avoid scanning all 52K+ nodes — only the ~200–2000
    /// nodes that share a query word stem are checked. Falls back to a full
    /// scan when candidates is empty (query has no word-index hits).
    fn network_node_similarities_filtered(
        &self,
        pattern: &[f32],
        threshold: f32,
        candidates: &HashSet<String>,
    ) -> Vec<(Seed, f32)> {
        if candidates.is_empty() {
            return self.network_node_similarities(pattern, threshold);
        }
        let mut results = Vec::new();
        for hex in candidates {
            let node = match self.network.get_node_by_hex(hex) {
                Some(n) if !n.is_revoked() => n,
                _ => continue,
            };
            if node.pattern.len() != pattern.len() || node.pattern.is_empty() {
                continue;
            }
            let sim: f32 = node.pattern.iter()
                .zip(pattern.iter())
                .map(|(a, b)| a * b)
                .sum();
            if sim >= threshold {
                if let Ok(seed) = Seed::from_hex(hex) {
                    results.push((seed, sim));
                }
            }
        }
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    /// Pattern similarity scan restricted to Media nodes only.
    /// Used by cross-modal recall to find images/audio/video that
    /// match a text query's pattern vector. Uses a lower threshold
    /// than text matching because the modality offset (~0.02) pushes
    /// Media vectors into a slightly different region.
    fn network_media_similarities(
        &self,
        pattern: &[f32],
        threshold: f32,
    ) -> Vec<(Seed, f32)> {
        let mut results = Vec::new();
        for (hex, node) in self.network.active_nodes() {
            if node.node_type != NodeType::Media {
                continue;
            }
            if node.pattern.len() != pattern.len() || node.pattern.is_empty() {
                continue;
            }
            let sim: f32 = node.pattern.iter()
                .zip(pattern.iter())
                .map(|(a, b)| a * b)
                .sum();
            if sim >= threshold {
                if let Ok(seed) = Seed::from_hex(hex) {
                    results.push((seed, sim));
                }
            }
        }
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    // ─── Thinking Layer (Coordinator) ───────────────────────

    /// Start a new thinking task. Creates a temporary Meta node
    /// for the goal and runs initial gathering (recall + reason).
    /// Returns the initial ThinkingState with the first round of proposals.
    ///
    /// Only one thinking task can be active at a time.
    /// All operations route through the terminal bond.
    ///
    /// Simulation mode: use `think_simulate()` for dry-run that
    /// previews without creating real nodes/bonds.
    pub fn think_simulate(
        &mut self,
        goal: &str,
    ) -> Result<coordinator::ThinkingSummary, String> {
        let config = coordinator::ThinkConfig {
            dry_run: true,
            auto_approve_safe: true,
            ..Default::default()
        };
        self.think_start(goal, config)?;
        // Auto-run to completion
        loop {
            match self.think_step() {
                Ok(state) => {
                    if state.state == coordinator::GoalState::Complete || state.state == coordinator::GoalState::Failed {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        self.think_stop()
    }

    pub fn think_start(
        &mut self,
        goal: &str,
        config: coordinator::ThinkConfig,
    ) -> Result<coordinator::ThinkingState, String> {
        // Only one active thinking task at a time
        if self.active_goal.is_some() {
            return Err("A thinking task is already active. Use think_stop() first.".into());
        }

        // Create the root goal as a Meta node in the graph
        let pattern = self.pattern_engine.encode(goal);
        let goal_label = format!("GOAL: {}", goal);
        let node = KnowledgeNode::new(
            NodeType::Meta,
            goal_label,
            pattern,
            Confidence::Unverified,
            "thinking_layer",
        );
        let goal_seed = self.network.add_node(node);

        // Create the GoalCluster
        let cluster = coordinator::GoalCluster::new(goal, goal_seed, config);
        self.active_goal = Some(cluster);

        // Run the first gathering pass
        self.think_gather();

        // Generate initial proposals from the gathering results
        self.think_plan();

        Ok(self.active_goal.as_ref().unwrap().to_state())
    }

    /// Start a thinking session seeded from Lu's top curiosity wonder.
    ///
    /// Peeks at the highest-priority unresolved knowledge gap and uses
    /// its suggested_query (cleaned) as the thinking goal. The wonder
    /// stays in the queue and is resolved in `think_observe()` when the
    /// session completes with good coherence.
    pub fn think_from_wonder(&mut self, config: coordinator::ThinkConfig) -> Result<coordinator::ThinkingState, String> {
        // Peek at top wonder — does not consume it yet
        let query = {
            self.curiosity.wonder_queue.peek(1)
                .first()
                .map(|w| w.suggested_query.clone())
                .ok_or_else(|| "No wonders in the curiosity queue".to_string())?
        };
        let cleaned = Self::clean_wonder_query(&query);
        self.think_start(&cleaned, config)
    }

    /// Advance one iteration of the thinking loop.
    /// The state machine determines what happens next.
    pub fn think_step(&mut self) -> Result<coordinator::ThinkingState, String> {
        let state = self.active_goal.as_ref()
            .ok_or("No active thinking task")?
            .state.clone();

        match state {
            coordinator::GoalState::Gathering => {
                self.think_gather();
                self.think_plan();
            }
            coordinator::GoalState::Planning => {
                // Still planning — auto-approve safe proposals if configured
                let auto = self.active_goal.as_ref()
                    .map(|g| g.config.auto_approve_safe)
                    .unwrap_or(false);
                if auto {
                    self.think_auto_approve_safe();
                }
                // If there are still pending proposals, wait for approval
                let has_pending = self.active_goal.as_ref()
                    .map(|g| !g.pending_proposals.is_empty())
                    .unwrap_or(false);
                if !has_pending {
                    // All proposals handled, move to acting
                    if let Some(ref mut cluster) = self.active_goal {
                        cluster.state = coordinator::GoalState::Acting;
                    }
                    self.think_act();
                    self.think_observe();
                }
            }
            coordinator::GoalState::AwaitingApproval => {
                // Auto-approve safe proposals if configured
                let auto = self.active_goal.as_ref()
                    .map(|g| g.config.auto_approve_safe)
                    .unwrap_or(false);
                if auto {
                    self.think_auto_approve_safe();
                }
                // If all proposals handled, move to acting
                let has_pending = self.active_goal.as_ref()
                    .map(|g| !g.pending_proposals.is_empty())
                    .unwrap_or(false);
                if !has_pending {
                    if let Some(ref mut cluster) = self.active_goal {
                        cluster.state = coordinator::GoalState::Acting;
                    }
                    self.think_act();
                    self.think_observe();
                }
            }
            coordinator::GoalState::Acting => {
                self.think_act();
                self.think_observe();
            }
            coordinator::GoalState::Observing => {
                // Decide: loop or complete
                let should_continue = self.active_goal.as_ref()
                    .map(|g| g.should_continue())
                    .unwrap_or(false);
                if should_continue {
                    if let Some(ref mut cluster) = self.active_goal {
                        cluster.iteration += 1;
                        cluster.state = coordinator::GoalState::Gathering;
                    }
                    self.think_gather();
                    self.think_plan();
                } else {
                    if let Some(ref mut cluster) = self.active_goal {
                        cluster.state = coordinator::GoalState::Complete;
                    }
                }
            }
            coordinator::GoalState::Complete | coordinator::GoalState::Failed => {
                return Err("Thinking task already finished. Call think_stop() to get summary.".into());
            }
        }

        Ok(self.active_goal.as_ref().unwrap().to_state())
    }

    /// Approve a thinking proposal by ID.
    pub fn think_approve(&mut self, proposal_id: &str) -> Result<String, String> {
        let cluster = self.active_goal.as_mut()
            .ok_or("No active thinking task")?;

        let idx = cluster.pending_proposals.iter()
            .position(|p| p.id == proposal_id)
            .ok_or(format!("Proposal '{}' not found", proposal_id))?;

        let proposal = cluster.pending_proposals.remove(idx);
        let desc = proposal.description.clone();
        cluster.approved_proposals.push(proposal);

        // If no more pending proposals, transition
        if cluster.pending_proposals.is_empty() {
            cluster.state = coordinator::GoalState::Acting;
        }

        Ok(format!("Approved: {}", desc))
    }

    /// Reject a thinking proposal by ID.
    pub fn think_reject(&mut self, proposal_id: &str) -> Result<String, String> {
        let cluster = self.active_goal.as_mut()
            .ok_or("No active thinking task")?;

        let idx = cluster.pending_proposals.iter()
            .position(|p| p.id == proposal_id)
            .ok_or(format!("Proposal '{}' not found", proposal_id))?;

        let proposal = cluster.pending_proposals.remove(idx);
        let desc = proposal.description.clone();

        // If no more pending proposals, transition
        if cluster.pending_proposals.is_empty() {
            cluster.state = coordinator::GoalState::Acting;
        }

        Ok(format!("Rejected: {}", desc))
    }

    /// Get current thinking status (read-only).
    pub fn think_status(&self) -> Option<coordinator::ThinkingState> {
        self.active_goal.as_ref().map(|g| g.to_state())
    }

    /// Stop the thinking loop. Cleans up temporary nodes,
    /// returns a summary of what was accomplished.
    pub fn think_stop(&mut self) -> Result<coordinator::ThinkingSummary, String> {
        let cluster = self.active_goal.take()
            .ok_or("No active thinking task")?;

        // Build conclusion from last activation cache
        let conclusion = if let Some(last_recall) = cluster.activation_cache.last() {
            let top_labels: Vec<&str> = last_recall.activated_nodes.iter()
                .take(3)
                .map(|n| n.label.as_str())
                .collect();
            if top_labels.is_empty() {
                "No conclusion reached.".to_string()
            } else {
                format!("Thinking about '{}' activated: {}",
                    cluster.goal_text,
                    top_labels.join("; "))
            }
        } else {
            "Thinking stopped before gathering results.".to_string()
        };

        let summary = cluster.to_summary(conclusion);

        // Phase 5: Final self_reflect pass on promoted bonds/nodes.
        // After a thinking session, the promoted knowledge represents
        // validated connections. Run self_reflect on the final activation
        // state to consolidate plasticity — strengthen paths that survived
        // the full Gather→Plan→Act→Observe cycle.
        if !cluster.config.dry_run && !cluster.promoted_seeds.is_empty() {
            // Use the last activation cache to populate last_recall
            // so self_reflect sees the thinking session's final state
            if let Some(last_activation) = cluster.activation_cache.last() {
                self.last_recall = Some(last_activation.clone());
                let _final_reflect = self.self_reflect();
            }
        }

        // Clean up temporary nodes that were NOT promoted
        let cleanup_nodes = cluster.cleanup_seeds();
        let cleanup_bonds = cluster.cleanup_bond_seeds();

        for seed in &cleanup_nodes {
            self.network.deactivate_temporary(seed);
        }
        for seed in &cleanup_bonds {
            self.network.deactivate_temporary(seed);
        }

        Ok(summary)
    }

    // ─── Thinking Internal Methods ──────────────────────

    /// Gather phase: spread activation from goal + sub-goals.
    fn think_gather(&mut self) {
        let (goal_text, sub_goal_texts) = {
            let cluster = match self.active_goal.as_ref() {
                Some(c) => c,
                None => return,
            };
            let subs: Vec<String> = cluster.sub_goals.iter()
                .filter(|sg| sg.status == coordinator::SubGoalStatus::Pending
                           || sg.status == coordinator::SubGoalStatus::Active)
                .map(|sg| sg.text.clone())
                .collect();
            (cluster.goal_text.clone(), subs)
        };

        // Recall from the main goal
        let goal_result = self.recall(&goal_text, 3, 15);

        // Recall from each active sub-goal
        let mut sub_results: Vec<RecallResult> = Vec::new();
        for sub_text in &sub_goal_texts {
            let sub_result = self.recall(sub_text, 2, 10);
            sub_results.push(sub_result);
        }

        // Seed from recent conversation history (last 5 turns, shallow recall).
        // This prevents cold-start: concepts active in recent conversation are
        // pre-activated before goal-directed spreading begins.
        let recent_turns: Vec<String> = self.conversation_history
            .iter()
            .rev()
            .take(5)
            .cloned()
            .collect();
        let mut context_results: Vec<RecallResult> = Vec::new();
        for turn in &recent_turns {
            let ctx = self.recall(turn, 1, 5);
            context_results.push(ctx);
        }

        // Cache results and update sub-goal activation.
        // Context recalls are pushed first so goal_result stays last
        // (think_plan uses activation_cache.last() for proposal generation).
        if let Some(ref mut cluster) = self.active_goal {
            for ctx in context_results {
                cluster.activation_cache.push(ctx);
            }
            cluster.activation_cache.push(goal_result);

            for (i, sg) in cluster.sub_goals.iter_mut().enumerate() {
                if sg.status == coordinator::SubGoalStatus::Pending {
                    sg.status = coordinator::SubGoalStatus::Active;
                }
                if let Some(result) = sub_results.get(i) {
                    // Coherence = how many of the top activated nodes are relevant
                    let top_strength: f32 = result.activated_nodes.iter()
                        .take(5)
                        .map(|n| n.strength)
                        .sum();
                    sg.coherence_score = (top_strength / 100.0).min(1.0);
                    sg.activation_result = Some(result.clone());

                    if sg.coherence_score > 0.4 {
                        sg.status = coordinator::SubGoalStatus::Resolved;
                    }
                }
            }

            cluster.state = coordinator::GoalState::Planning;
        }
    }

    /// Plan phase: analyze gathering results, generate proposals.
    fn think_plan(&mut self) {
        // Phase 1: collect proposals and insight candidates (immutable borrow of cluster)
        let (mut proposals, insight_candidates) = {
            let cluster = match self.active_goal.as_ref() {
                Some(c) => c,
                None => return,
            };

            let last_recall = match cluster.activation_cache.last() {
                Some(r) => r,
                None => return,
            };

            let mut props: Vec<coordinator::ThinkingProposal> = Vec::new();
            let goal_hex = cluster.goal_seed.to_hex();

            // Strategy 1: If this is the first iteration with no sub-goals,
            // propose sub-goals based on the reasoning chains
            if cluster.sub_goals.is_empty() && !last_recall.activated_nodes.is_empty() {
                // Use the top activated nodes as sub-goal candidates
                let top_nodes: Vec<&ActivatedNode> = last_recall.activated_nodes.iter()
                    .filter(|n| n.label.split_whitespace().count() > 2) // skip single-word nodes
                    .take(3)
                    .collect();

                for node in &top_nodes {
                    let desc = format!("Explore: {}", node.label);
                    props.push(coordinator::ThinkingProposal::sub_goal(&desc, &goal_hex));
                }
            }

            // Strategy 2: Look for pairs of activated nodes that aren't bonded
            // (same logic as rumination, but goal-directed)
            if last_recall.activated_nodes.len() >= 2 {
                let nodes = &last_recall.activated_nodes;
                for i in 0..nodes.len().min(5) {
                    for j in (i + 1)..nodes.len().min(5) {
                        let a = &nodes[i];
                        let b = &nodes[j];
                        // Check if they're already bonded
                        if self.network.bond_weight(&a.seed_hex, &b.seed_hex).is_none() {
                            // Check scars without creating probe bonds
                            let a_seed = Seed::from_hex(&a.seed_hex);
                            let b_seed = Seed::from_hex(&b.seed_hex);
                            let mut scar_conflicts = Vec::new();
                            if let (Ok(sa), Ok(sb)) = (&a_seed, &b_seed) {
                                if let Some(warning) = self.network.has_scar_conflict(sa, sb) {
                                    scar_conflicts.push(warning);
                                }
                            }

                            let desc = format!(
                                "Hypothesis bond: '{}' <-> '{}'",
                                Self::truncate_label(&a.label, 40),
                                Self::truncate_label(&b.label, 40),
                            );
                            props.push(coordinator::ThinkingProposal::hypothesis_bond(
                                &desc,
                                &a.seed_hex,
                                &b.seed_hex,
                                scar_conflicts,
                            ));

                            // Limit to 2 hypothesis bond proposals per iteration
                            let bond_count = props.iter()
                                .filter(|p| matches!(p.proposal_type,
                                    coordinator::ThinkingProposalType::FormHypothesisBond))
                                .count();
                            if bond_count >= 2 { break; }
                        }
                    }
                }
            }

            // Collect sub-goal texts eligible for Strategy 3 reasoning
            // (only after at least one gather pass has run, i.e. iteration >= 1)
            let candidates: Vec<String> = if cluster.iteration >= 1 {
                cluster.sub_goals.iter()
                    .filter(|sg| {
                        sg.status == coordinator::SubGoalStatus::Resolved
                            || (sg.status == coordinator::SubGoalStatus::Active
                                && sg.coherence_score >= 0.4)
                    })
                    .map(|sg| sg.text.clone())
                    .collect()
            } else {
                Vec::new()
            };

            (props, candidates)
        };

        // Phase 2: Strategy 3 — reason on resolved sub-goals to derive LearnInsight proposals.
        // This requires &mut self so it must run after the immutable borrow above ends.
        // Limit to 1 LearnInsight proposal per iteration.
        for sg_text in &insight_candidates {
            let already_has_insight = proposals.iter().any(|p| {
                matches!(p.proposal_type, coordinator::ThinkingProposalType::LearnInsight)
            });
            if already_has_insight { break; }

            let reasoning = self.reason(&sg_text, 2);
            if !reasoning.conclusion.is_empty() {
                let max_conf = reasoning.chains.iter()
                    .map(|c| c.confidence)
                    .fold(0.0_f32, f32::max);
                if max_conf >= 0.5 {
                    proposals.push(coordinator::ThinkingProposal::learn_insight(
                        &reasoning.conclusion,
                    ));
                }
            }
        }

        // Phase 3: apply proposals to cluster
        if let Some(ref mut cluster) = self.active_goal {
            cluster.pending_proposals = proposals;
            if cluster.pending_proposals.is_empty() {
                // Nothing to propose — skip to observing
                cluster.state = coordinator::GoalState::Observing;
            } else {
                cluster.state = coordinator::GoalState::AwaitingApproval;
            }
        }
    }

    /// Auto-approve safe (non-destructive, no scar conflicts) proposals.
    fn think_auto_approve_safe(&mut self) {
        let cluster = match self.active_goal.as_mut() {
            Some(c) => c,
            None => return,
        };

        let (safe, unsafe_): (Vec<_>, Vec<_>) = cluster.pending_proposals
            .drain(..)
            .partition(|p| p.is_safe());

        cluster.approved_proposals.extend(safe);
        cluster.pending_proposals = unsafe_;

        if cluster.pending_proposals.is_empty() {
            cluster.state = coordinator::GoalState::Acting;
        }
    }

    /// Act phase: execute approved proposals.
    fn think_act(&mut self) {
        let dry_run = self.active_goal.as_ref()
            .map(|g| g.config.dry_run)
            .unwrap_or(true);

        // Take the approved proposals out to avoid borrow issues
        let approved = {
            let cluster = match self.active_goal.as_mut() {
                Some(c) => c,
                None => return,
            };
            std::mem::take(&mut cluster.approved_proposals)
        };

        for proposal in &approved {
            if dry_run {
                // In dry-run mode, just observe what WOULD happen
                if let Some(ref mut cluster) = self.active_goal {
                    cluster.observe(
                        &format!("[DRY RUN] {}", proposal.description),
                        "Would execute if not in dry-run mode",
                        0.0,
                    );
                }
                continue;
            }

            match proposal.proposal_type {
                coordinator::ThinkingProposalType::CreateSubGoal => {
                    let sub_text = proposal.description
                        .strip_prefix("Explore: ")
                        .unwrap_or(&proposal.description);
                    let pattern = self.pattern_engine.encode(sub_text);
                    let label = format!("SUBGOAL: {}", sub_text);
                    let mut node = KnowledgeNode::new(
                        NodeType::Procedure,
                        label,
                        pattern,
                        Confidence::Unverified,
                        "thinking_layer",
                    );
                    // Link to parent goal via parent_seeds
                    if let Some(ref cluster) = self.active_goal {
                        node.parent_seeds.push(cluster.goal_seed.clone());
                    }
                    let seed = self.network.add_node(node);
                    if let Some(ref mut cluster) = self.active_goal {
                        cluster.add_sub_goal(seed, sub_text.to_string());
                        cluster.observe(
                            &format!("Created sub-goal: {}", sub_text),
                            "Procedure node added to graph",
                            0.05,
                        );
                    }
                }
                coordinator::ThinkingProposalType::FormHypothesisBond => {
                    if proposal.target_seeds.len() >= 2 {
                        let src = Seed::from_hex(&proposal.target_seeds[0]);
                        let tgt = Seed::from_hex(&proposal.target_seeds[1]);
                        if let (Ok(s), Ok(t)) = (src, tgt) {
                            let bond_label = format!("hypothesis: thinking_layer");
                            match self.network.add_bond(
                                &s, &t, bond_label, 0.2, "thinking_layer"
                            ) {
                                Ok(bond_seed) => {
                                    if let Some(ref mut cluster) = self.active_goal {
                                        cluster.add_hypothesis_bond(bond_seed);
                                        cluster.observe(
                                            &proposal.description,
                                            "Hypothesis bond formed (weight 0.2)",
                                            0.05,
                                        );
                                    }
                                }
                                Err(e) => {
                                    if let Some(ref mut cluster) = self.active_goal {
                                        cluster.observe(
                                            &proposal.description,
                                            &format!("Bond blocked: {}", e),
                                            -0.05,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                coordinator::ThinkingProposalType::LearnInsight => {
                    let seed = self.learn_fast(
                        &proposal.description,
                        Confidence::Unverified,
                        "thinking_layer",
                    );
                    if let Some(ref mut cluster) = self.active_goal {
                        // promote() marks the seed as permanent so it survives cleanup
                        cluster.promote(&seed);
                        cluster.created_node_seeds.push(seed);
                        cluster.observe(
                            &format!("Learned insight: {}", proposal.description),
                            "Knowledge node created",
                            0.1,
                        );
                    }
                }
                coordinator::ThinkingProposalType::PromoteBond => {
                    if let Some(hex) = proposal.target_seeds.first() {
                        if let Ok(seed) = Seed::from_hex(hex) {
                            if let Some(ref mut cluster) = self.active_goal {
                                cluster.promote(&seed);
                                cluster.observe(
                                    &proposal.description,
                                    "Bond promoted to permanent",
                                    0.1,
                                );
                            }
                        }
                    }
                }
                coordinator::ThinkingProposalType::RevokeContradiction => {
                    if let Some(hex) = proposal.target_seeds.first() {
                        if let Ok(seed) = Seed::from_hex(hex) {
                            self.network.revoke(&seed, "thinking_layer: contradiction");
                            if let Some(ref mut cluster) = self.active_goal {
                                cluster.observe(
                                    &proposal.description,
                                    "Revoked with scar",
                                    -0.1,
                                );
                            }
                        }
                    }
                }
            }
        }

        // Move to observing
        if let Some(ref mut cluster) = self.active_goal {
            cluster.state = coordinator::GoalState::Observing;
        }
    }

    /// Observe phase: measure coherence, decide if loop continues.
    fn think_observe(&mut self) {
        // Extract what we need from the cluster first (avoids borrow conflict with self_reflect)
        let (dry_run, iteration) = match self.active_goal.as_ref() {
            Some(c) => (c.config.dry_run, c.iteration),
            None => return,
        };

        // Phase 5: Trigger plasticity via self_reflect on each observation.
        // self_reflect works from self.last_recall (set during think_gather's recall calls).
        // Strengthen co-activated paths, create novel associations.
        // Skipped in dry_run mode — no permanent modifications.
        let reflect_report = if !dry_run {
            let report = self.self_reflect();
            Some(report)
        } else {
            None
        };

        if let Some(ref mut cluster) = self.active_goal {
            let old_coherence = cluster.coherence;
            cluster.coherence = cluster.compute_coherence();
            let delta = cluster.coherence - old_coherence;

            cluster.observe(
                &format!("Iteration {} observation", iteration),
                &format!(
                    "Coherence: {:.2} (delta {}{:.2}). Sub-goals: {}/{} resolved.",
                    cluster.coherence,
                    if delta >= 0.0 { "+" } else { "" },
                    delta,
                    cluster.sub_goals.iter()
                        .filter(|sg| sg.status == coordinator::SubGoalStatus::Resolved)
                        .count(),
                    cluster.sub_goals.len(),
                ),
                delta,
            );

            // Record plasticity results from self_reflect
            if let Some(ref report) = reflect_report {
                if report.bonds_strengthened > 0 || report.bonds_created > 0 {
                    cluster.observe(
                        &format!("Plasticity (iter {})", iteration),
                        &format!(
                            "self_reflect: {} bonds strengthened, {} novel associations created",
                            report.bonds_strengthened, report.bonds_created
                        ),
                        // Plasticity improving coherence slightly
                        (report.bonds_strengthened as f32 * 0.01)
                            + (report.bonds_created as f32 * 0.02),
                    );
                }
            }

            // Decide next state
            if cluster.coherence >= cluster.config.coherence_threshold {
                cluster.state = coordinator::GoalState::Complete;
            } else if cluster.iteration >= cluster.config.max_iterations {
                cluster.state = coordinator::GoalState::Complete;
            } else {
                // Stay in Observing — think_step() will advance to Gathering
                cluster.state = coordinator::GoalState::Observing;
            }

            // Mark unresolved sub-goals as Failed when the session ends.
            if cluster.state == coordinator::GoalState::Complete {
                for sg in &mut cluster.sub_goals {
                    if sg.status != coordinator::SubGoalStatus::Resolved {
                        sg.status = coordinator::SubGoalStatus::Failed;
                    }
                }
            }
        }

        // Scar failed sub-goals so Lu remembers what didn't work.
        // Scars warn against re-attempting the same decomposition path.
        if !dry_run {
            let failed_sub_goals: Vec<(Seed, String, Vec<f32>)> = self.active_goal.as_ref()
                .map(|cluster| {
                    cluster.sub_goals.iter()
                        .filter(|sg| sg.status == coordinator::SubGoalStatus::Failed)
                        .map(|sg| {
                            let pattern = self.pattern_engine.encode(&sg.text);
                            (sg.seed.clone(), sg.text.clone(), pattern)
                        })
                        .collect()
                })
                .unwrap_or_default();

            for (seed, text, pattern) in failed_sub_goals {
                let goal_text = self.active_goal.as_ref()
                    .map(|c| c.goal_text.as_str())
                    .unwrap_or("unknown");
                self.network.add_scar(Scar {
                    revoked_seed: seed,
                    label: format!("failed sub-goal: {}", text),
                    reason: format!(
                        "Sub-goal did not resolve during thinking about '{}'",
                        goal_text,
                    ),
                    revoked_at: chrono::Utc::now(),
                    pattern_fingerprint: pattern,
                });
            }
        }

        // If the session just completed, resolve any matching curiosity wonder.
        // This closes the loop for think_from_wonder() sessions: the wonder that
        // triggered thinking is marked resolved once thinking achieves good coherence.
        let just_completed = self.active_goal.as_ref()
            .map_or(false, |c| c.state == coordinator::GoalState::Complete);
        if just_completed {
            let goal_text = self.active_goal.as_ref()
                .map(|c| c.goal_text.clone())
                .unwrap_or_default();
            let cleaned_goal = Self::clean_wonder_query(&goal_text);
            // Collect matching wonder ids/domains without holding a borrow
            let matching: Option<(String, String)> = {
                let hits: Vec<(String, String)> = self.curiosity.wonder_queue.peek(10)
                    .iter()
                    .filter(|w| Self::clean_wonder_query(&w.suggested_query) == cleaned_goal)
                    .map(|w| (w.id.clone(), w.domain.clone()))
                    .collect();
                hits.into_iter().next()
            };
            if let Some((id, domain)) = matching {
                self.curiosity.resolve_gap(&id, &domain);
            }
        }
    }

    // ─── Left Brain: Session Memory API ─────────────────────

    /// Add a string to the session context window (rolling recent queries).
    pub fn session_add_context(&mut self, text: &str) {
        self.session_memory.add_context(text);
    }

    /// Add a temporary session fact. Returns the fact's id.
    ///
    /// The content is encoded through the pattern engine so it lives in
    /// the same 256-dim space as Right Brain nodes — enabling semantic
    /// similarity retrieval during recall.
    ///
    /// Session facts carry a TTL and will decay unless promoted.
    /// They are NEVER auto-promoted; use `session_propose_promote` to
    /// request human approval.
    pub fn session_add_fact(
        &mut self,
        content: &str,
        source: &str,
        confidence: f32,
    ) -> String {
        let pattern = self.pattern_engine.encode(content);
        self.session_memory.add_session_fact(content, source, confidence, pattern)
    }

    /// Propose promoting a session fact to permanent Right Brain knowledge.
    ///
    /// This creates a standard `LearningProposal` — identical to what
    /// `Brain::propose()` produces for external content. The proposal
    /// must then be approved by a human via `brain.approve_proposal(id)`.
    ///
    /// On success, marks the session fact as `promoted = true` so it is
    /// no longer surfaced in recall context (avoids double-counting).
    ///
    /// Returns None if no fact with that id exists or it has already
    /// been promoted or expired.
    pub fn session_propose_promote(&mut self, fact_id: &str) -> Option<LearningProposal> {
        // Clone the fact data we need before borrowing self mutably for propose().
        let (content, source) = {
            let fact = self.session_memory.get_fact_for_promotion(fact_id)?;
            (fact.content.clone(), fact.source.clone())
        };

        // Use the standard propose() pipeline — same scar checks, similarity
        // analysis, and recommendation logic as any external ingestion.
        let proposal = self.propose(&content, &format!("session_fact:{source}"));

        // Mark as promoted so the Left Brain stops surfacing it.
        // The Right Brain node only exists after human approval.
        self.session_memory.mark_promoted(fact_id);

        Some(proposal)
    }

    /// Return the current session memory status snapshot.
    pub fn session_status(&self) -> session_memory::SessionStatus {
        self.session_memory.status()
    }

    /// Clear all session memory (context, facts, candidates).
    pub fn session_clear(&mut self) {
        self.session_memory.clear();
    }

    /// Store topic seeds from a recall result (for tool_recall which bypasses ask()).
    pub fn session_memory_set_topic_seeds(&mut self, seeds: Vec<session_memory::TopicSeed>) {
        self.session_memory.set_topic_seeds(seeds);
    }

    /// Get the active bond count for a node by hex key.
    pub fn bond_count_for(&self, node_hex: &str) -> Option<usize> {
        self.network.bond_count_for(node_hex)
    }

    // ━━━ Cross-Modal Search (public API) ━━━━━━━━━━━━━━━━━━━━━━━━━

    /// Search for Media nodes (images, audio, video) that relate to a text query.
    /// Returns (seed_hex, label, media_type, similarity) tuples.
    pub fn cross_modal_search(&mut self, query: &str, limit: usize) -> Vec<(String, String, String, f32)> {
        let pattern = self.pattern_engine.encode(query);
        let matches = self.network_media_similarities(&pattern, 0.70);
        matches.iter()
            .take(limit)
            .filter_map(|(seed, sim)| {
                if let Some(node) = self.network.get_node(seed) {
                    let media_type = node.metadata.get("media_type")
                        .cloned().unwrap_or_else(|| "unknown".to_string());
                    Some((seed.short(), node.label.clone(), media_type, *sim))
                } else {
                    None
                }
            })
            .collect()
    }

    // ━━━ Autonomy Engine (public API) ━━━━━━━━━━━━━━━━━━━━━━━━━━━

    /// Get the current autonomy status: tier assignments, queue, history.
    pub fn autonomy_status(&self) -> autonomy::AutonomyStatus {
        self.autonomy.status()
    }

    /// Elevate (or lower) an action type to a different execution tier.
    /// Returns the previous tier as a string.
    pub fn autonomy_set_tier(
        &mut self,
        action_str: &str,
        tier_str: &str,
    ) -> Result<String, String> {
        let action = match action_str.to_lowercase().as_str() {
            "recall" => autonomy::ActionType::Recall,
            "createsubgoal" | "create_sub_goal" => autonomy::ActionType::CreateSubGoal,
            "formhypothesis" | "form_hypothesis" => autonomy::ActionType::FormHypothesis,
            "promotebond" | "promote_bond" => autonomy::ActionType::PromoteBond,
            "learninsight" | "learn_insight" => autonomy::ActionType::LearnInsight,
            "revoke" => autonomy::ActionType::Revoke,
            "strengthenbond" | "strengthen_bond" => autonomy::ActionType::StrengthenBond,
            "weakenbond" | "weaken_bond" => autonomy::ActionType::WeakenBond,
            "createnode" | "create_node" => autonomy::ActionType::CreateNode,
            "deletenode" | "delete_node" => autonomy::ActionType::DeleteNode,
            _ => return Err(format!("Unknown action type: '{}'. Valid: Recall, CreateSubGoal, FormHypothesis, PromoteBond, LearnInsight, Revoke, StrengthenBond, WeakenBond, CreateNode, DeleteNode", action_str)),
        };

        let tier = match tier_str.to_lowercase().as_str() {
            "observe" => autonomy::ExecutionTier::Observe,
            "suggest" => autonomy::ExecutionTier::Suggest,
            "act" => autonomy::ExecutionTier::Act,
            "create" => autonomy::ExecutionTier::Create,
            _ => return Err(format!("Unknown tier: '{}'. Valid: Observe, Suggest, Act, Create", tier_str)),
        };

        let prev = self.autonomy.policy.set_tier(action, tier);
        Ok(format!("{}", prev))
    }

    /// Queue a thinking proposal through the autonomy pipeline.
    /// Returns the queued action ID.
    pub fn autonomy_enqueue(
        &mut self,
        proposal: &coordinator::ThinkingProposal,
        max_bond_count: usize,
    ) -> String {
        let has_scars = !proposal.scar_conflicts.is_empty();
        self.autonomy.pipeline.enqueue(proposal, has_scars, max_bond_count)
    }

    /// Try to execute the next queued action (if tier allows).
    /// Returns the result. The caller still needs to perform the
    /// actual graph mutation for Success results.
    pub fn autonomy_execute_next(&mut self) -> Option<autonomy::ExecutionResult> {
        let (result, action) = self.autonomy.pipeline.try_execute_next(&self.autonomy.policy)?;

        // If the pipeline approved the action, perform the actual graph mutation
        if let Some(action) = action {
            match self.execute_autonomy_action(&action) {
                Ok(msg) => {
                    // Success — already recorded in pipeline history
                    return Some(autonomy::ExecutionResult::Success {
                        action_id: action.id,
                        description: msg,
                    });
                }
                Err(err) => {
                    return Some(autonomy::ExecutionResult::Failed {
                        action_id: action.id,
                        reason: err,
                    });
                }
            }
        }

        Some(result)
    }

    /// Execute a queued action by performing the actual graph mutation.
    fn execute_autonomy_action(&mut self, action: &autonomy::QueuedAction) -> Result<String, String> {
        match &action.action_type {
            autonomy::ActionType::Recall => {
                // Read-only, nothing to mutate
                Ok("Recall completed.".to_string())
            }
            autonomy::ActionType::CreateSubGoal => {
                let seed = self.learn_fast(
                    &action.description,
                    Confidence::Unverified,
                    "autonomy_executor",
                );
                Ok(format!("Created sub-goal node: {}", seed.short()))
            }
            autonomy::ActionType::FormHypothesis => {
                if action.target_seeds.len() >= 2 {
                    if let (Ok(a), Ok(b)) = (
                        Seed::from_hex(&action.target_seeds[0]),
                        Seed::from_hex(&action.target_seeds[1]),
                    ) {
                        let _ = self.associate(&a, &b, &format!("hypothesis: {}", action.description), 0.3, "autonomy_executor");
                        Ok(format!("Hypothesis bond created between {} and {}", action.target_seeds[0].get(..8).unwrap_or("?"), action.target_seeds[1].get(..8).unwrap_or("?")))
                    } else {
                        Err("Invalid target seeds for hypothesis bond".to_string())
                    }
                } else {
                    Err("FormHypothesis requires at least 2 target seeds".to_string())
                }
            }
            autonomy::ActionType::PromoteBond => {
                if action.target_seeds.len() >= 2 {
                    let _ = self.strengthen_bond(&action.target_seeds[0], &action.target_seeds[1], 1.0);
                    Ok(format!("Bond promoted (strengthened by 1.0)"))
                } else {
                    Err("PromoteBond requires at least 2 target seeds".to_string())
                }
            }
            autonomy::ActionType::LearnInsight => {
                let seed = self.learn_fast(
                    &action.description,
                    Confidence::Unverified,
                    "autonomy_executor",
                );
                Ok(format!("Learned insight: {}", seed.short()))
            }
            autonomy::ActionType::Revoke => {
                if let Some(target) = action.target_seeds.first() {
                    if let Ok(seed) = Seed::from_hex(target) {
                        let report = self.revoke(&seed, &action.description);
                        Ok(format!("Revoked seed {}. Children to review: {}", report.revoked_seed.get(..16).unwrap_or("?"), report.children_to_review.len()))
                    } else {
                        Err("Invalid seed hex for revocation".to_string())
                    }
                } else {
                    Err("Revoke requires a target seed".to_string())
                }
            }
            autonomy::ActionType::StrengthenBond => {
                if action.target_seeds.len() >= 2 {
                    let _ = self.strengthen_bond(&action.target_seeds[0], &action.target_seeds[1], 0.5);
                    Ok("Bond strengthened by 0.5".to_string())
                } else {
                    Err("StrengthenBond requires 2 target seeds".to_string())
                }
            }
            autonomy::ActionType::WeakenBond => {
                if action.target_seeds.len() >= 2 {
                    let _ = self.weaken_bond(&action.target_seeds[0], &action.target_seeds[1], 0.5);
                    Ok("Bond weakened by 0.5".to_string())
                } else {
                    Err("WeakenBond requires 2 target seeds".to_string())
                }
            }
            autonomy::ActionType::CreateNode => {
                let seed = self.learn_fast(
                    &action.description,
                    Confidence::Unverified,
                    "autonomy_executor",
                );
                Ok(format!("Node created: {}", seed.short()))
            }
            autonomy::ActionType::DeleteNode => {
                // Node deletion is always Critical risk — should never reach here
                // via auto-execution, but handle it safely
                Err("Node deletion requires manual revocation — too destructive for auto-execution".to_string())
            }
        }
    }

    /// Human-approve a specific queued action by ID.
    pub fn autonomy_approve_action(&mut self, action_id: &str) -> bool {
        self.autonomy.pipeline.approve(action_id)
    }

    /// Get the execution policy summary.
    pub fn autonomy_policy(&self) -> Vec<(String, String)> {
        self.autonomy.policy.to_summary()
    }

    /// Can this proposal auto-execute under current autonomy settings?
    /// Checks BOTH the proposal's safety AND the tier level.
    pub fn autonomy_can_auto_approve(
        &self,
        proposal: &coordinator::ThinkingProposal,
    ) -> bool {
        self.autonomy.can_auto_approve(proposal)
    }

    // ━━━ Curiosity Engine Public API ━━━━━━━━━━━━━━━━━━━━━━━━━━━

    /// Get curiosity engine status.
    pub fn curiosity_status(&self) -> curiosity::CuriosityStatus {
        self.curiosity.status()
    }

    /// Enable or disable the curiosity engine.
    pub fn curiosity_set_enabled(&mut self, enabled: bool) {
        self.curiosity.set_enabled(enabled);
    }

    /// Manually trigger curiosity analysis on a query.
    /// Runs a recall and feeds the result to the curiosity engine.
    ///
    /// Note: `recall()` internally calls `after_recall` and may set
    /// `pending_auto_research = true`.  We clear that flag here so the
    /// MCP worker's post-call drain does NOT silently consume the wonder
    /// before the caller can invoke `brain_auto_resolve_wonder` manually.
    pub fn curiosity_trigger(&mut self, query: &str, domain: &str) {
        let result = self.recall(query, 3, 10);
        // `recall()` already ran `after_recall` with an auto-detected domain.
        // Run it again with the caller-supplied domain so the wonder is tagged
        // correctly. `push_signals` deduplicates by suggested_query, so the
        // only net effect is the domain label being set to the right value.
        self.curiosity.after_recall(&result, query, domain);
        // A manual trigger is meant for user inspection — suppress the
        // background auto-drain so the wonder stays in the queue for an
        // explicit `brain_auto_resolve_wonder` call.
        self.pending_auto_research = false;
    }

    /// Get the next wonder (highest-priority gap).
    pub fn curiosity_next_wonder(&mut self) -> Option<curiosity::CuriositySignal> {
        self.curiosity.next_wonder()
    }

    /// Peek at top N wonders without consuming them.
    pub fn curiosity_top_wonders(&self, n: usize) -> Vec<&curiosity::CuriositySignal> {
        self.curiosity.top_wonders(n)
    }

    /// Set interest weight for a domain.
    pub fn curiosity_set_interest(&mut self, domain: &str, weight: f32) {
        self.curiosity.set_interest(domain, weight);
    }

    /// Mark a gap as resolved.
    pub fn curiosity_resolve(&mut self, signal_id: &str, domain: &str) {
        self.curiosity.resolve_gap(signal_id, domain);
    }

    /// Run garbage collection on the curiosity engine.
    pub fn curiosity_gc(&mut self) {
        self.curiosity.gc();
    }

    /// Enable or disable automatic wonder resolution.
    /// Updates both the in-memory CuriosityEngine and the persisted mirror
    /// field so the setting survives MCP server restarts.
    pub fn curiosity_set_auto_resolve(&mut self, enabled: bool) {
        self.curiosity.set_auto_resolve(enabled);
        self.curiosity_auto_resolve = enabled;
    }

    // ━━━ Curiosity Auto-Resolve Loop ━━━━━━━━━━━━━━━━━━━━━━━━━━━

    /// Clean a wonder's suggested_query into a tight web search query.
    ///
    /// Wonder queries can have boilerplate prefixes ("What is X?",
    /// "Verify: A, B, C", "Research: domain") that are bad search
    /// queries. This strips the noise and returns the meaningful terms.
    fn clean_wonder_query(raw: &str) -> String {
        let s = raw.trim();

        // Strip question-word prefixes in a loop — handles "What is What are ...",
        // "What are What is ...", etc.
        let mut s = s;
        loop {
            let lower = s.to_lowercase();
            let question_prefixes = [
                "what is ", "what are ", "what was ", "what were ",
                "what does ", "what do ", "how does ", "how do ",
                "why does ", "why do ", "who is ", "where is ",
                "when did ", "which is ",
            ];
            let mut stripped = false;
            for prefix in &question_prefixes {
                if lower.starts_with(prefix) {
                    s = &s[prefix.len()..];
                    stripped = true;
                    break;
                }
            }
            if !stripped { break; }
        }

        // Strip "Verify: " — extract keywords from the first clause
        if s.to_lowercase().starts_with("verify: ") {
            let after = &s["verify: ".len()..];
            let first_clause = after.split([',', '.']).next().unwrap_or(after).trim();
            return Self::wonder_keywords(first_clause, 5);
        }

        // Strip "Research: "
        let s = if s.to_lowercase().starts_with("research: ") {
            &s["research: ".len()..]
        } else {
            s
        };

        // Strip trailing punctuation aggressively (handles '??', '!', '.')
        let s = s.trim_end_matches(|c| matches!(c, '?' | '!' | '.')).trim();

        // Truncate at first newline — wonder queries can be multi-paragraph explanations
        let s = match s.find('\n') {
            Some(pos) => s[..pos].trim(),
            None => s,
        };

        // If the remaining text looks like a full sentence, extract keywords
        let lower = s.to_lowercase();
        let is_sentence = lower.contains(" are ") || lower.contains(" is ")
            || lower.contains(" were ") || lower.contains(" was ")
            || lower.contains(" have ") || lower.contains(" responsible for ")
            || lower.contains(" consist of ");
        if is_sentence {
            return Self::wonder_keywords(s, 5);
        }

        // Cap at 100 chars, breaking at a word boundary
        let s = if s.len() > 100 {
            s[..100].rfind(' ').map(|i| s[..i].trim()).unwrap_or(&s[..100])
        } else {
            s
        };

        s.to_string()
    }

    /// Extract up to `max_words` significant keywords from a sentence, lowercased.
    fn wonder_keywords(text: &str, max_words: usize) -> String {
        const STOP: &[&str] = &[
            "a", "an", "the", "is", "are", "was", "were", "be", "been", "being",
            "have", "has", "had", "do", "does", "did", "will", "would", "could",
            "should", "may", "might", "must", "shall", "can", "for", "in", "on",
            "at", "to", "of", "as", "by", "with", "about", "from", "that", "this",
            "these", "those", "or", "and", "but", "not", "it", "its", "their",
            "there", "they", "we", "i", "you", "he", "she", "which", "who",
            "roughly", "approximately", "currently", "typically", "generally",
            "often", "usually", "also", "only", "both", "between", "among",
            "within", "during", "while", "after", "before", "since", "until",
            "into", "through", "across", "over", "under", "up", "down", "out",
            "off", "then", "now", "here", "if", "because", "although", "though",
            "whereas", "therefore", "however", "thus", "hence", "so", "than",
            "too", "very", "just", "responsible", "used", "using", "makes",
            "made", "all", "each", "some", "any", "most", "more", "other",
            "such", "no",
        ];
        let words: Vec<String> = text
            .split_whitespace()
            .filter_map(|w| {
                let clean: String = w.chars()
                    .filter(|c| c.is_alphanumeric() || *c == '-')
                    .collect();
                let lower = clean.to_lowercase();
                if lower.len() >= 3 && !STOP.contains(&lower.as_str()) {
                    Some(lower)
                } else {
                    None
                }
            })
            .take(max_words)
            .collect();
        words.join(" ")
    }

    /// Auto-resolve the highest-priority wonder via web research.
    ///
    /// 1. Pops the top wonder from the queue
    /// 2. Cleans its suggested_query
    /// 3. Runs web research with a tight budget (max 2 pages, 30s)
    /// 4. Ingests facts via the normal propose pipeline
    /// 5. Auto-approves up to 20 proposals (skipping scar conflicts)
    /// 6. Marks the wonder as resolved
    /// 7. Returns a summary
    pub fn auto_resolve_wonder(&mut self) -> Option<AutoResolveResult> {
        // Take the top unresolved wonder
        let wonder = self.curiosity.next_wonder()?;

        let raw_query = wonder.suggested_query.clone();
        let cleaned = Self::clean_wonder_query(&raw_query);
        if cleaned.is_empty() {
            // Nothing meaningful to search — mark resolved anyway
            self.curiosity.resolve_gap(&wonder.id, &wonder.domain);
            return Some(AutoResolveResult {
                wonder_id: wonder.id,
                query_used: raw_query,
                pages_visited: 0,
                facts_proposed: 0,
                proposals_approved: 0,
                gap_filled: false,
                summary: "Wonder query was empty after cleaning — skipped.".to_string(),
            });
        }

        eprintln!("[auto_resolve] Researching wonder '{}' → query='{}'", raw_query, cleaned);

        // Run web research with a tight budget so we don't block too long
        let safety = web_safety::WebSafety::new();
        let fetcher = web_fetch::WebFetcher::new(safety);
        let config = web_research::ResearchConfig {
            max_pages: 2,
            max_depth: 1,
            max_time_secs: 30,
            max_results_per_search: 5,
            min_relevance: 0.3,
        };
        let researcher = web_research::WebResearcher::with_config(fetcher, config);
        let result = researcher.research(&cleaned);

        let pages_visited = result.pages_visited.len();
        let facts_count = result.knowledge_extracted.len();
        let gap_filled = result.gap_filled;

        eprintln!(
            "[auto_resolve] Research done: {} pages, {} facts, gap_filled={}",
            pages_visited, facts_count, gap_filled
        );

        // Ingest facts → propose them
        let proposed = self.web_ingest_research(&result);

        // Auto-approve up to 20 pending text proposals (no scar conflicts).
        // Safety cap prevents flooding the graph from a single wonder.
        const AUTO_APPROVE_CAP: usize = 20;
        let ids_to_approve: Vec<String> = self.pending_proposals
            .iter()
            .filter(|(_, p)| p.scar_conflicts.is_empty())
            .take(AUTO_APPROVE_CAP)
            .map(|(id, _)| id.clone())
            .collect();
        let approved_count = ids_to_approve.len();
        self.approve_all_ready(&ids_to_approve);

        // Mark the wonder as resolved
        if gap_filled || approved_count > 0 {
            self.curiosity.resolve_gap(&wonder.id, &wonder.domain);
        }

        // Invalidate word index so new nodes appear in next recall
        if approved_count > 0 {
            self.word_index_valid = false;
        }

        let summary = format!(
            "Wonder '{}': visited {} pages, proposed {} facts, approved {} nodes. Gap filled: {}.",
            cleaned, pages_visited, proposed, approved_count, gap_filled
        );
        eprintln!("[auto_resolve] {}", summary);

        Some(AutoResolveResult {
            wonder_id: wonder.id,
            query_used: cleaned,
            pages_visited,
            facts_proposed: proposed,
            proposals_approved: approved_count,
            gap_filled,
            summary,
        })
    }

    /// Drain pending auto-research if one was queued by recall().
    /// Called by the MCP worker loop AFTER sending the response,
    /// so it never blocks the tool call that triggered the recall.
    /// Returns Some(result) if research ran, None if nothing was pending.
    pub fn drain_auto_resolve(&mut self) -> Option<AutoResolveResult> {
        if !self.pending_auto_research {
            return None;
        }
        self.pending_auto_research = false;
        self.auto_resolve_wonder()
    }

    // ━━━ Variant Generation Public API ━━━━━━━━━━━━━━━━━━━━━━━━

    /// Generate competing variants for a query.
    /// Spawns N variants, runs activation for each, scores them.
    pub fn variants_generate(&mut self, query: &str, count: usize) -> arena::ArenaStatus {
        let base_pattern = self.pattern_engine.encode(query);

        // Gather hub hexes (nodes with most bonds) for perturbation
        let bond_counts = self.network.bond_counts();
        let mut hubs: Vec<(String, usize)> = bond_counts.into_iter().collect();
        hubs.sort_by(|a, b| b.1.cmp(&a.1));
        let hub_hexes: Vec<String> = hubs.iter().take(5).map(|(h, _)| h.clone()).collect();

        // Gather scar-adjacent seed hexes for contrarian variants
        let scar_hexes: Vec<String> = self.network.scars()
            .iter()
            .take(3)
            .map(|s| s.revoked_seed.to_hex())
            .collect();

        // Spawn variants
        let clusters = variants::VariantSpawner::spawn(
            &base_pattern, &hub_hexes, &scar_hexes, count,
        );
        let mut arena_inst = arena::VariantArena::new(query, clusters);

        // Run activation for each variant
        for variant in &mut arena_inst.variants {
            variant.status = variants::VariantStatus::Activating;
            // Build initial activation set from perturbed pattern
            let matches = self.network.spread_activation(
                &self.pattern_to_seeds(&variant.perturbed_pattern, 5),
                2,
                20,
            );
            variant.activated_nodes = matches.activated_nodes
                .iter()
                .map(|n| (n.seed_hex.clone(), n.strength))
                .collect();
        }

        // Gather high-confidence node hexes for scoring
        let high_conf: std::collections::HashSet<String> = self.network
            .active_nodes()
            .filter(|(_, node)| {
                matches!(node.confidence, graph::Confidence::Terminal | graph::Confidence::Verified)
            })
            .map(|(hex, _)| hex.to_string())
            .collect();

        let total_nodes = self.network.stats().active_nodes;
        arena_inst.score_all(&high_conf, total_nodes);

        let status = arena_inst.status();
        self.active_arena = Some(arena_inst);
        status
    }

    /// Helper: convert a pattern to initial activation seeds.
    fn pattern_to_seeds(&self, pattern: &[f32], top_k: usize) -> Vec<(df_core::Seed, f32)> {
        let mut matches: Vec<(df_core::Seed, f32)> = Vec::new();
        for (hex, node) in self.network.active_nodes() {
            let sim = cosine_similarity(pattern, &node.pattern);
            if sim > 0.3 {
                if let Ok(seed) = df_core::Seed::from_hex(hex) {
                    matches.push((seed, sim * 100.0));
                }
            }
        }
        matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        matches.truncate(top_k);
        matches
    }

    /// Get current arena status.
    pub fn variants_status(&self) -> Option<arena::ArenaStatus> {
        self.active_arena.as_ref().map(|a| a.status())
    }

    /// Promote top N variants from the arena.
    pub fn variants_promote(&mut self, n: usize) -> Option<variant_promotion::PromotionReport> {
        if let Some(ref mut arena_inst) = self.active_arena {
            Some(variant_promotion::run_promotion(arena_inst, n))
        } else {
            None
        }
    }

    /// Clear the active arena.
    pub fn variants_clear(&mut self) {
        self.active_arena = None;
    }

    // ━━━ Knowledge Health Public API ━━━━━━━━━━━━━━━━━━━━━━━━━━

    /// Compute knowledge health metrics.
    pub fn health_report(&self) -> knowledge_health::KnowledgeHealth {
        let stats = self.network.stats();
        let bond_counts = self.network.bond_counts();
        let node_labels = self.network.node_label_map();
        let bond_weights: Vec<f32> = self.network.viz_bonds()
            .iter()
            .map(|(_, _, w)| *w)
            .collect();
        let scar_count = self.network.scars().len();
        knowledge_health::compute_health(
            &stats, &bond_counts, &bond_weights, &node_labels, scar_count,
        )
    }

    /// Clean up glossary/appendix noise nodes from the knowledge graph.
    /// Identifies and revokes nodes that are index-like entries:
    /// - Labels that start with "glossary:", "appendix:", "index:"
    /// - Single-word or two-word labels that have very high bond counts (100+)
    ///   and no meaningful content (just connective hub noise)
    /// - Empty or whitespace-only labels
    /// Returns a report of what was cleaned.
    pub fn cleanup_glossary_nodes(&mut self) -> GlossaryCleanupReport {
        let bond_counts = self.network.bond_counts();
        let mut candidates: Vec<(String, String, String)> = Vec::new(); // (hex, label, reason)

        // Collect candidates (can't borrow network mutably while iterating)
        for (hex, node) in self.network.active_nodes() {
            let label = &node.label;
            let label_lower = label.to_lowercase().trim().to_string();

            // Empty or near-empty labels
            if label.trim().is_empty() || label.trim().len() < 3 {
                candidates.push((hex.to_string(), label.clone(),
                    "empty or near-empty label".to_string()));
                continue;
            }

            // Glossary/appendix/index prefixes
            if label_lower.starts_with("glossary:")
                || label_lower.starts_with("appendix:")
                || label_lower.starts_with("index:")
                || label_lower.starts_with("table of contents")
                || label_lower.starts_with("see also:")
            {
                candidates.push((hex.to_string(), label.clone(),
                    "glossary/appendix/index prefix".to_string()));
                continue;
            }

            // Short labels with extremely high bond counts = index noise
            let word_count = label.split_whitespace().count();
            let bc = bond_counts.get(hex).copied().unwrap_or(0);
            if word_count <= 2 && bc > 100 {
                candidates.push((hex.to_string(), label.clone(),
                    format!("short label ({} words) with {} bonds — likely index entry", word_count, bc)));
                continue;
            }
        }

        let total_candidates = candidates.len();
        let mut revoked = Vec::new();

        for (hex, label, reason) in &candidates {
            if let Ok(seed) = Seed::from_hex(hex) {
                self.network.revoke(&seed, &format!("glossary_cleanup: {}", reason));
                revoked.push((label.clone(), reason.clone()));
            }
        }

        GlossaryCleanupReport {
            candidates_found: total_candidates,
            nodes_revoked: revoked.len(),
            details: revoked,
        }
    }

    /// Scan all active nodes for junk accumulated during PDF textbook training.
    /// Detects page numbers, TOC entries, index references, copyright notices,
    /// exercise prompts, formatting artifacts, and too-short labels.
    ///
    /// - execute=false → dry run: report only, revoke nothing
    /// - execute=true  → actually revoke all flagged nodes
    /// - min_label_length: nodes with labels shorter than this (in chars) are flagged
    pub fn brain_cleanup(&mut self, execute: bool, min_label_length: usize) -> BrainCleanupReport {
        let mut candidates: Vec<(String, String, String)> = Vec::new(); // (hex, label, category)

        for (hex, node) in self.network.active_nodes() {
            let label = &node.label;
            let label_trimmed = label.trim();
            let lower = label_trimmed.to_lowercase();
            let char_count = label_trimmed.chars().count();

            // Too short
            if char_count < min_label_length {
                candidates.push((hex.to_string(), label.clone(), "too_short".to_string()));
                continue;
            }

            // Pure page number (only digits)
            if label_trimmed.chars().all(|c| c.is_ascii_digit()) {
                candidates.push((hex.to_string(), label.clone(), "page_refs".to_string()));
                continue;
            }

            // Roman numeral only lines
            if junk_is_roman_numeral_only(label_trimmed) {
                candidates.push((hex.to_string(), label.clone(), "page_refs".to_string()));
                continue;
            }

            // Bare chapter/section headers with no title text
            if junk_is_bare_chapter_header(label_trimmed) || junk_is_bare_section_header(label_trimmed) {
                candidates.push((hex.to_string(), label.clone(), "page_refs".to_string()));
                continue;
            }

            // "Page N", "See page", "See also", "Refer to"
            if lower.starts_with("page ") || lower.starts_with("see page")
                || lower.starts_with("see also") || lower.starts_with("refer to")
            {
                candidates.push((hex.to_string(), label.clone(), "page_refs".to_string()));
                continue;
            }

            // Structural section labels
            if matches!(lower.as_str(),
                "table of contents" | "index" | "appendix" | "bibliography" | "glossary"
                | "references" | "contents" | "preface" | "foreword"
                | "acknowledgements" | "acknowledgments"
            ) {
                candidates.push((hex.to_string(), label.clone(), "page_refs".to_string()));
                continue;
            }

            // TOC entries ("Topic . . . . 42" or tab/space-separated page numbers)
            if junk_is_toc_entry(label_trimmed) {
                candidates.push((hex.to_string(), label.clone(), "page_refs".to_string()));
                continue;
            }

            // Index entries ("term, 23, 45, 67")
            if junk_is_index_entry(label_trimmed) {
                candidates.push((hex.to_string(), label.clone(), "page_refs".to_string()));
                continue;
            }

            // Copyright/ISBN/publisher boilerplate
            if lower.contains("copyright ©")
                || (lower.contains('©') && char_count < 200)
                || lower.starts_with("isbn")
                || lower.contains("all rights reserved")
                || lower.starts_with("published by")
                || lower.starts_with("printed in")
            {
                candidates.push((hex.to_string(), label.clone(), "formatting_artifacts".to_string()));
                continue;
            }

            // Bare "Figure N" / "Table N" references
            if junk_is_bare_figure_ref(label_trimmed) {
                candidates.push((hex.to_string(), label.clone(), "formatting_artifacts".to_string()));
                continue;
            }

            // Mostly non-alphanumeric (formatting artifacts, bullets, etc.)
            let alpha_count = label_trimmed.chars().filter(|c| c.is_alphanumeric()).count();
            if char_count > 3 && alpha_count * 3 < char_count {
                candidates.push((hex.to_string(), label.clone(), "formatting_artifacts".to_string()));
                continue;
            }

            // Exercise/problem/question prompts
            if junk_is_exercise_prompt(label_trimmed) {
                candidates.push((hex.to_string(), label.clone(), "textbook_filler".to_string()));
                continue;
            }

            // Short multiple-choice option lines: "a)", "b)", "A.", "B." etc.
            if char_count < 20 {
                let mc_prefixes = ["a)", "b)", "c)", "d)", "e)", "a.", "b.", "c.", "d.", "e.",
                                   "A)", "B)", "C)", "D)", "E)", "A.", "B.", "C.", "D.", "E."];
                if mc_prefixes.iter().any(|p| label_trimmed.starts_with(p)) {
                    candidates.push((hex.to_string(), label.clone(), "textbook_filler".to_string()));
                    continue;
                }
            }

            // ── Content junk: generic questions with no answer value ──────────
            if junk_is_generic_question(label_trimmed) {
                candidates.push((hex.to_string(), label.clone(), "content_junk".to_string()));
                continue;
            }

            // ── Content junk: textbook meta-text ─────────────────────────────
            if junk_is_textbook_meta(label_trimmed) {
                candidates.push((hex.to_string(), label.clone(), "content_junk".to_string()));
                continue;
            }

            // ── Content junk: attribution/citation fragments ──────────────────
            if junk_is_attribution(label_trimmed) {
                candidates.push((hex.to_string(), label.clone(), "content_junk".to_string()));
                continue;
            }

            // ── Content junk: children's book / whimsical non-factual ────────
            if junk_is_whimsical_fragment(label_trimmed) {
                candidates.push((hex.to_string(), label.clone(), "content_junk".to_string()));
                continue;
            }
        }

        // ── Repeated structural prefix detection (second pass) ────────────────
        // Count how many nodes start with the same 5-word prefix. If > 10 share
        // a prefix they're likely template/structural content.
        {
            let mut prefix_counts: HashMap<String, usize> = HashMap::new();
            let mut prefix_to_nodes: HashMap<String, Vec<(String, String)>> = HashMap::new();
            // Only consider nodes NOT already flagged
            let already_flagged: HashSet<String> = candidates.iter().map(|(h,_,_)| h.clone()).collect();
            for (hex, node) in self.network.active_nodes() {
                if already_flagged.contains(hex) { continue; }
                let label = node.label.trim();
                let prefix = five_word_prefix(label);
                if prefix.split_whitespace().count() >= 5 {
                    *prefix_counts.entry(prefix.clone()).or_insert(0) += 1;
                    prefix_to_nodes.entry(prefix).or_default().push((hex.to_string(), node.label.clone()));
                }
            }
            for (prefix, count) in &prefix_counts {
                if *count > 10 {
                    if let Some(nodes) = prefix_to_nodes.get(prefix) {
                        for (hex, label) in nodes {
                            candidates.push((hex.clone(), label.clone(), "content_junk".to_string()));
                        }
                    }
                }
            }
        }

        // recompute content_junk after second pass (done in tally below)

        // Tally by category
        let mut page_refs = 0usize;
        let mut textbook_filler = 0usize;
        let mut formatting_artifacts = 0usize;
        let mut too_short = 0usize;
        let mut content_junk = 0usize;
        for (_, _, cat) in &candidates {
            match cat.as_str() {
                "page_refs"            => page_refs += 1,
                "textbook_filler"      => textbook_filler += 1,
                "formatting_artifacts" => formatting_artifacts += 1,
                "too_short"            => too_short += 1,
                "content_junk"         => content_junk += 1,
                _ => {}
            }
        }

        let total_flagged = candidates.len();
        let mut revoked_count = 0usize;

        if execute {
            for (hex, _label, category) in &candidates {
                if let Ok(seed) = Seed::from_hex(hex) {
                    self.network.revoke(&seed, &format!("brain_cleanup: {}", category));
                    revoked_count += 1;
                }
            }
            self.word_index_valid = false;
            self.phrase_index_cache = None;
        }

        // Cap details at 500 to avoid giant responses
        let details: Vec<(String, String)> = candidates.iter()
            .take(500)
            .map(|(_, label, cat)| (label.clone(), cat.clone()))
            .collect();

        BrainCleanupReport {
            page_refs,
            textbook_filler,
            formatting_artifacts,
            too_short,
            content_junk,
            total_flagged,
            revoked: revoked_count,
            dry_run: !execute,
            details,
        }
    }

    // ━━━ Invention Engine Public API ━━━━━━━━━━━━━━━━━━━━━━━━━━━

    /// Get invention engine status.
    pub fn invention_status(&self) -> invention::InventionStatus {
        self.invention.status()
    }

    /// Get recent bridge events.
    pub fn invention_recent_bridges(&self, n: usize) -> Vec<&invention::BridgeEvent> {
        self.invention.recent_bridges(n)
    }

    // ━━━ Self-Improvement Public API ━━━━━━━━━━━━━━━━━━━━━━━━━━

    /// Get self-improvement engine status.
    pub fn self_improvement_status(&self) -> self_improvement::SelfImprovementStatus {
        self.self_improvement.status()
    }

    /// Run meta-reflection: analyse patterns and propose improvements.
    pub fn meta_reflect(&mut self) -> Vec<self_improvement::MetaReflection> {
        self.self_improvement.meta_reflect()
    }

    /// Run a synthesis pass: find knowledge triangles with missing bonds.
    pub fn synthesis_pass(&self) -> knowledge_health::SynthesisReport {
        let viz = self.network.viz_bonds();
        let label_map = self.network.bond_label_map();
        let node_labels = self.network.node_label_map();

        // Build bond list with labels
        let bonds: Vec<(String, String, f32, String)> = viz.iter().map(|(s, t, w)| {
            let key = format!("{}→{}", s, t);
            let label = label_map.get(&key).cloned()
                .unwrap_or_else(|| "related".to_string());
            (s.clone(), t.clone(), *w, label)
        }).collect();

        // Build existing bond set for fast lookup
        let existing: std::collections::HashSet<String> = viz.iter()
            .map(|(s, t, _)| format!("{}→{}", s, t))
            .collect();

        knowledge_health::synthesis_pass(&bonds, &existing, &node_labels)
    }

    // ━━━ Orchestrator ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    /// The smart front door. Takes raw user input and routes it
    /// to the correct Brain subsystem. Returns a clean, conversational response.
    /// No /commands needed — just talk to Lu.
    pub fn orchestrate(&mut self, input: &str) -> orchestrator::OrchestratorResponse {
        let action = orchestrator::classify_input(input);

        match action {
            orchestrator::OrchestratorAction::Greet => {
                orchestrator::OrchestratorResponse {
                    action: "greet".into(),
                    response: "Hello! I'm Lu. Ask me anything, or just tell me something and I'll learn it.".into(),
                    confidence: 1.0,
                    pending_action: None,
                    atoms_used: 0,
                    scar_warnings: vec![],
                }
            }

            orchestrator::OrchestratorAction::MetaSelf => {
                let stats = self.network.stats();
                orchestrator::OrchestratorResponse {
                    action: "meta_self".into(),
                    response: format!(
                        "I'm Lu — an associative intelligence built on the DialectForge protocol. \
                        I have {} knowledge nodes connected by {} bonds. I learn through pattern encoding, \
                        think through spreading activation, and everything is gated by the terminal bond. \
                        No LLM — just a graph that thinks.",
                        stats.total_nodes, stats.total_bonds
                    ),
                    confidence: 1.0,
                    pending_action: None,
                    atoms_used: 0,
                    scar_warnings: vec![],
                }
            }

            orchestrator::OrchestratorAction::Respond { query } => {
                let result = self.ask(&query, 3, 10);
                orchestrator::OrchestratorResponse {
                    action: "respond".into(),
                    response: orchestrator::polish_response(&result.text),
                    confidence: result.coherence_score,
                    pending_action: None,
                    atoms_used: result.atoms_used,
                    scar_warnings: result.scar_warnings,
                }
            }

            orchestrator::OrchestratorAction::Reason { query } => {
                let result = self.reason(&query, 3);
                let response = if !result.direct_answer.is_empty() {
                    let mut text = result.direct_answer.clone();
                    if !result.chains.is_empty() {
                        let best = &result.chains[0];
                        if best.confidence > 5.0 && !best.synthesis.is_empty() {
                            text = format!("{}. My reasoning: {}", text, best.synthesis);
                        }
                    }
                    text
                } else {
                    result.conclusion.clone()
                };
                orchestrator::OrchestratorResponse {
                    action: "reason".into(),
                    response: orchestrator::polish_response(&response),
                    confidence: if result.chains.is_empty() { 0.3 } else {
                        (result.chains[0].confidence / 20.0).min(0.95)
                    },
                    pending_action: None,
                    atoms_used: result.chains.len(),
                    scar_warnings: vec![],
                }
            }

            orchestrator::OrchestratorAction::Think { goal } => {
                let config = coordinator::ThinkConfig {
                    dry_run: true,
                    auto_approve_safe: true,
                    ..Default::default()
                };
                match self.think_start(&goal, config) {
                    Ok(state) => {
                        orchestrator::OrchestratorResponse {
                            action: "think".into(),
                            response: format!(
                                "Thinking about: {}. Found {} sub-goals so far. \
                                State: {:?}, coherence: {:.2}.",
                                goal, state.sub_goal_count, state.state, state.coherence
                            ),
                            confidence: state.coherence,
                            pending_action: Some("Use /think_step to advance, /think_stop to finish.".into()),
                            atoms_used: state.sub_goal_count,
                            scar_warnings: vec![],
                        }
                    }
                    Err(e) => {
                        orchestrator::OrchestratorResponse {
                            action: "think".into(),
                            response: format!("I tried to think about that but hit a wall: {}", e),
                            confidence: 0.0,
                            pending_action: None,
                            atoms_used: 0,
                            scar_warnings: vec![],
                        }
                    }
                }
            }

            orchestrator::OrchestratorAction::ToolAction { query } => {
                match self.tool_detect(&query) {
                    Some((id, intent)) => {
                        let status = if intent.confidence >= 0.6 {
                            format!("Request {} queued ({}, confidence {:.0}%). ", id, intent.tool_type, intent.confidence * 100.0)
                        } else {
                            "Detected but confidence too low to queue. ".to_string()
                        };
                        let pending = if intent.tool_type.to_string().contains("file_read")
                            || intent.tool_type.to_string().contains("web_fetch") {
                            None // Safe tools auto-approve
                        } else {
                            Some(format!("Awaiting approval. Use /tool_approve {} to proceed.", id))
                        };
                        orchestrator::OrchestratorResponse {
                            action: "tool_action".into(),
                            response: format!("{}I want to {} — {}",
                                status, intent.tool_type, intent.explanation),
                            confidence: intent.confidence,
                            pending_action: pending,
                            atoms_used: 0,
                            scar_warnings: vec![],
                        }
                    }
                    None => {
                        // Fall back to conversational
                        let result = self.ask(&query, 3, 10);
                        orchestrator::OrchestratorResponse {
                            action: "respond".into(),
                            response: orchestrator::polish_response(&result.text),
                            confidence: result.coherence_score,
                            pending_action: None,
                            atoms_used: result.atoms_used,
                            scar_warnings: result.scar_warnings,
                        }
                    }
                }
            }

            orchestrator::OrchestratorAction::Learn { content, source } => {
                let proposal = self.propose(&content, &source);
                let preview = if content.len() > 80 {
                    format!("{}...", &content[..80])
                } else {
                    content
                };
                orchestrator::OrchestratorResponse {
                    action: "learn".into(),
                    response: format!("Got it. I've created a learning proposal for: \"{}\"", preview),
                    confidence: 1.0,
                    pending_action: Some(format!("Proposal {} awaiting approval.", proposal.id)),
                    atoms_used: 0,
                    scar_warnings: vec![],
                }
            }

            orchestrator::OrchestratorAction::Ingest { content } => {
                let report = self.ingest_text(&content, "user_ingest");
                orchestrator::OrchestratorResponse {
                    action: "ingest".into(),
                    response: format!(
                        "Processed {} sentences. {} proposals ready, {} blocked, {} duplicates skipped.",
                        report.total_sentences, report.proposals_ready,
                        report.proposals_blocked, report.duplicates_skipped
                    ),
                    confidence: 0.8,
                    pending_action: if report.proposals_ready > 0 {
                        Some("Review and approve proposals to commit to memory.".into())
                    } else {
                        None
                    },
                    atoms_used: 0,
                    scar_warnings: vec![],
                }
            }

            orchestrator::OrchestratorAction::Explore { topic } => {
                // Deeper recall with more hops and atoms
                let result = self.ask(&topic, 4, 15);
                orchestrator::OrchestratorResponse {
                    action: "explore".into(),
                    response: orchestrator::polish_response(&result.text),
                    confidence: result.coherence_score,
                    pending_action: None,
                    atoms_used: result.atoms_used,
                    scar_warnings: result.scar_warnings,
                }
            }

            orchestrator::OrchestratorAction::Unknown { input } => {
                // Try conversational as last resort
                let result = self.ask(&input, 3, 10);
                if result.atoms_used > 0 {
                    orchestrator::OrchestratorResponse {
                        action: "respond".into(),
                        response: orchestrator::polish_response(&result.text),
                        confidence: result.coherence_score,
                        pending_action: None,
                        atoms_used: result.atoms_used,
                        scar_warnings: result.scar_warnings,
                    }
                } else {
                    orchestrator::OrchestratorResponse {
                        action: "unknown".into(),
                        response: "I'm not sure what to do with that. Try asking me a question, or say 'learn that...' to teach me something.".into(),
                        confidence: 0.0,
                        pending_action: None,
                        atoms_used: 0,
                        scar_warnings: vec![],
                    }
                }
            }
        }
    }

    // ━━━ Tool Integration Layer ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    /// Get the tool engine status.
    pub fn tool_status(&self) -> tools::ToolEngineStatus {
        tools::ToolEngineStatus::from(&self.tool_engine)
    }

    /// Process a query through intent detection. Returns request ID + intent if tool action detected.
    pub fn tool_detect(&mut self, query: &str) -> Option<(String, tools::DetectedIntent)> {
        self.tool_engine.process_query(query)
    }

    /// List pending tool requests awaiting approval.
    pub fn tool_pending(&self) -> Vec<tools::PendingToolSummary> {
        self.tool_engine.awaiting_approval().iter()
            .map(|r| tools::PendingToolSummary::from(*r))
            .collect()
    }

    /// Approve a pending tool request.
    pub fn tool_approve(&mut self, id: &str) -> Result<(), String> {
        self.tool_engine.approve(id)
    }

    /// Reject a pending tool request.
    pub fn tool_reject(&mut self, id: &str, reason: &str) -> Result<(), String> {
        self.tool_engine.reject(id, reason)
    }

    /// Record a tool execution result.
    pub fn tool_record_result(&mut self, result: tools::ToolResult) {
        self.tool_engine.record_result(result);
    }

    /// Get tool requests that are ready to execute.
    pub fn tool_ready(&self) -> Vec<tools::PendingToolSummary> {
        self.tool_engine.ready_to_execute().iter()
            .map(|r| tools::PendingToolSummary::from(*r))
            .collect()
    }

    /// Enable/disable the tool engine.
    pub fn tool_set_enabled(&mut self, enabled: bool) {
        self.tool_engine.enabled = enabled;
    }

    /// Garbage collect completed tool requests.
    pub fn tool_gc(&mut self) -> usize {
        self.tool_engine.gc()
    }

    /// Register a custom tool.
    pub fn tool_register_custom(&mut self, name: &str, description: &str, risk: &str, params: Vec<String>) -> Result<(), String> {
        let risk = match risk {
            "safe" => tools::ToolRisk::Safe,
            "moderate" => tools::ToolRisk::Moderate,
            "dangerous" => tools::ToolRisk::Dangerous,
            _ => return Err(format!("Unknown risk level: {}. Use safe/moderate/dangerous", risk)),
        };
        self.tool_engine.register_custom(tools::CustomToolDef {
            name: name.to_string(),
            description: description.to_string(),
            risk,
            parameters: params,
        });
        Ok(())
    }

    // ═══ Web Research Engine API ═══════════════════════════════════════

    /// Research a topic by searching the web. Lu searches, browses,
    /// thinks, follows links, and extracts knowledge.
    pub fn web_research(&mut self, query: &str) -> web_research::ResearchResult {
        self.idle_explorer.record_interaction();
        let safety = web_safety::WebSafety::new();
        let fetcher = web_fetch::WebFetcher::new(safety);
        let researcher = web_research::WebResearcher::new(fetcher);
        researcher.research(query)
    }

    /// Research a specific URL. Lu reads the page, follows links, extracts knowledge.
    pub fn web_research_url(&mut self, url: &str, query: &str) -> web_research::ResearchResult {
        self.idle_explorer.record_interaction();
        let safety = web_safety::WebSafety::new();
        let fetcher = web_fetch::WebFetcher::new(safety);
        let researcher = web_research::WebResearcher::new(fetcher);
        researcher.research_url(url, query)
    }

    /// Ingest all knowledge from a research result into the brain.
    /// Returns how many facts were proposed for learning.
    pub fn web_ingest_research(&mut self, result: &web_research::ResearchResult) -> usize {
        let mut count = 0;
        for fact in &result.knowledge_extracted {
            if fact.confidence < 0.2 { continue; }
            let source = format!("web:{}", fact.source_url);
            let _proposal = self.propose(&fact.content, &source);
            count += 1;
        }
        // Ingest image knowledge through the multimodal perception pipeline.
        // Creates Media-type nodes with pattern encoding of the visual description,
        // enabling cross-modal search to find these images by concept.
        for img in &result.images_found {
            if img.relevance < 0.2 { continue; }
            count += self.web_ingest_image_perception(img);
        }
        count
    }

    /// Ingest a web image through the multimodal perception pipeline.
    /// Instead of just storing "[Image: url] description" as a text Fact,
    /// this creates a Media-type node with a cross-modal pattern encoding
    /// derived from the image's textual context (alt text, caption,
    /// surrounding text). This lets Lu's cross-modal search find web
    /// images by conceptual similarity.
    fn web_ingest_image_perception(&mut self, img: &web_research::ImageKnowledge) -> usize {
        let label = format!("[Image: {}] {}", img.image_url, img.description);

        // Check for duplicate
        if self.find_existing_content(&label).is_some() {
            return 0;
        }

        // Encode the description through the pattern engine.
        // This places the image in the same 256-dim vector space as text,
        // enabling cross-modal search between text queries and image knowledge.
        let pattern = self.pattern_engine.encode(&img.description);

        let mut node = KnowledgeNode::new(
            NodeType::Media,
            label,
            pattern,
            Confidence::Unverified,
            &format!("web_perception:{}", img.source_url),
        );

        // Store image URL and source in metadata for later retrieval
        node.metadata.insert("image_url".to_string(), img.image_url.clone());
        node.metadata.insert("source_url".to_string(), img.source_url.clone());
        node.metadata.insert("perception_type".to_string(), "web_visual_context".to_string());

        let seed = self.network.add_node(node);

        // Auto-associate with related knowledge so the image appears
        // in spreading activation when its concepts are queried
        self.auto_associate(&seed, 0.3, 3);
        1
    }

    /// Check if Lu should explore and do it if so.
    /// Call this periodically (e.g., on every MCP request).
    /// Returns Some(result) if exploration happened.
    pub fn web_check_idle(&mut self) -> Option<web_research::ResearchResult> {
        if !self.idle_explorer.should_explore() { return None; }

        // Pick top wonder from curiosity engine
        let wonder = self.curiosity.wonder_queue.peek(1).first().cloned()?.clone();

        // Research it
        let result = self.web_research(&wonder.suggested_query);

        // Record the exploration
        self.idle_explorer.record_exploration(web_idle::ExplorationRecord {
            wonder_id: wonder.id.clone(),
            query: wonder.suggested_query.clone(),
            pages_visited: result.pages_visited.len(),
            facts_learned: result.knowledge_extracted.len(),
            images_found: result.images_found.len(),
            gap_filled: result.gap_filled,
            started_at: chrono::Utc::now(),
            duration_ms: result.total_time_ms,
        });

        // Ingest what we learned
        self.web_ingest_research(&result);

        // If gap was filled, resolve it in curiosity engine
        if result.gap_filled {
            self.curiosity.resolve_gap(&wonder.id, &wonder.domain);
        }

        Some(result)
    }

    /// Get idle explorer stats.
    pub fn web_idle_stats(&self) -> web_idle::IdleExplorerStats {
        self.idle_explorer.stats()
    }

    /// Record user interaction (resets idle timer).
    pub fn web_record_interaction(&mut self) {
        self.idle_explorer.record_interaction();
    }

    /// Set idle exploration enabled/disabled.
    pub fn web_set_idle_enabled(&mut self, enabled: bool) {
        self.idle_explorer.set_enabled(enabled);
    }

    /// Truncate a label for display.
    fn truncate_label(label: &str, max: usize) -> String {
        if label.len() <= max {
            label.to_string()
        } else {
            format!("{}...", &label[..max - 3])
        }
    }

    // ━━━ Computer Use ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    /// Generate a code block. Uses Lu's knowledge to inform the description,
    /// but the actual code content is provided by the caller (e.g. Claude via MCP).
    pub fn code_generate(&mut self, description: &str, language: &str, code: &str) -> computer_use::CodeBlock {
        computer_use::ComputerUseEngine::scaffold_code_block(description, language, code)
    }

    /// Execute a code block in the sandbox.
    pub fn code_execute(&mut self, code: &computer_use::CodeBlock) -> Result<computer_use::ExecutionResult, String> {
        self.computer_use.execute(code)
    }

    /// Read a file from the sandbox.
    pub fn sandbox_file_read(&self, filename: &str) -> Result<String, String> {
        self.computer_use.file_read(filename)
    }

    /// Write a file to the sandbox.
    pub fn sandbox_file_write(&self, filename: &str, content: &str) -> Result<String, String> {
        self.computer_use.file_write(filename, content)
    }

    /// List files in the sandbox.
    pub fn sandbox_file_list(&self, subdir: Option<&str>) -> Result<Vec<computer_use::FileEntry>, String> {
        self.computer_use.file_list(subdir)
    }

    /// Get execution history.
    pub fn code_execution_history(&self) -> &[computer_use::ExecutionRecord] {
        self.computer_use.execution_history()
    }

    /// Check if a query is asking for code.
    pub fn is_code_request(query: &str) -> bool {
        computer_use::ComputerUseEngine::is_code_request(query)
    }

    /// Detect language from query.
    pub fn detect_code_language(query: &str) -> String {
        computer_use::ComputerUseEngine::detect_language(query)
    }

    /// Get sandbox directory path.
    pub fn sandbox_dir(&self) -> &std::path::Path {
        &self.computer_use.sandbox_dir
    }

    /// Clean up the sandbox.
    pub fn sandbox_cleanup(&self) -> Result<(), String> {
        self.computer_use.cleanup()
    }
}

// ─── Rumination Trait Implementation ─────────────────────────

impl rumination::BrainRuminator for Brain {
    fn active_node_hexes(&self) -> Vec<String> {
        self.network.active_nodes()
            .map(|(hex, _)| hex.to_string())
            .collect()
    }

    fn node_label(&self, hex: &str) -> Option<String> {
        self.network.get_node_by_hex(hex).map(|n| n.label.clone())
    }

    fn activate_from(
        &mut self,
        seed_hex: &str,
        depth: usize,
        top_k: usize,
    ) -> Vec<(String, String, f32)> {
        let seed = match Seed::from_hex(seed_hex) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let activation_set = vec![(seed, 1.0)];
        let result = self.network.spread_activation(
            &activation_set, depth, top_k,
        );
        result.activated_nodes.iter()
            .map(|n| (n.seed_hex.clone(), n.label.clone(), n.strength))
            .collect()
    }

    fn has_bond(&self, hex_a: &str, hex_b: &str) -> bool {
        self.network.bond_weight(hex_a, hex_b).is_some()
    }

    fn create_rumination_bond(
        &mut self,
        hex_a: &str,
        hex_b: &str,
        strength: f32,
    ) -> bool {
        let seed_a = match Seed::from_hex(hex_a) {
            Ok(s) => s,
            Err(_) => return false,
        };
        let seed_b = match Seed::from_hex(hex_b) {
            Ok(s) => s,
            Err(_) => return false,
        };
        self.network.add_bond(
            &seed_a,
            &seed_b,
            "rumination: co-activated without direct bond".to_string(),
            strength,
            "rumination",
        ).is_ok()
    }
}


// ━━━ PDF Text Cleaning ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Clean raw text extracted from a PDF before ingestion.
///
/// Strips common junk generated by PDF-to-text extraction:
/// - Page numbers (bare digit lines)
/// - Repeated headers/footers (lines occurring 3+ times)
/// - Table of contents entries (dot leaders, tab-separated page numbers)
/// - Index entries ("term, 23, 45, 67")
/// - Copyright/ISBN/publisher boilerplate
/// - Bare "Chapter N" / "Section N.M" headers (no title text)
/// - Exercise/question prompts with no answer content
/// - Formatting artifacts (mostly special chars)
/// - Lines under 5 characters
/// - Multiple consecutive blank lines collapsed to one
pub fn clean_pdf_text(raw_text: &str) -> String {
    let lines: Vec<&str> = raw_text.lines().collect();

    // Count line frequencies to detect repeated headers/footers
    let mut freq: HashMap<String, usize> = HashMap::new();
    for line in &lines {
        let t = line.trim();
        if !t.is_empty() && t.len() < 120 {
            *freq.entry(t.to_lowercase()).or_insert(0) += 1;
        }
    }

    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut consecutive_blanks = 0usize;
    // Accumulates consecutive code lines before flushing as a tagged block
    let mut code_block: Vec<&str> = Vec::new();

    for line in &lines {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            // Blank line ends any open code block (bonds block to prose above)
            pdf_flush_code_block(&mut code_block, &mut out);
            consecutive_blanks += 1;
            if consecutive_blanks <= 1 {
                out.push(String::new());
            }
            continue;
        }
        consecutive_blanks = 0;

        // Code lines bypass all junk filters and accumulate into the current block
        if is_code_line(line) {
            code_block.push(line);
            continue;
        }

        // Transitioning out of a code block — flush before processing prose
        pdf_flush_code_block(&mut code_block, &mut out);

        // Strip lines under 5 characters
        if trimmed.len() < 5 {
            continue;
        }

        // Strip repeated headers/footers (3+ occurrences)
        if trimmed.len() < 120 {
            if freq.get(&trimmed.to_lowercase()).copied().unwrap_or(0) >= 3 {
                continue;
            }
        }

        // Strip pure page numbers
        if trimmed.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        let lower = trimmed.to_lowercase();

        // Strip "Page N" lines
        if lower.starts_with("page ") {
            let rest = lower[5..].trim();
            if rest.chars().all(|c| c.is_ascii_digit() || c == ' ' || c == 'f' || c == 'o') {
                continue;
            }
        }

        // Strip bare chapter/section headers
        if junk_is_bare_chapter_header(trimmed) || junk_is_bare_section_header(trimmed) {
            continue;
        }

        // Strip TOC entries
        if junk_is_toc_entry(trimmed) {
            continue;
        }

        // Strip index entries ("term, 23, 45, 67")
        if junk_is_index_entry(trimmed) {
            continue;
        }

        // Strip structural labels
        if matches!(lower.as_str(),
            "table of contents" | "index" | "appendix" | "bibliography" | "glossary"
            | "references" | "contents" | "preface" | "foreword"
            | "acknowledgements" | "acknowledgments"
        ) {
            continue;
        }

        // Strip navigation markers
        if lower.starts_with("see page") || lower.starts_with("see also")
            || lower.starts_with("refer to")
        {
            continue;
        }

        // Strip Roman-numeral-only lines
        if junk_is_roman_numeral_only(trimmed) {
            continue;
        }

        // Strip copyright/ISBN/publisher boilerplate
        if lower.contains("copyright ©")
            || (lower.contains('©') && trimmed.len() < 200)
            || lower.starts_with("isbn")
            || lower.contains("all rights reserved")
            || lower.starts_with("published by")
            || lower.starts_with("printed in")
        {
            continue;
        }

        // Strip exercise/question prompts
        if junk_is_exercise_prompt(trimmed) {
            continue;
        }

        // Strip bare figure/table references
        if junk_is_bare_figure_ref(trimmed) {
            continue;
        }

        out.push(line.to_string());
    }

    // Flush any trailing code block
    pdf_flush_code_block(&mut code_block, &mut out);

    out.join("\n")
}

// ─── Junk-detection helpers ───────────────────────────────────────────────────

fn junk_is_bare_chapter_header(s: &str) -> bool {
    let lower = s.to_lowercase();
    if !lower.starts_with("chapter ") {
        return false;
    }
    // Skip "chapter "
    let rest = s[8..].trim();
    // Consume leading digits
    let after_digits: &str = rest.trim_start_matches(|c: char| c.is_ascii_digit());
    // What remains (after digits) should be empty for a bare header
    after_digits.trim().is_empty()
}

fn junk_is_bare_section_header(s: &str) -> bool {
    let lower = s.to_lowercase();
    if !lower.starts_with("section ") {
        return false;
    }
    let rest = s[8..].trim();
    // "Section 1.2" or "Section 1.2.3" — only digits and dots
    !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit() || c == '.')
}

fn junk_is_toc_entry(s: &str) -> bool {
    // Must end with a digit
    if !s.chars().last().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        return false;
    }
    // Dot leaders
    if s.contains("...") || s.contains(". . .") {
        return true;
    }
    // Tab-separated page number at end
    if s.contains('\t') {
        let last = s.rsplit('\t').next().unwrap_or("").trim();
        if last.chars().all(|c| c.is_ascii_digit()) {
            return true;
        }
    }
    // Multiple spaces before trailing digits
    if s.contains("   ") {
        let last_word = s.split_whitespace().last().unwrap_or("");
        if last_word.chars().all(|c| c.is_ascii_digit()) {
            let prefix = &s[..s.len().saturating_sub(last_word.len())];
            if prefix.ends_with("   ") || prefix.ends_with("  ") {
                return true;
            }
        }
    }
    false
}

fn junk_is_index_entry(s: &str) -> bool {
    // "term, 23, 45, 67" — at least two purely-numeric comma parts after the first
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() < 3 {
        return false;
    }
    let numeric_suffix_count = parts[1..].iter()
        .filter(|p| !p.trim().is_empty() && p.trim().chars().all(|c| c.is_ascii_digit()))
        .count();
    numeric_suffix_count >= 2
}

fn junk_is_roman_numeral_only(s: &str) -> bool {
    matches!(s.to_lowercase().trim(),
        "i" | "ii" | "iii" | "iv" | "v" | "vi" | "vii" | "viii" | "ix" | "x"
        | "xi" | "xii" | "xiii" | "xiv" | "xv" | "xvi" | "xvii" | "xviii" | "xix" | "xx"
        | "xxi" | "xxii" | "xxiii" | "xxiv" | "xxv" | "l" | "c" | "d" | "m"
    )
}

fn junk_is_exercise_prompt(s: &str) -> bool {
    let lower = s.to_lowercase();
    // "Exercise N", "Problem N", "Question N"
    for prefix in &["exercise ", "problem ", "question "] {
        if lower.starts_with(prefix) {
            let rest = lower[prefix.len()..].trim_start();
            if rest.starts_with(|c: char| c.is_ascii_digit()) {
                return true;
            }
        }
    }
    // Answer/Solution/Hint
    if lower.starts_with("answer:") || lower.starts_with("solution:") || lower.starts_with("hint:") {
        return true;
    }
    // Generic "what do you know" style filler
    let filler_starts = [
        "what do you already know about",
        "what do you want to know",
        "consider what you want to know",
        "how does this work reflect",
    ];
    filler_starts.iter().any(|f| lower.starts_with(f))
}

fn junk_is_bare_figure_ref(s: &str) -> bool {
    let lower = s.to_lowercase();
    for prefix in &["figure ", "table "] {
        if lower.starts_with(prefix) {
            let rest = lower[prefix.len()..].trim();
            if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit() || c == '.') {
                return true;
            }
        }
    }
    false
}

/// Generic questions that have no answer content — they are prompts, not knowledge.
fn junk_is_generic_question(s: &str) -> bool {
    let lower = s.to_lowercase();
    let question_starts = [
        "what do you ", "how do you ", "why do you ", "can you ", "do you ",
        "what does ", "how does ",
    ];
    for prefix in &question_starts {
        if lower.starts_with(prefix) {
            // "what does" / "how does" under 60 chars → textbook exercise prompt
            if *prefix == "what does " || *prefix == "how does " {
                if s.chars().count() < 60 {
                    return true;
                }
            } else {
                return true;
            }
        }
    }
    // Filler engagement phrases
    lower.contains("what you want to know") || lower.contains("what you already know")
}

/// Textbook meta-text: chapter navigation, learning objective boilerplate.
fn junk_is_textbook_meta(s: &str) -> bool {
    let lower = s.to_lowercase();
    let meta_phrases = [
        "in this chapter", "in this section", "in the next chapter", "in the previous chapter",
        "we will discuss", "we will explore", "we have learned", "we have seen",
        "as mentioned earlier", "as discussed in", "as we saw in",
        "learning objectives", "key terms", "review questions", "discussion questions",
        "at the end of this chapter", "after reading this chapter", "by the end of this",
    ];
    meta_phrases.iter().any(|p| lower.starts_with(p) || lower.contains(p))
}

/// Attribution/citation fragments with no knowledge content.
fn junk_is_attribution(s: &str) -> bool {
    let lower = s.to_lowercase();
    // "Retrieved from", "Accessed on", "Available at"
    if lower.starts_with("retrieved from") || lower.starts_with("accessed on") || lower.starts_with("available at") {
        return true;
    }
    // Pure URL
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("www.") {
        return true;
    }
    // "Smith (2019)" or "Johnson et al., 2020" style citation-only nodes
    // Pattern: short label (< 60 chars) containing "(20XX)" or "et al" with no other content
    let char_count = s.chars().count();
    if char_count < 60 {
        let has_year = {
            let mut found = false;
            for year in 1900u32..=2030 {
                if lower.contains(&format!("({})", year)) || lower.contains(&format!(", {}", year)) {
                    found = true;
                    break;
                }
            }
            found
        };
        if has_year && (lower.contains("et al") || {
            // Check if it's mostly a name + date (no sentence structure)
            !s.contains('.') || lower.ends_with(')')
        }) {
            return true;
        }
    }
    false
}

/// Very short whimsical/children's book fragments with no factual content.
fn junk_is_whimsical_fragment(s: &str) -> bool {
    let char_count = s.chars().count();
    if char_count >= 40 {
        return false;
    }
    // Must contain an exclamation mark (whimsical style marker)
    if !s.contains('!') {
        return false;
    }
    // Must not contain any technical indicator words
    let lower = s.to_lowercase();
    let technical_indicators = [
        "function", "method", "class", "type", "system", "data", "value",
        "algorithm", "process", "network", "memory", "code", "error",
        "protocol", "module", "struct", "variable", "parameter",
    ];
    !technical_indicators.iter().any(|t| lower.contains(t))
}

// ─── Code-block detection helpers ─────────────────────────────────────────────

/// Returns true when a raw PDF line looks like source code.
/// Checks for leading indentation and keywords that rarely open prose sentences.
fn is_code_line(line: &str) -> bool {
    // Leading 4+ spaces or a tab almost always means indented code in PDFs
    if line.starts_with("    ") || line.starts_with("\t") {
        return true;
    }
    let t = line.trim();
    if t.is_empty() {
        return false;
    }
    let lower = t.to_lowercase();
    // Keywords that almost never begin prose sentences
    for kw in &[
        "def ", "fn ", "pub fn ", "pub struct ", "pub enum ", "pub trait ",
        "impl ", ">>> ", "... ",
    ] {
        if lower.starts_with(kw) {
            return true;
        }
    }
    // `class Foo:` style (Python class with colon terminator)
    if lower.starts_with("class ") && t.ends_with(':') {
        return true;
    }
    // import / from — only when they look like code (no commas, short line)
    if (lower.starts_with("import ") || lower.starts_with("from "))
        && !t.contains(',')
        && t.split_whitespace().count() <= 5
    {
        return true;
    }
    // let / const / var with assignment
    if (lower.starts_with("let ") || lower.starts_with("const ") || lower.starts_with("var "))
        && t.contains('=')
    {
        return true;
    }
    false
}

/// Infer the language tag from accumulated code block lines.
/// Defaults to "python" when signals are ambiguous.
fn detect_code_language(lines: &[&str]) -> &'static str {
    let mut py = 0i32;
    let mut rs = 0i32;
    for line in lines {
        let t = line.trim();
        let lower = t.to_lowercase();
        if lower.starts_with("def ") || lower.starts_with("class ")
            || lower.contains("self.") || t.ends_with(':')
        {
            py += 1;
        }
        if lower.starts_with("fn ") || lower.starts_with("pub ")
            || lower.starts_with("impl ") || t.contains("->") || t.contains("::")
        {
            rs += 1;
        }
    }
    if rs > py { "rust" } else { "python" }
}

/// Flush a gathered code block into `out`, prefixed with `[code:<lang>]`.
/// No blank line is inserted before the tag — that bonds the block to the prose above.
fn pdf_flush_code_block(block: &mut Vec<&str>, out: &mut Vec<String>) {
    if block.is_empty() {
        return;
    }
    let lang = detect_code_language(block);
    out.push(format!("[code:{}]", lang));
    for line in block.iter() {
        out.push(line.to_string());
    }
    block.clear();
}

/// Extract the first 5 words of a string as a normalized lowercase prefix.
fn five_word_prefix(s: &str) -> String {
    s.split_whitespace()
        .take(5)
        .map(|w| w.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ")
}

// ━━━ Utility Functions ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}
