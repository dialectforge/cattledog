// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Narration Engine - Left Brain (V3)
//!
//! Converts spreading activation results into natural language.
//! This is Lu's voice - constructed entirely from graph traversal.
//! No LLM. No NLP library. Just activated knowledge stitched together
//! using bond-label connectors.
//!
//! V3 improvements over V2:
//! - Fragment extraction: extracts the most query-relevant clause from a node,
//!   dropping attribution tails and relative clauses when they add noise.
//! - Bond labels fixed: traversal bond labels are now wired to NarrationFacts.
//! - Meaningful connectors: bond category determines connector phrase.
//!   Unknown/user-defined bonds get no connector (period, fresh start).
//! - Query-aware opening: intent shapes the lead sentence directly.
//! - Content-word deduplication: 60% threshold on non-stop-word overlap.
//! - Coherence check: if < 2 query keywords appear in response, fall back
//!   to the single best fact.

use std::collections::{HashMap, HashSet};
use crate::connectors::ConnectorMap;
use crate::QueryIntent;

/// Minimum cosine similarity between query pattern and fact pattern
/// for a fact to be considered relevant. Below this = filtered out.
const RELEVANCE_THRESHOLD: f32 = 0.15;

/// A single fact extracted from an activated node, ready for narration.
#[derive(Debug, Clone)]
struct NarrationFact {
    text: String,
    #[allow(dead_code)]
    strength: f32,
    bond_label: String,
    #[allow(dead_code)]
    distance: u32,
    /// Cosine similarity to query pattern (0.0 - 1.0). Higher = more relevant.
    relevance: f32,
    /// Generality of this fact: 0=definitional, 1=general/category, 2=specific/example.
    /// Used to sort facts general-first for teaching-style narration.
    generality_rank: u8,
    /// Hex ID of the source node — survives the generality re-sort so selected_hexes
    /// in NarrationResult reflects the final post-sort order.
    hex: String,
}

/// Result of the narration process.
#[derive(Debug, Clone)]
pub struct NarrationResult {
    pub text: String,
    pub facts_used: usize,
    pub hub: String,
    pub confidence: NarrationConfidence,
    /// Image URLs found in activated knowledge during narration.
    pub image_urls: Vec<NarrationImage>,
    /// Hex IDs of the nodes narration actually selected, in narration order.
    /// Used by ask() to surface the correct sources (narration re-ranks nodes;
    /// the original recall order differs from what narration uses).
    pub selected_hexes: Vec<String>,
}

/// An image reference surfaced during narration.
#[derive(Debug, Clone)]
pub struct NarrationImage {
    pub url: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NarrationConfidence {
    Strong,
    Moderate,
    Weak,
    Empty,
}

/// The narration engine - Left Brain's speech center.
pub struct NarrationEngine {
    connectors: ConnectorMap,
    min_strength: f32,
    max_facts: usize,
}

impl NarrationEngine {
    pub fn new() -> Self {
        Self {
            connectors: ConnectorMap::new(),
            min_strength: 10.0,
            max_facts: 4,
        }
    }

    /// The core narration function (V3 - fragment extraction + bond connectors).
    pub fn narrate(
        &mut self,
        activated_nodes: &[(String, f32, String)],
        traversal_path: &[(String, String, String, f32, u32)],
        bond_labels: &HashMap<String, String>,
        _node_labels: &HashMap<String, String>,
        query: &str,
        intent: &QueryIntent,
        query_pattern: Option<&[f32]>,
        node_patterns: &HashMap<String, Vec<f32>>,
        node_bond_counts: &HashMap<String, usize>,
        conversation_context: &[String],
    ) -> NarrationResult {
        self.connectors.reset_recent();

        if activated_nodes.is_empty() {
            return NarrationResult {
                text: "I don't know about that yet.".to_string(),
                facts_used: 0,
                hub: String::new(),
                confidence: NarrationConfidence::Empty,
                image_urls: Vec::new(),
                selected_hexes: Vec::new(),
            };
        }

        // Step 1: Extract query words for keyword matching
        let query_words: HashSet<String> = query.to_lowercase()
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|w| !w.is_empty() && !Self::is_stop_word(w))
            .collect();

        // Step 2: Score ALL activated nodes by relevance to query
        let mut scored: Vec<(usize, f32, f32)> = Vec::new();
        for (i, (label, strength, hex)) in activated_nodes.iter().enumerate() {
            if *strength < self.min_strength { continue; }
            if label.split_whitespace().count() <= 2 { continue; }
            // Skip raw training-data artifacts ("Q: ... A: ...", "User: ... Assistant: ...")
            if Self::is_raw_artifact(label) { continue; }
            let mut rel = Self::compute_relevance(
                label, hex, &query_words, query_pattern, node_patterns
            );
            let label_lower: String = label.to_lowercase()
                .chars().map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' })
                .collect();
            let keyword_hits = query_words.iter()
                .filter(|w| w.len() > 2 && label_lower.contains(w.as_str()))
                .count();
            if keyword_hits > 0 {
                let keyword_ratio = keyword_hits as f32 / query_words.len().max(1) as f32;
                let boost = 0.6 * keyword_ratio;
                rel = (rel + boost).min(1.0);
                rel = rel.max(0.4 * keyword_ratio);
            }
            // Hub penalty: nodes with many bonds are traversal connectors
            let bond_count = node_bond_counts.get(hex).copied().unwrap_or(0);
            if bond_count > 500 && keyword_hits == 0 {
                continue;
            }
            if bond_count > 100 {
                let penalty = 0.15 * ((bond_count as f32 / 100.0).ln().max(0.0));
                rel = (rel - penalty).max(0.0);
            }

            // Conversation context boost
            if !conversation_context.is_empty() {
                let conv_keywords: HashSet<String> = conversation_context.iter()
                    .flat_map(|q| {
                        q.to_lowercase()
                            .split_whitespace()
                            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
                            .filter(|w| w.len() > 2 && !Self::is_stop_word(w))
                            .collect::<Vec<_>>()
                    })
                    .collect();
                if !conv_keywords.is_empty() {
                    let conv_hits = conv_keywords.iter()
                        .filter(|w| label_lower.contains(w.as_str()))
                        .count();
                    if conv_hits > 0 {
                        let conv_ratio = conv_hits as f32 / conv_keywords.len() as f32;
                        let conv_boost = 0.28 * conv_ratio;
                        rel = (rel + conv_boost).min(1.0);
                        rel = rel.max(0.3 * conv_ratio);
                    }
                }
            }

            // Intent-aware scoring
            match intent {
                QueryIntent::Definition { .. } => {
                    if label_lower.contains(" is a ") || label_lower.contains(" is the ")
                        || label_lower.contains(" is an ") || label_lower.contains(" means ")
                        || label_lower.contains(" refers to ") || label_lower.contains(" defined as ")
                    {
                        // Only boost if this is a genuine definition, not a contrastive statement.
                        // "This is the opposite of the Big Bang" contains " is the " but describes
                        // a contrast, not a definition of the query topic.
                        if !Self::is_contrastive_for(&label_lower, &query_words) {
                            rel = (rel + 0.2).min(1.0);
                        } else {
                            // Penalise: contrasting nodes should rank below real definitions.
                            rel = (rel - 0.25).max(0.0);
                        }
                    }
                }
                QueryIntent::Process { .. } => {
                    if label_lower.contains(" by ") || label_lower.contains(" through ")
                        || label_lower.contains(" step") || label_lower.contains(" process")
                        || label_lower.contains(" using ") || label_lower.contains(" works by")
                    {
                        rel = (rel + 0.15).min(1.0);
                    }
                }
                QueryIntent::Why { .. } => {
                    if label_lower.contains(" because ") || label_lower.contains(" causes ")
                        || label_lower.contains(" due to ") || label_lower.contains(" results in")
                        || label_lower.contains(" reason ") || label_lower.contains(" leads to")
                    {
                        rel = (rel + 0.15).min(1.0);
                    }
                }
                _ => {}
            }
            scored.push((i, rel, *strength));
        }

        // Sort by relevance first, then strength as tiebreaker
        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
                .then(b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal))
        });

        let hub_idx = match scored.first() {
            Some((idx, _rel, _str)) => *idx,
            None => return NarrationResult {
                text: "I don't know about that yet.".to_string(),
                facts_used: 0,
                hub: String::new(),
                confidence: NarrationConfidence::Empty,
                image_urls: Vec::new(),
                selected_hexes: Vec::new(),
            },
        };
        let (hub_label, _hub_strength, _hub_hex) = &activated_nodes[hub_idx];

        // Step 3: Build node → bond_label map from traversal
        // Each node's "incoming" bond is the one used to reach it during spreading activation.
        let mut node_to_bond: HashMap<String, String> = HashMap::new();
        for (_from_hex, bond_hex, to_hex, _str, _hop) in traversal_path.iter() {
            if let Some(blabel) = bond_labels.get(bond_hex) {
                node_to_bond.entry(to_hex.clone())
                    .or_insert_with(|| blabel.clone());
            }
        }

        // Step 4: Gather relevant facts with bond labels populated
        let mut facts: Vec<NarrationFact> = Vec::new();
        let mut seen_content: HashSet<String> = HashSet::new();

        for &(idx, relevance, _strength) in scored.iter() {
            let (label, strength, hex) = &activated_nodes[idx];

            // Filter irrelevant facts (always allow the hub/first)
            if relevance < RELEVANCE_THRESHOLD && !facts.is_empty() {
                continue;
            }

            let normalized = label.to_lowercase();
            if seen_content.contains(&normalized) { continue; }
            seen_content.insert(normalized);

            // Retrieve the bond label that connected this node during traversal
            let bond_label = node_to_bond.get(hex).cloned().unwrap_or_default();
            let bond_count = node_bond_counts.get(hex).copied().unwrap_or(0);
            let generality_rank = Self::compute_generality_rank(label, bond_count);

            facts.push(NarrationFact {
                text: Self::clean_fact(label),
                strength: *strength,
                bond_label,
                distance: 0,
                relevance,
                generality_rank,
                hex: hex.clone(),
            });
            if facts.len() >= self.max_facts { break; }
        }

        // Step 5a: Hierarchical re-sort for teaching intents.
        // Put definitional/general facts first so the opening sentence anchors the explanation.
        // Only applies to intents where we want a structured explanation.
        match intent {
            QueryIntent::Definition { .. } | QueryIntent::General { .. } | QueryIntent::Explore { .. } => {
                facts.sort_by(|a, b| {
                    a.generality_rank.cmp(&b.generality_rank)
                        .then(b.relevance.partial_cmp(&a.relevance).unwrap_or(std::cmp::Ordering::Equal))
                });
            }
            _ => {}
        }

        // Step 5: Compute confidence
        let avg_relevance = if facts.is_empty() {
            0.0
        } else {
            facts.iter().map(|f| f.relevance).sum::<f32>() / facts.len() as f32
        };

        // Keyword coverage among selected facts: if most facts contain query
        // keywords, the recall is topically on-target even if individual relevance
        // scores are spread thin across many correct nodes (e.g., "gradient descent"
        // finds 4 correct source nodes each contributing ~3%).
        let facts_with_keyword_hits = facts.iter()
            .filter(|f| {
                let f_lower = f.text.to_lowercase();
                query_words.iter().any(|kw| kw.len() > 2 && f_lower.contains(kw.as_str()))
            })
            .count();
        let fact_keyword_ratio = facts_with_keyword_hits as f32 / facts.len().max(1) as f32;

        let confidence = if facts.len() >= 3 && avg_relevance > 0.4 {
            NarrationConfidence::Strong
        } else if facts.len() >= 3 && fact_keyword_ratio >= 0.6 && avg_relevance > 0.15 {
            // Many facts with keyword hits — content is topically relevant even
            // though individual relevance scores are diluted across multiple nodes.
            NarrationConfidence::Strong
        } else if facts.len() >= 2 && avg_relevance > 0.25 {
            NarrationConfidence::Moderate
        } else if facts.len() >= 2 && fact_keyword_ratio >= 0.5 && avg_relevance > 0.1 {
            NarrationConfidence::Moderate
        } else if !facts.is_empty() {
            NarrationConfidence::Weak
        } else {
            NarrationConfidence::Empty
        };

        if facts.is_empty() {
            return NarrationResult {
                text: "I don't know about that yet.".to_string(),
                facts_used: 0,
                hub: hub_label.clone(),
                confidence: NarrationConfidence::Empty,
                image_urls: Vec::new(),
                selected_hexes: Vec::new(),
            };
        }

        // Extract image URLs from activated nodes
        let mut image_urls: Vec<NarrationImage> = Vec::new();
        for (label, _strength, _hex) in activated_nodes.iter() {
            if let Some(rest) = label.strip_prefix("[Image: ") {
                if let Some(bracket_end) = rest.find(']') {
                    let url = rest[..bracket_end].trim().to_string();
                    let desc = rest[bracket_end + 1..].trim().to_string();
                    if !url.is_empty() {
                        image_urls.push(NarrationImage { url, description: desc });
                    }
                }
            }
        }

        let selected_hexes: Vec<String> = facts.iter().map(|f| f.hex.clone()).collect();
        let text = self.compose(&facts, intent, query, &query_words);

        NarrationResult {
            text,
            facts_used: facts.len(),
            hub: hub_label.clone(),
            confidence,
            image_urls,
            selected_hexes,
        }
    }

    /// Compose facts into natural language.
    ///
    /// Structure: lead sentence (query-intent-aware) + supporting sentences
    /// (connected by bond-type-derived connectors, or standalone if bond unknown).
    fn compose(
        &mut self,
        facts: &[NarrationFact],
        intent: &QueryIntent,
        query: &str,
        query_words: &HashSet<String>,
    ) -> String {
        if facts.is_empty() {
            return "I don't know about that yet.".to_string();
        }

        // UTF-8–safe truncation helper for debug strings (allocates, debug-only).
        let trunc = |s: &str, n: usize| -> String {
            s.chars().take(n).collect()
        };

        // Short-circuit fixed-response intents
        match intent {
            QueryIntent::Greeting => return "Hello! Ask me something.".to_string(),
            QueryIntent::MetaSelf => return "I'm Lu — an associative memory. I learn facts as nodes and connect them with bonds. No LLM, just a graph that thinks.".to_string(),
            _ => {}
        }

        let mut debug_lines: Vec<String> = Vec::new();
        debug_lines.push(format!(
            "[narrate] query_type={} facts_in={} query_words={:?}",
            Self::intent_name(intent), facts.len(),
            query_words.iter().collect::<Vec<_>>()
        ));

        // Step 1: Extract the most query-relevant fragment from each fact.
        // This drops attribution clauses ("developed by...", "first described in...")
        // and relative clauses that aren't relevant to the query.
        let fragments: Vec<(String, &str)> = facts.iter()
            .map(|f| {
                let frag = Self::extract_fragment(&f.text, query_words);
                debug_lines.push(format!(
                    "  [fragment] bond={:?} original={:?} → extracted={:?}",
                    trunc(&f.bond_label, 30),
                    trunc(&f.text, 60),
                    trunc(&frag, 60)
                ));
                (frag, f.bond_label.as_str())
            })
            .collect();

        // Step 2: Deduplicate by content-word overlap (> 60% = redundant, keep first).
        let mut deduped: Vec<(String, &str)> = Vec::new();
        'outer: for (frag, bond) in fragments {
            for (existing, _) in &deduped {
                if Self::content_word_overlap(&frag, existing) > 0.6 {
                    debug_lines.push(format!(
                        "  [dedup] dropped {:?} (overlaps {:?})",
                        trunc(&frag, 40),
                        trunc(existing, 40)
                    ));
                    continue 'outer;
                }
            }
            deduped.push((frag, bond));
        }

        if deduped.is_empty() {
            return Self::ensure_period(&Self::uppercase_first(&facts[0].text));
        }

        // Step 3: Build opening sentence shaped by query intent.
        let (lead_frag, _) = &deduped[0];
        let opening = Self::build_opening(lead_frag, intent, query);
        debug_lines.push(format!("  [compose] opening={:?}", trunc(&opening, 80)));

        // Step 4: Append supporting sentences with meaningful connectors.
        // Bond-type-derived connectors are used for semantic bonds.
        // Unknown/user-defined bonds → fall back to content-based connectors.
        let mut sentences: Vec<String> = vec![opening];
        let lead_frag: &str = &deduped[0].0;
        let mut prev_frag: &str = lead_frag;

        for (i, (frag, bond_label)) in deduped.iter().enumerate().skip(1) {
            // Topic coherence check: if this fragment shares < 15% content words
            // with the lead fragment, the topics are unrelated — skip it rather
            // than joining with a misleading connector like "Related to this,".
            let topic_overlap = Self::content_word_overlap(frag, lead_frag);
            if topic_overlap < 0.15 {
                debug_lines.push(format!(
                    "  [compose] [{i}] skipped (topic_overlap={:.2} with lead, frag={:?})",
                    topic_overlap,
                    trunc(frag, 60)
                ));
                continue;
            }

            // Try bond-label connector first; if it gives nothing, use content analysis.
            let bond_connector = Self::connector_for_bond(bond_label, &mut self.connectors);
            let connector = if bond_connector.is_empty() {
                Self::connector_for_content(frag, prev_frag, i)
            } else {
                bond_connector
            };
            debug_lines.push(format!(
                "  [compose] [{i}] bond={:?} connector={:?} frag={:?}",
                trunc(bond_label, 30),
                connector,
                trunc(frag, 60)
            ));

            let sentence = if connector.is_empty() {
                // No connector: fact starts fresh as its own sentence
                Self::uppercase_first(frag)
            } else if connector.ends_with(',') {
                // "Related to this, [lowercase fact]"
                format!("{} {}", connector, Self::lowercase_first(frag))
            } else {
                // "One type is [lowercase fact]"
                format!("{} {}", connector, Self::lowercase_first(frag))
            };
            sentences.push(sentence);
            prev_frag = frag;
        }

        // Step 5: Assemble with proper sentence punctuation.
        let mut response = String::new();
        for (i, sentence) in sentences.iter().enumerate() {
            if i > 0 {
                // Ensure previous content ends with punctuation
                let tail = response.trim_end_matches(' ');
                if !tail.ends_with('.') && !tail.ends_with('!') && !tail.ends_with('?') {
                    let trimmed = tail.to_string();
                    response = trimmed;
                    response.push('.');
                }
                response.push(' ');
            }
            response.push_str(sentence);
        }
        response = Self::ensure_period(&response);

        // Step 6: Coherence check — does the response actually address the query?
        // If fewer than 2 query keywords appear in the response, it's probably
        // off-topic noise from hub traversal. Fall back to single best fact.
        let content_query_words: Vec<&String> = query_words.iter()
            .filter(|w| w.len() > 2)
            .collect();
        if !content_query_words.is_empty() {
            let response_lower = response.to_lowercase();
            let kw_hits = content_query_words.iter()
                .filter(|w| response_lower.contains(w.as_str()))
                .count();
            let threshold = content_query_words.len().min(2);
            if kw_hits < threshold {
                debug_lines.push(format!(
                    "[narrate] coherence_fail: kw_hits={} threshold={} → fallback to single fact",
                    kw_hits, threshold
                ));
                Self::write_debug(&debug_lines);
                return Self::ensure_period(&Self::clean_fact(&facts[0].text));
            }
        }

        // Step 7: Thin-response check — if the explanation is too short for a
        // definition/general question, flag that knowledge is sparse.
        let word_count = response.split_whitespace().count();
        if word_count < 50 {
            match intent {
                QueryIntent::Definition { topic } | QueryIntent::General { topic }
                | QueryIntent::Explore { topic } if !topic.is_empty() => {
                    debug_lines.push(format!(
                        "[narrate] thin_response: {} words for {:?} intent", word_count, Self::intent_name(intent)
                    ));
                    response.push_str(&format!(
                        " (I have limited knowledge about {topic} — consider teaching me more.)"
                    ));
                }
                _ => {}
            }
        }

        Self::write_debug(&debug_lines);
        response
    }

    // ── Fragment extraction ───────────────────────────────────────────────────

    /// Extract the most query-relevant clause from a fact sentence.
    ///
    /// Strategy:
    /// 1. Try relative clause split (", which", ", who", " which", " where")
    ///    → use main clause before the relative pronoun if it captures the relevance
    /// 2. Try comma split → use first clause if it captures ≥75% of keyword relevance
    ///    and is meaningfully shorter than the full sentence
    /// 3. Fall back to full sentence
    fn extract_fragment(text: &str, query_words: &HashSet<String>) -> String {
        let word_count = text.split_whitespace().count();
        // Short sentences are already concise — don't fragment them
        if word_count < 10 || query_words.is_empty() {
            return text.to_string();
        }

        // Definitional nodes must never be truncated — they carry the core meaning.
        // Detect by standard definitional phrasing anywhere in the text.
        // Exception: contrastive statements ("opposite of X", "contrary to X") should
        // NOT be protected — they aren't definitions even if they contain "is the".
        let lower_text = text.to_lowercase();
        let definitional_markers = [
            " is a ", " is an ", " is the ", " is like ", " is when ", " is where ",
            " refers to ", " defined as ", " means ", " are a ", " are the ",
        ];
        if definitional_markers.iter().any(|m| lower_text.contains(m))
            && !Self::is_contrastive_for(&lower_text, query_words)
        {
            return text.to_string();
        }

        let score_clause = |clause: &str| -> usize {
            let lower = clause.to_lowercase();
            query_words.iter()
                .filter(|w| w.len() > 2 && lower.contains(w.as_str()))
                .count()
        };

        // Relative clause split: the clause before the relative pronoun is
        // usually the main fact; the clause after is typically elaboration/attribution.
        for splitter in &[", which ", ", who ", ", where ", " which ", " who ", " where "] {
            if let Some(idx) = text.find(splitter) {
                let main = text[..idx].trim();
                if main.split_whitespace().count() >= 5 {
                    let main_score = score_clause(main);
                    let tail_score = score_clause(&text[idx..]);
                    // Use main clause if it has at least as much relevance as the tail
                    if main_score >= 1 && main_score >= tail_score {
                        return main.to_string();
                    }
                }
            }
        }

        // Comma split: "key fact, attribution, more detail"
        // Use the first clause if it captures most of the query relevance
        // AND the full sentence would be significantly longer.
        let comma_parts: Vec<&str> = text.splitn(4, ',').collect();
        if comma_parts.len() >= 2 {
            let first = comma_parts[0].trim();
            let first_wc = first.split_whitespace().count();
            if first_wc >= 5 {
                let first_score = score_clause(first);
                let full_score = score_clause(text);
                if first_score > 0
                    && full_score > 0
                    && first_score as f32 >= full_score as f32 * 0.75
                    && first_wc < word_count * 3 / 4
                {
                    return first.to_string();
                }
            }
        }

        text.to_string()
    }

    // ── Deduplication ────────────────────────────────────────────────────────

    /// Content-word overlap ratio between two text fragments.
    /// Ignores stop words and very short words (≤ 3 chars).
    /// Used to detect near-duplicate facts before composing.
    fn content_word_overlap(a: &str, b: &str) -> f32 {
        let content_words = |s: &str| -> HashSet<String> {
            s.to_lowercase()
                .split_whitespace()
                .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
                .filter(|w| w.len() > 3 && !Self::is_stop_word(w))
                .collect()
        };
        let wa = content_words(a);
        let wb = content_words(b);
        if wa.is_empty() || wb.is_empty() { return 0.0; }
        let overlap = wa.intersection(&wb).count();
        overlap as f32 / wa.len().min(wb.len()) as f32
    }

    // ── Opening construction ─────────────────────────────────────────────────

    /// Build the opening sentence shaped by query intent.
    fn build_opening(fact: &str, intent: &QueryIntent, query: &str) -> String {
        match intent {
            QueryIntent::Definition { topic } => {
                let fact_lower = fact.to_lowercase();
                // If the fact already IS a definition, use it directly.
                if fact_lower.contains(" is a ") || fact_lower.contains(" is the ")
                    || fact_lower.contains(" is an ") || fact_lower.contains(" are a ")
                    || fact_lower.contains(" means ") || fact_lower.contains(" refers to ")
                    || fact_lower.contains(" defined as ")
                {
                    return Self::uppercase_first(fact);
                }
                // Try to frame a short label using the topic.
                if !topic.is_empty() {
                    let topic_lower = topic.to_lowercase();
                    // Pattern: "[topic] [component]" with only 1-3 trailing words
                    // e.g., "Neural network layers" (topic="neural network") → "A neural network has layers."
                    if fact_lower.starts_with(&topic_lower) {
                        let remainder = fact[topic_lower.len()..].trim();
                        let rem_words = remainder.split_whitespace().count();
                        if rem_words >= 1 && rem_words <= 3 && !remainder.is_empty()
                            && !remainder.starts_with("is") && !remainder.starts_with("are")
                        {
                            return format!("A {} has {}.", topic_lower, remainder.to_lowercase());
                        }
                    }
                    // Pattern: fact mentions topic but isn't a full definition.
                    // Use as-is — it's at least coherent.
                    if fact_lower.contains(&topic_lower) {
                        return Self::uppercase_first(fact);
                    }
                    // Fact doesn't mention topic at all — add a framing prefix.
                    if fact.split_whitespace().count() <= 5 {
                        return format!("{}: {}.", Self::uppercase_first(topic), Self::lowercase_first(fact));
                    }
                }
                Self::uppercase_first(fact)
            }

            QueryIntent::Process { topic } => {
                // "How does X work?" — frame with mechanism language.
                let fact_lower = fact.to_lowercase();
                if fact_lower.contains("works by") || fact_lower.contains("works through")
                    || fact_lower.contains("by ") || fact_lower.contains("through ")
                    || fact_lower.contains("step") || fact_lower.contains("process")
                {
                    return Self::uppercase_first(fact);
                }
                if !topic.is_empty() && !fact_lower.contains(&topic.to_lowercase()) {
                    return format!("{} involves {}.",
                        Self::uppercase_first(topic),
                        Self::lowercase_first(fact));
                }
                Self::uppercase_first(fact)
            }

            QueryIntent::General { topic } | QueryIntent::Explore { topic } => {
                // "Tell me about X" — lead with the topic if the fact doesn't.
                let fact_lower = fact.to_lowercase();
                if !topic.is_empty() && !fact_lower.contains(&topic.to_lowercase())
                    && fact.split_whitespace().count() <= 5
                {
                    return format!("{}: {}.", Self::uppercase_first(topic), Self::lowercase_first(fact));
                }
                Self::uppercase_first(fact)
            }

            QueryIntent::Who { .. } => Self::uppercase_first(fact),

            QueryIntent::Why { topic } => {
                let lower = fact.to_lowercase();
                // If the fact is already causal, use it directly
                if lower.contains("because") || lower.contains("due to")
                    || lower.contains("since ") || lower.contains("causes ")
                    || lower.contains("results in") || lower.contains("leads to")
                {
                    Self::uppercase_first(fact)
                } else if !topic.is_empty() {
                    // Frame it as a causal answer
                    format!("{} because {}",
                        Self::uppercase_first(topic),
                        Self::lowercase_first(fact))
                } else {
                    Self::uppercase_first(fact)
                }
            }

            QueryIntent::YesNo { .. } => {
                let lower_fact = fact.to_lowercase();
                let lower_query = query.to_lowercase();
                let qwords: Vec<&str> = lower_query.split_whitespace()
                    .filter(|w| !Self::is_stop_word(w) && w.len() > 2)
                    .collect();
                let hits = qwords.iter().filter(|w| lower_fact.contains(**w)).count();
                if hits >= qwords.len().saturating_sub(1).max(1) {
                    format!("Yes. {}", Self::uppercase_first(fact))
                } else {
                    Self::uppercase_first(fact)
                }
            }

            QueryIntent::Comparison { topic_a, topic_b } => {
                if !topic_a.is_empty() && !topic_b.is_empty() {
                    format!("Comparing {} and {}: {}",
                        topic_a, topic_b,
                        Self::lowercase_first(fact))
                } else {
                    Self::uppercase_first(fact)
                }
            }

            QueryIntent::List { topic, .. } => {
                if !topic.is_empty() {
                    format!("About {}: {}",
                        topic,
                        Self::lowercase_first(fact))
                } else {
                    Self::uppercase_first(fact)
                }
            }

            QueryIntent::Think { .. } => {
                format!("I think {}", Self::lowercase_first(fact))
            }

            QueryIntent::Ability { topic } => {
                format!("I know about {}. {}",
                    topic,
                    Self::uppercase_first(fact))
            }

            // These are handled before compose() is called
            QueryIntent::Greeting | QueryIntent::MetaSelf => Self::uppercase_first(fact),
        }
    }

    // ── Connector selection ──────────────────────────────────────────────────

    /// Select a connector phrase for the given bond label.
    ///
    /// Semantic bond types (reindex, self-derived, rumination, auto-association,
    /// self-association) get a meaningful transitional phrase from ConnectorMap.
    ///
    /// Unknown or user-defined bonds return "" — the caller will start a fresh
    /// sentence instead. No connector is better than a fake one.
    fn connector_for_bond(bond_label: &str, connectors: &mut ConnectorMap) -> &'static str {
        if bond_label.is_empty() {
            return "";
        }
        let category = ConnectorMap::classify_label(bond_label);
        match category {
            // Non-semantic bonds: just use a period, start fresh
            "unknown" | "user-defined" => "",
            // Semantic bonds: use the connector map
            _ => connectors.get_connector(bond_label),
        }
    }

    /// Content-based connector when bond label provides no semantic connector.
    /// Analyzes the current fact's content to pick appropriate transitional language:
    /// - Named type with abbreviation (e.g., "RNNs") → "One type is"
    /// - Historical/dated fact ("introduced in 2016") → "For example,"
    /// - More detailed than previous fact → "More specifically,"
    /// - Default → "Additionally," / "Also,"
    fn connector_for_content(frag: &str, prev_frag: &str, position: usize) -> &'static str {
        let lower = frag.to_lowercase();

        // Named type with short abbreviation in parens: "Recurrent neural networks (RNNs)"
        let has_abbrev = if let Some(start) = frag.find('(') {
            if let Some(end) = frag[start..].find(')') {
                let paren_inner = &frag[start+1..start+end];
                paren_inner.len() <= 8 && !paren_inner.contains(' ')
            } else { false }
        } else { false };

        // Historical/dated: "was introduced in 19xx" or "developed in 20xx"
        let has_date = (lower.contains(" in 19") || lower.contains(" in 20"))
            && (lower.contains("introduc") || lower.contains("develop")
                || lower.contains("creat") || lower.contains("publish")
                || lower.contains("present") || lower.contains("propos"));

        if has_abbrev && !has_date {
            return "One type is";
        }
        if has_date {
            return "For example,";
        }

        // More words than previous = more detailed/specific → "More specifically,"
        let prev_wc = prev_frag.split_whitespace().count();
        let curr_wc = lower.split_whitespace().count();
        if curr_wc > prev_wc + 5 {
            return "More specifically,";
        }

        match position {
            1 => "Additionally,",
            _ => "Also,",
        }
    }

    /// Compute how general vs specific a fact is.
    /// 0 = definitional (contains "is a/an/the", "means", "refers to")
    /// 1 = general/category (short label, or many bonds)
    /// 2 = specific/example (longer, dated, detailed)
    fn compute_generality_rank(text: &str, bond_count: usize) -> u8 {
        let lower = text.to_lowercase();
        if lower.contains(" is a ") || lower.contains(" is the ") || lower.contains(" is an ")
            || lower.contains(" are a ") || lower.contains(" are the ")
            || lower.contains(" means ") || lower.contains(" refers to ")
            || lower.contains(" defined as ") || lower.contains(" is defined ")
        {
            return 0;
        }
        let word_count = text.split_whitespace().count();
        // Short labels or highly-connected nodes tend to be category-level
        if word_count <= 4 || bond_count > 15 {
            return 1;
        }
        2 // specific/example
    }

    // ── Relevance computation (unchanged from V2) ────────────────────────────

    fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() { return 0.0; }
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if mag_a < 1e-9 || mag_b < 1e-9 { return 0.0; }
        (dot / (mag_a * mag_b)).clamp(-1.0, 1.0)
    }

    fn compute_relevance(
        label: &str,
        hex: &str,
        query_words: &HashSet<String>,
        query_pattern: Option<&[f32]>,
        node_patterns: &HashMap<String, Vec<f32>>,
    ) -> f32 {
        let pattern_sim = if let Some(qp) = query_pattern {
            if let Some(np) = node_patterns.get(hex) {
                Self::cosine_sim(qp, np) * 0.5
            } else {
                0.0
            }
        } else {
            0.0
        };
        let fact_words: HashSet<String> = label.to_lowercase()
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|w| !w.is_empty() && !Self::is_stop_word(w))
            .collect();
        let keyword_overlap = if fact_words.is_empty() || query_words.is_empty() {
            0.0
        } else {
            let overlap = query_words.intersection(&fact_words).count();
            overlap as f32 / query_words.len().max(1) as f32
        };
        if keyword_overlap > 0.0 {
            (0.5 * keyword_overlap + pattern_sim).max(0.3)
        } else {
            pattern_sim
        }
    }

    // ── String utilities ─────────────────────────────────────────────────────

    fn is_stop_word(word: &str) -> bool {
        matches!(word,
            "a" | "an" | "the" | "is" | "are" | "was" | "were" | "be" | "been" |
            "am" | "do" | "does" | "did" | "have" | "has" | "had" | "will" |
            "would" | "could" | "should" | "may" | "might" | "shall" | "can" |
            "to" | "of" | "in" | "for" | "on" | "with" | "at" | "by" | "from" |
            "it" | "its" | "this" | "that" | "these" | "those" | "i" | "me" |
            "my" | "we" | "our" | "you" | "your" | "he" | "she" | "they" |
            "what" | "where" | "when" | "who" | "how" | "why" | "which" |
            "and" | "or" | "but" | "not" | "if" | "so" | "as" | "about"
        )
    }

    fn clean_fact(text: &str) -> String {
        let trimmed = text.trim();

        // Strip Q:/A: training-data artifacts — extract only the answer part.
        let lower = trimmed.to_lowercase();
        if lower.starts_with("q:") || lower.contains("\nq:") {
            // Look for an "A:" answer section (newline-prefixed or inline)
            for sep in &["\na:", "\nA:", " a:", " A:"] {
                if let Some(pos) = trimmed.find(sep) {
                    let answer = trimmed[pos + sep.len()..].trim();
                    if !answer.is_empty() {
                        return Self::clean_fact(answer);
                    }
                }
            }
        }
        // Strip User:/Assistant: artifacts — extract only the assistant reply.
        if lower.contains("assistant:") && (lower.starts_with("user:") || lower.contains("\nuser:")) {
            for sep in &["Assistant:", "assistant:"] {
                if let Some(pos) = trimmed.find(sep) {
                    let answer = trimmed[pos + sep.len()..].trim();
                    if !answer.is_empty() {
                        return Self::clean_fact(answer);
                    }
                }
            }
        }

        if trimmed.ends_with('.') {
            trimmed[..trimmed.len()-1].trim().to_string()
        } else {
            trimmed.to_string()
        }
    }

    /// Returns true when a node label is an unprocessed training-data artifact
    /// that should be skipped entirely rather than narrated as a fact.
    fn is_raw_artifact(text: &str) -> bool {
        let lower = text.to_lowercase();
        // Q: ... A: style (question-answer pairs)
        if (lower.starts_with("q:") || lower.contains("\nq:")) && lower.contains("a:") {
            return true;
        }
        // User: ... Assistant: style (chat transcripts)
        if (lower.starts_with("user:") || lower.contains("\nuser:")) && lower.contains("assistant:") {
            return true;
        }
        // Python REPL lines
        if lower.starts_with(">>> ") {
            return true;
        }
        false
    }

    /// Returns true when a label describes a contrast/opposition involving any query keyword
    /// rather than being a genuine definition of that keyword.
    ///
    /// Example: "This is the opposite of the Big Bang" for query "big bang" → true
    fn is_contrastive_for(label_lower: &str, query_words: &HashSet<String>) -> bool {
        const CONTRASTIVE: &[&str] = &[
            "opposite of", "contrary to", "unlike", "not to be confused",
            "contrast with", "contrast to", "different from", "as opposed to",
            "in contrast to", "not the same as", "rather than",
        ];
        for marker in CONTRASTIVE {
            if let Some(pos) = label_lower.find(marker) {
                // Check whether any query keyword appears AFTER the contrastive marker —
                // that means the query topic is being contrasted, not defined.
                let after = &label_lower[pos + marker.len()..];
                if query_words.iter().any(|w| w.len() > 2 && after.contains(w.as_str())) {
                    return true;
                }
            }
        }
        false
    }

    fn ensure_period(s: &str) -> String {
        let t = s.trim_end();
        if t.ends_with('.') || t.ends_with('!') || t.ends_with('?') {
            t.to_string()
        } else {
            format!("{}.", t)
        }
    }

    fn lowercase_first(s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(c) => c.to_lowercase().to_string() + chars.as_str(),
        }
    }

    fn uppercase_first(s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(c) => c.to_uppercase().to_string() + chars.as_str(),
        }
    }

    fn intent_name(intent: &QueryIntent) -> &'static str {
        match intent {
            QueryIntent::Definition { .. } => "Definition",
            QueryIntent::Why { .. } => "Why",
            QueryIntent::Process { .. } => "Process",
            QueryIntent::Who { .. } => "Who",
            QueryIntent::YesNo { .. } => "YesNo",
            QueryIntent::Comparison { .. } => "Comparison",
            QueryIntent::List { .. } => "List",
            QueryIntent::Think { .. } => "Think",
            QueryIntent::Explore { .. } => "Explore",
            QueryIntent::General { .. } => "General",
            QueryIntent::Greeting => "Greeting",
            QueryIntent::MetaSelf => "MetaSelf",
            QueryIntent::Ability { .. } => "Ability",
        }
    }

    fn write_debug(lines: &[String]) {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true).append(true)
            .open(r"lu_debug.log")
        {
            for line in lines {
                let _ = writeln!(f, "{}", line);
            }
        }
    }
}
