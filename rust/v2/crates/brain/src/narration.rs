// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Narration engine v2 — multi-KU paragraph composition with StyleVector adaptation.
//!
//! Phase 2 upgrade: instead of surfacing a single KU's raw_text, the engine
//! now *composes* a coherent paragraph by layering all available typed KUs in
//! semantic order:
//!
//!   Definition → Composition → Mechanism → Purpose → Causal → Example → Contrast
//!
//! Cleanup upgrade: `narrate_with_style` uses the brain's StyleVector to tune:
//!   - verbosity  < 0.3 → short responses (fewer KU types, no extras)
//!   - verbosity  > 0.7 → full detail with examples
//!   - formality  < 0.3 → casual connectors ("basically", "think of it like")
//!   - formality  > 0.7 → formal connectors ("specifically", "it is important to note")
//!   - technicality > 0.7 → technical terms and mechanism emphasis
//!   - technicality < 0.3 → simplified language, lead with examples
//!
//! Template renderers delegate to `compose_paragraph` so every response
//! automatically benefits from the richer knowledge extracted by the temporal
//! lobe's Phase 2 patterns.

use graph::knowledge_unit::{KnowledgeUnit, KuKind};
use graph::network::{ActivatedNode, RecallResult};

use crate::frontal::{QueryIntent, ResponsePlan};
use crate::hippocampus::StyleVector;

/// Extract content words from a query (lowercase, >3 chars, no stop words).
fn query_content_words(query: &str) -> Vec<String> {
    const STOP: &[&str] = &[
        "what", "which", "where", "when", "does", "how",
        "that", "this", "with", "from", "have", "been",
        "about", "tell", "make", "more", "many", "each",
        "some", "your", "like", "just", "give", "help",
        "actually", "really", "know", "understand", "explain",
        "interesting", "switching", "gears", "don't",
        "textbook", "answer", "take",
    ];
    query
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .map(|w| w.to_lowercase())
        .filter(|w| w.len() > 3 && !STOP.contains(&w.as_str()))
        .collect()
}

/// Score how relevant a node's text is to the query's content words.
/// Returns 0.0–1.0 where 1.0 means all content words appear in the node text.
///
/// Words are **weighted by their character length** so that long/specific terms
/// (e.g. "neurologically" = 14) outweigh short common ones (e.g. "song" = 4).
/// This naturally elevates domain-specific matches over superficial ones and
/// prevents a node mentioning only "music" + "song" from passing a filter meant
/// to gate out off-topic content on a neuroscience query.
///
/// **Stem/prefix matching** for content words >= 7 chars: a node word is accepted
/// as matching a content word if the content word *starts with* that node word
/// (i.e. the node word is a root/stem of the query word).  This lets a node
/// containing "neural" score full credit for the query word "neurologically",
/// and a node containing "emotion" match "emotional".  Node words must be >= 4
/// chars to prevent trivial matches (e.g. "bra" falsely matching "brain").
fn relevance_score(node_text: &str, content_words: &[String]) -> f32 {
    if content_words.is_empty() {
        return 0.5; // neutral if we can't judge
    }
    let text_lower = node_text.to_lowercase();
    let total_weight: f32 = content_words.iter().map(|w| w.len() as f32).sum();
    if total_weight == 0.0 {
        return 0.0;
    }

    // Tokenise node text once for prefix/stem matching (4+ char words only).
    let node_words: Vec<&str> = text_lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 4)
        .collect();

    let matched_weight: f32 = content_words
        .iter()
        .filter(|w| {
            // Direct substring match (original behaviour).
            if text_lower.contains(w.as_str()) {
                return true;
            }
            // Stem match for long query words (>= 7 chars): accept if any 4+
            // char node word is a prefix of the content word.
            // "neural" (node) matches "neurologically" (query) because
            //   "neurologically".starts_with("neural") == true.
            if w.len() >= 7 {
                return node_words.iter().any(|nw| nw.len() >= 4 && w.starts_with(*nw));
            }
            false
        })
        .map(|w| w.len() as f32)
        .sum();
    matched_weight / total_weight
}

/// Extract the best sentence(s) from a text block that are most relevant to the query.
/// This prevents dumping an entire paragraph when only one sentence answers the question.
fn extract_best_sentences(text: &str, content_words: &[String], max_sentences: usize) -> String {
    // Split on sentence boundaries
    let sentences: Vec<&str> = text
        .split(|c: char| c == '.' || c == '!' || c == '?')
        .map(|s| s.trim())
        .filter(|s| s.len() > 15) // skip very short fragments
        .collect();

    if sentences.len() <= max_sentences {
        return text.to_string();
    }

    // Score each sentence by COUNT-based content word overlap (not weighted).
    // Using count/total here (not length-weighted) ensures that a sentence with
    // TWO short content word matches beats a sentence with ONE long-word match.
    // This prevents high-weight rare words (e.g. "emotional" weight 9) from
    // promoting an off-topic sentence above a topically richer one.
    let score_sentence = |s: &str| -> f32 {
        if content_words.is_empty() {
            return 0.0;
        }
        let sl = s.to_lowercase();
        let matched = content_words.iter().filter(|w| sl.contains(w.as_str())).count();
        matched as f32 / content_words.len() as f32
    };

    let mut scored: Vec<(f32, &str)> = sentences
        .iter()
        .map(|s| (score_sentence(s), *s))
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // If even the best sentence has zero content-word matches, return the full
    // text — extracting a zero-match sentence would just pick arbitrarily.
    if scored.first().map_or(true, |(s, _)| *s == 0.0) {
        return text.to_string();
    }

    // Take top sentences, maintaining original order
    let top: Vec<&str> = scored.iter().take(max_sentences).map(|(_, s)| *s).collect();
    let ordered: Vec<&str> = sentences
        .iter()
        .copied()
        .filter(|s| top.contains(s))
        .collect();

    if ordered.is_empty() {
        return text.to_string();
    }

    ordered.iter().map(|s| format!("{}.", s)).collect::<Vec<_>>().join(" ")
}

// ── ResponseTemplate ──────────────────────────────────────────────────────────

/// One template per QueryIntent variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseTemplate {
    /// "X is a Y. [composition] [mechanism] [examples]"
    Definition,
    /// "Step 1: … Step 2: …"
    StepByStep,
    /// "X vs Y: on [dimension], X is … while Y is …"
    Contrast,
    /// "Because [cause], [effect] [under condition]."
    Causal,
    /// "[Subject] is [role]. [purpose/context]."
    WhoIs,
    /// Free-form creative render
    Creative,
    /// General fallback — compose from all available KU kinds
    Fallback,
}

impl ResponseTemplate {
    pub fn for_intent(intent: &QueryIntent) -> Self {
        match intent {
            QueryIntent::Definition { .. }  => ResponseTemplate::Definition,
            QueryIntent::Mechanism { .. }   => ResponseTemplate::StepByStep,
            QueryIntent::Procedure { .. }   => ResponseTemplate::StepByStep,
            QueryIntent::Comparison { .. }  => ResponseTemplate::Contrast,
            QueryIntent::Causal { .. }      => ResponseTemplate::Causal,
            QueryIntent::Who { .. }         => ResponseTemplate::WhoIs,
            QueryIntent::Creative { .. }    => ResponseTemplate::Creative,
            QueryIntent::OpenEnded { .. }   => ResponseTemplate::Fallback,
            // Complex and MultiPart queries are handled by the thinking loop;
            // if narration is called directly (e.g. for a sub-question) use Fallback.
            QueryIntent::Complex { .. } | QueryIntent::MultiPart { .. } => ResponseTemplate::Fallback,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ResponseTemplate::Definition  => "definition",
            ResponseTemplate::StepByStep  => "step_by_step",
            ResponseTemplate::Contrast    => "contrast",
            ResponseTemplate::Causal      => "causal",
            ResponseTemplate::WhoIs       => "who_is",
            ResponseTemplate::Creative    => "creative",
            ResponseTemplate::Fallback    => "fallback",
        }
    }
}

// ── StyleHints ────────────────────────────────────────────────────────────────

/// Derived from a StyleVector; controls composition decisions.
#[derive(Debug, Clone)]
pub struct StyleHints {
    /// How many KU layers to include (1 = very terse, 7 = full).
    pub max_ku_layers: usize,
    /// Max examples to include (0–3).
    pub max_examples: usize,
    /// Prefix to use for casual tone ("basically", "think of it like …", "").
    pub casual_prefix: Option<&'static str>,
    /// Whether to insert a formality opener like "Specifically, " or "It is important to note".
    pub formal_opener: bool,
    /// Whether to emphasise mechanism KUs over definition KUs (high technicality).
    pub emphasise_mechanism: bool,
}

impl StyleHints {
    pub fn from_style(style: &StyleVector) -> Self {
        // Verbosity drives how many KU layers we include.
        let max_ku_layers = if style.verbosity < 0.3 {
            2 // terse: definition + one supporting KU
        } else if style.verbosity > 0.7 {
            7 // verbose: all layers
        } else {
            4 // moderate: definition, composition/mechanism, purpose, 1 example
        };

        // Verbosity also controls how many examples appear.
        let max_examples = if style.verbosity < 0.3 {
            0
        } else if style.verbosity > 0.7 {
            3
        } else {
            1
        };

        // Formality selects connectors.
        // Only apply casual/formal framing once we have enough samples to trust the
        // style estimate (prevents cold-start formality=0.0 from triggering "basically,"
        // on every answer before any conversation has happened).
        let casual_prefix: Option<&'static str> = if style.formality < 0.3 && style.sample_count >= 5 {
            Some("basically")
        } else {
            None
        };
        let formal_opener = style.formality > 0.7 && style.sample_count >= 5;

        // High technicality → lead with mechanism, use technical phrasing.
        let emphasise_mechanism = style.technicality > 0.7;

        StyleHints {
            max_ku_layers,
            max_examples,
            casual_prefix,
            formal_opener,
            emphasise_mechanism,
        }
    }

    /// Default neutral hints (no style adaptation).
    pub fn neutral() -> Self {
        StyleHints {
            max_ku_layers: 7,
            max_examples: 2,
            casual_prefix: None,
            formal_opener: false,
            emphasise_mechanism: false,
        }
    }
}

// ── NarrationEngine ───────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct NarrationEngine {
    pub max_tokens: usize,
    pub generation_count: u64,
    pub last_template_used: Option<String>,
}

impl Default for NarrationEngine {
    fn default() -> Self {
        NarrationEngine::new(2048)
    }
}

impl NarrationEngine {
    /// Returns true if the text looks like a training-data artifact or low-quality
    /// scaffold text that should never appear in a generated response.
    fn is_training_artifact(text: &str) -> bool {
        let t = text.trim();
        // Reject Q:/A: style artifacts (at start or embedded).
        let tl: String = t.chars().take(40).collect::<String>().to_lowercase();
        if tl.starts_with("q:") || tl.starts_with("a:") { return true; }
        if tl.starts_with("task:") || tl.starts_with("solution:") { return true; }
        if tl.starts_with("user:") || tl.starts_with("assistant:") { return true; }
        if t.contains(" Q:") || t.contains("\nQ:") { return true; }
        // Also catch lowercase embedded q: (some training dumps use lowercase).
        // Note: `tl` only covers first 40 chars, so check the full text via to_lowercase.
        let t_full_lower = t.to_lowercase();
        if t_full_lower.contains(" q:") || t_full_lower.contains("\nq:") { return true; }
        // Reject numbered chapter/section titles like "1 What Is the Internet"
        // (digit, space, uppercase letter).
        let t_chars: Vec<char> = t.chars().take(4).collect();
        if t_chars.len() >= 3
            && t_chars[0].is_ascii_digit()
            && t_chars[1] == ' '
            && t_chars[2].is_uppercase()
        {
            return true;
        }
        // Reject very short texts (< 25 chars) — these are fragments, not informative answers.
        if t.chars().count() < 25 { return true; }
        false
    }

    pub fn new(max_tokens: usize) -> Self {
        NarrationEngine {
            max_tokens,
            generation_count: 0,
            last_template_used: None,
        }
    }

    /// Generate a response from a plan and a recall result (neutral style).
    pub fn narrate(&mut self, plan: &ResponsePlan, recall: &RecallResult, query: &str) -> String {
        self.narrate_with_style(plan, recall, &StyleVector::default(), query)
    }

    /// Generate a response, applying the given StyleVector to tune output.
    pub fn narrate_with_style(
        &mut self,
        plan: &ResponsePlan,
        recall: &RecallResult,
        style: &StyleVector,
        query: &str,
    ) -> String {
        let template = ResponseTemplate::for_intent(&plan.intent);
        self.last_template_used = Some(template.label().to_string());
        self.generation_count += 1;

        let hints = StyleHints::from_style(style);
        let content_words = query_content_words(query);

        // Pre-filter nodes using length-weighted relevance score (with stem
        // matching — see `relevance_score`).
        //
        // Threshold 0.12: low enough to pass partially-relevant nodes (e.g. a
        // node about "music + brain" scores ~0.185 on a long neuroscience query)
        // while still blocking wholly off-topic nodes that only share one generic
        // word like "song" with the query (score ~0.09 or below).
        //
        // Strength floor: among the highest-activation nodes, include up to 3
        // that score >= 0.08 on relevance — they are likely on-topic but may
        // have missed the 0.12 cut because the query is long (big denominator).
        // The 0.08 guard prevents completely off-topic hub nodes (e.g. a "violin"
        // node that only matches "song") from sneaking in via high strength alone.
        let top_strength_seeds: std::collections::HashSet<String> = {
            let mut by_strength = recall.activated_nodes.clone();
            by_strength.sort_by(|a, b| {
                b.strength.partial_cmp(&a.strength).unwrap_or(std::cmp::Ordering::Equal)
            });
            by_strength
                .into_iter()
                .filter(|n| {
                    content_words.is_empty()
                        || relevance_score(n.knowledge_unit.raw_text(), &content_words) >= 0.08
                })
                .take(3)
                .map(|n| n.seed.to_hex())
                .collect()
        };

        let mut filtered_nodes: Vec<ActivatedNode> = recall.activated_nodes
            .iter()
            .filter(|n| {
                let score = relevance_score(n.knowledge_unit.raw_text(), &content_words);
                score >= 0.12 || top_strength_seeds.contains(&n.seed.to_hex())
            })
            .cloned()
            .collect();

        // Sort filtered nodes by descending relevance so downstream composition
        // picks the most on-topic KUs first.
        if !content_words.is_empty() {
            filtered_nodes.sort_by(|a, b| {
                let sa = relevance_score(a.knowledge_unit.raw_text(), &content_words);
                let sb = relevance_score(b.knowledge_unit.raw_text(), &content_words);
                sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        // If the filter removed everything, take only the top-5 nodes by weighted
        // score rather than dumping all 20 — this limits the damage from poor recall
        // while still giving compose_paragraph_hints something to gate against.
        let nodes_for_narration: Vec<ActivatedNode> = if filtered_nodes.is_empty() {
            let mut all = recall.activated_nodes.clone();
            if !content_words.is_empty() {
                all.sort_by(|a, b| {
                    let sa = relevance_score(a.knowledge_unit.raw_text(), &content_words);
                    let sb = relevance_score(b.knowledge_unit.raw_text(), &content_words);
                    sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            all.into_iter().take(5).collect()
        } else {
            filtered_nodes
        };

        let response = self.render_with_hints(&template, plan, &nodes_for_narration, &hints, &content_words);

        // Truncate to token budget (rough: 1 token ≈ 4 chars)
        let char_limit = self.max_tokens * 4;
        if response.len() > char_limit {
            let truncated = &response[..char_limit];
            format!("{}…", truncated.trim_end())
        } else {
            response
        }
    }

    fn render_with_hints(
        &self,
        template: &ResponseTemplate,
        plan: &ResponsePlan,
        nodes: &[ActivatedNode],
        hints: &StyleHints,
        content_words: &[String],
    ) -> String {
        // Filter nodes to required KU kinds; fall back to all recalled nodes if none match.
        let typed: Vec<&ActivatedNode> = nodes
            .iter()
            .filter(|n| plan.required_ku_kinds.contains(&n.knowledge_unit.kind()))
            .collect();
        let relevant: Vec<&ActivatedNode> = if typed.is_empty() {
            nodes.iter().collect()
        } else {
            typed
        };

        if relevant.is_empty() {
            let all: Vec<&ActivatedNode> = nodes.iter().collect();
            if all.is_empty() {
                return self.no_knowledge_response(plan.intent.topic());
            }
            return self.compose_paragraph_hints(&all, hints, content_words);
        }

        let raw = match template {
            // Definition, WhoIs, Creative delegate directly to compose_paragraph_hints
            // with style hints so the result is already style-adapted.
            ResponseTemplate::Definition | ResponseTemplate::WhoIs | ResponseTemplate::Creative => {
                let all: Vec<&ActivatedNode> = nodes.iter().collect();
                if all.is_empty() {
                    relevant.first()
                        .map(|n| n.knowledge_unit.raw_text().to_string())
                        .unwrap_or_else(|| "No information found.".to_string())
                } else {
                    self.compose_paragraph_hints(&all, hints, content_words)
                }
            }
            ResponseTemplate::StepByStep  => self.render_step_by_step(&relevant, nodes, hints),
            ResponseTemplate::Contrast    => self.render_contrast(&relevant, &plan.intent),
            ResponseTemplate::Causal      => self.render_causal(&relevant),
            ResponseTemplate::Fallback    => {
                let all: Vec<&ActivatedNode> = nodes.iter().collect();
                self.compose_paragraph_hints(&all, hints, content_words)
            }
        };

        self.apply_style_framing(&raw, hints)
    }

    /// Apply formality/casual framing to a composed response.
    fn apply_style_framing(&self, text: &str, hints: &StyleHints) -> String {
        if text.is_empty() {
            return text.to_string();
        }

        let mut result = text.to_string();

        // Casual prefix: "basically, " prepended to the first sentence.
        if let Some(prefix) = hints.casual_prefix {
            // Only add if the response doesn't already start with a casual marker.
            let lower = result.to_lowercase();
            if !lower.starts_with("basically") && !lower.starts_with("think of") {
                // Lowercase the first letter — but preserve acronyms/proper nouns.
                let first_word = result.split_whitespace().next().unwrap_or("");
                let is_acronym = first_word.len() >= 2
                    && first_word.chars().all(|c| c.is_uppercase() || !c.is_alphabetic());
                let has_internal_caps = first_word.len() >= 2 && {
                    let mut ci = first_word.chars();
                    let fc = ci.next().unwrap_or('a');
                    fc.is_uppercase() && ci.any(|c| c.is_uppercase())
                };
                if is_acronym || has_internal_caps {
                    // Preserve original casing: "basically, TCP is..."
                    result = format!("{}, {}", prefix, result);
                } else {
                    let mut chars = result.chars();
                    if let Some(first) = chars.next() {
                        result = format!("{}, {}{}", prefix, first.to_lowercase(), chars.as_str());
                    }
                }
            }
        }

        // Formal opener: prepend "Specifically, " if result doesn't already start formally.
        if hints.formal_opener {
            let lower = result.to_lowercase();
            let already_formal = lower.starts_with("specifically")
                || lower.starts_with("it is")
                || lower.starts_with("in summary")
                || lower.starts_with("more precisely");
            if !already_formal {
                result = format!("Specifically, {}", result);
            }
        }

        result
    }

    // ── Template renderers ────────────────────────────────────────────────────

    fn render_step_by_step(&self, relevant: &[&ActivatedNode], all: &[ActivatedNode], hints: &StyleHints) -> String {
        let mut parts: Vec<String> = Vec::new();

        if let Some(def_node) = all
            .iter()
            .find(|n| {
                n.knowledge_unit.kind() == KuKind::Definition
                    && !Self::is_training_artifact(n.knowledge_unit.raw_text())
            })
        {
            parts.push(ensure_period(def_node.knowledge_unit.raw_text()));
        }

        let mut found_steps = false;
        for n in relevant {
            match &n.knowledge_unit {
                KnowledgeUnit::Mechanism { steps, .. } if !steps.is_empty() => {
                    found_steps = true;
                    let raw = n.knowledge_unit.raw_text();
                    if !Self::is_training_artifact(raw) {
                        parts.push(ensure_period(raw));
                    }
                    if steps.len() > 1 {
                        for (i, step) in steps.iter().enumerate() {
                            if !Self::is_training_artifact(step) {
                                parts.push(format!("  {}. {}", i + 1, step));
                            }
                        }
                    }
                }
                KnowledgeUnit::Procedure { name, steps, expected_outcome, .. }
                    if !steps.is_empty() =>
                {
                    found_steps = true;
                    if !name.is_empty() {
                        parts.push(format!("**{}**:", name));
                    }
                    for (i, step) in steps.iter().enumerate() {
                        if !Self::is_training_artifact(step) {
                            parts.push(format!("  {}. {}", i + 1, step));
                        }
                    }
                    if let Some(outcome) = expected_outcome {
                        if !Self::is_training_artifact(outcome) {
                            parts.push(format!("  Expected outcome: {}", outcome));
                        }
                    }
                }
                // Skip Raw and other untyped nodes — they are not steps.
                // If no typed step nodes were found, we fall back to paragraph composition below.
                _ => {}
            }
        }

        // No step-structured KUs found — compose a coherent paragraph instead of
        // dumping whatever raw nodes happen to be in `relevant`.
        if !found_steps {
            let all_refs: Vec<&ActivatedNode> = all.iter().collect();
            return self.compose_paragraph_hints(&all_refs, hints, &[]);
        }

        parts.join("\n")
    }

    fn render_contrast(&self, nodes: &[&ActivatedNode], intent: &QueryIntent) -> String {
        let contrasts: Vec<&ActivatedNode> = nodes
            .iter()
            .copied()
            .filter(|n| n.knowledge_unit.kind() == KuKind::Contrast)
            .collect();

        if !contrasts.is_empty() {
            return contrasts
                .iter()
                .map(|n| match &n.knowledge_unit {
                    KnowledgeUnit::Contrast {
                        subject_a, subject_b, dimension, comparison, ..
                    } => {
                        if !subject_a.is_empty() && !subject_b.is_empty() {
                            format!("**{} vs {}** ({}): {}", subject_a, subject_b, dimension, comparison)
                        } else {
                            comparison.clone()
                        }
                    }
                    other => {
                        let text = other.raw_text();
                        if Self::is_training_artifact(text) { String::new() } else { text.to_string() }
                    }
                })
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");
        }

        // No Contrast KUs — group facts by which topic they mention.
        // A-facts first, B-facts second, shared/other last.
        if let QueryIntent::Comparison { topic_a, topic_b } = intent {
            if !topic_a.is_empty() && !topic_b.is_empty() {
                let a_lower = topic_a.to_lowercase();
                let b_lower = topic_b.to_lowercase();
                let group = |n: &&ActivatedNode| -> u8 {
                    let text = n.knowledge_unit.raw_text().to_lowercase();
                    let has_a = a_lower.split_whitespace()
                        .filter(|w| w.len() > 2)
                        .any(|w| text.contains(w));
                    let has_b = b_lower.split_whitespace()
                        .filter(|w| w.len() > 2)
                        .any(|w| text.contains(w));
                    match (has_a, has_b) {
                        (true, false) => 0,
                        (false, true) => 1,
                        _             => 2,
                    }
                };
                let mut sorted: Vec<&ActivatedNode> = nodes.to_vec();
                sorted.sort_by_key(|n| group(&n));

                // Build grouped response: A-section, B-section, shared.
                let mut parts: Vec<String> = Vec::new();
                let a_facts: Vec<&ActivatedNode> = sorted.iter().copied().filter(|n| group(&n) == 0).collect();
                let b_facts: Vec<&ActivatedNode> = sorted.iter().copied().filter(|n| group(&n) == 1).collect();
                let shared: Vec<&ActivatedNode>  = sorted.iter().copied().filter(|n| group(&n) == 2).collect();

                if !a_facts.is_empty() {
                    let para = self.compose_paragraph(&a_facts);
                    if !para.is_empty() {
                        parts.push(format!("**{}**: {}", topic_a, para));
                    }
                }
                if !b_facts.is_empty() {
                    let para = self.compose_paragraph(&b_facts);
                    if !para.is_empty() {
                        parts.push(format!("**{}**: {}", topic_b, para));
                    }
                }
                if !shared.is_empty() {
                    let para = self.compose_paragraph(&shared);
                    if !para.is_empty() {
                        parts.push(para);
                    }
                }

                if !parts.is_empty() {
                    return parts.join("\n\n");
                }
            }
        }

        self.compose_paragraph(nodes)
    }

    fn render_causal(&self, nodes: &[&ActivatedNode]) -> String {
        let causals: Vec<&ActivatedNode> = nodes
            .iter()
            .copied()
            .filter(|n| {
                n.knowledge_unit.kind() == KuKind::Causal
                    && !Self::is_training_artifact(n.knowledge_unit.raw_text())
            })
            .collect();

        if causals.is_empty() {
            return self.compose_paragraph(nodes);
        }

        causals
            .iter()
            .map(|n| match &n.knowledge_unit {
                KnowledgeUnit::Causal { cause, effect, condition, .. } => match condition {
                    Some(c) => format!("Because {} (under {}), {}.", cause, c, effect),
                    None => format!("{} causes {}.", cause, effect),
                },
                other => {
                    let text = other.raw_text();
                    if Self::is_training_artifact(text) { String::new() } else { text.to_string() }
                }
            })
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    // ── Multi-KU paragraph composer (neutral) ────────────────────────────────

    /// Weave all available typed KUs into a coherent paragraph (neutral style).
    fn compose_paragraph(&self, nodes: &[&ActivatedNode]) -> String {
        self.compose_paragraph_hints(nodes, &StyleHints::neutral(), &[])
    }

    /// Weave KUs into a paragraph, respecting style hints for layer count and examples.
    /// content_words: query keywords used to extract the most relevant sentences.
    pub fn compose_paragraph_hints(
        &self,
        nodes: &[&ActivatedNode],
        hints: &StyleHints,
        content_words: &[String],
    ) -> String {
        // Deduplicate nodes by seed before bucketing.
        let mut seen_seeds: std::collections::HashSet<String> = std::collections::HashSet::new();
        let deduped: Vec<&ActivatedNode> = nodes
            .iter()
            .copied()
            .filter(|n| seen_seeds.insert(n.seed.to_hex()))
            .collect();
        let nodes: &[&ActivatedNode] = &deduped;
        let mut sentences: Vec<String> = Vec::new();

        // ── Content quality helpers ───────────────────────────────────────────────

        // Nodes containing contrastive language describe what something is NOT.
        // Exclude from the definition bucket so they don't surface as the primary answer.
        let contrastive_markers = [
            "opposite of", "contrary to", "not to be confused", "contrast with",
            "in contrast to", "as opposed to", "not the same as", "unlike",
            "differs from", "different from",
        ];
        let is_contrastive_node = |text: &str| -> bool {
            let tl = text.to_lowercase();
            contrastive_markers.iter().any(|m| tl.contains(m))
        };

        // Reject training-data artifact nodes regardless of KU kind.
        // Delegates to the shared is_training_artifact() method.
        // Note: category-tag prefixed nodes ("science_general: ...") are NOT rejected here —
        // they contain useful content after the tag; clean_text() strips the prefix instead.
        let is_artifact_text = |text: &str| -> bool {
            Self::is_training_artifact(text)
        };

        // Strip underscore-style category-tag prefixes ("science_general: ", "climatology_weather: ").
        // These are ingest-time labels added by training pipelines; the content after them is the fact.
        let clean_text = |text: &str| -> String {
            if let Some(colon_pos) = text.find(':') {
                if colon_pos >= 3 && colon_pos <= 30 {
                    let tag_lower = text[..colon_pos].to_lowercase();
                    if tag_lower.contains('_')
                        && tag_lower.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                    {
                        let after = text[colon_pos + 1..].trim_start();
                        if after.len() > 10 { return after.to_string(); }
                    }
                }
            }
            text.to_string()
        };

        // Bucket nodes by KU kind, pre-filtering artifacts from every bucket.
        let defs: Vec<_>      = nodes.iter().filter(|n| {
            let text = n.knowledge_unit.raw_text();
            n.knowledge_unit.kind() == KuKind::Definition
                && !is_artifact_text(text)
                && !is_contrastive_node(text)
        }).collect();
        let comps: Vec<_>     = nodes.iter().filter(|n| {
            n.knowledge_unit.kind() == KuKind::Composition && !is_artifact_text(n.knowledge_unit.raw_text())
        }).collect();
        let mechs: Vec<_>     = nodes.iter().filter(|n| {
            n.knowledge_unit.kind() == KuKind::Mechanism && !is_artifact_text(n.knowledge_unit.raw_text())
        }).collect();
        let purposes: Vec<_>  = nodes.iter().filter(|n| {
            n.knowledge_unit.kind() == KuKind::Purpose && !is_artifact_text(n.knowledge_unit.raw_text())
        }).collect();
        let causals: Vec<_>   = nodes.iter().filter(|n| {
            n.knowledge_unit.kind() == KuKind::Causal && !is_artifact_text(n.knowledge_unit.raw_text())
        }).collect();
        let examples: Vec<_>  = nodes.iter().filter(|n| {
            n.knowledge_unit.kind() == KuKind::Example && !is_artifact_text(n.knowledge_unit.raw_text())
        }).collect();
        let contrasts: Vec<_> = nodes.iter().filter(|n| {
            n.knowledge_unit.kind() == KuKind::Contrast && !is_artifact_text(n.knowledge_unit.raw_text())
        }).collect();
        let raws: Vec<_>      = nodes.iter().filter(|n| {
            let text = n.knowledge_unit.raw_text();
            n.knowledge_unit.kind() == KuKind::Raw
                && !is_artifact_text(text)
                && !is_contrastive_node(text)
        }).collect();

        // High technicality → mechanism before definition for expert framing.
        // buckets: [0=defs, 1=comps, 2=mechs, 3=purposes, 4=causals, 5=examples, 6=contrasts]
        let layer_order: &[usize] = if hints.emphasise_mechanism {
            &[2, 0, 1, 3, 4, 5, 6]  // mech, def, comp, purpose, causal, example, contrast
        } else {
            &[0, 1, 2, 3, 4, 5, 6]  // def, comp, mech, purpose, causal, example, contrast
        };

        let buckets: [&Vec<_>; 7] = [&defs, &comps, &mechs, &purposes, &causals, &examples, &contrasts];
        let mut layers_added = 0usize;

        for &bucket_idx in layer_order {
            if layers_added >= hints.max_ku_layers {
                break;
            }
            let bucket = buckets[bucket_idx];

            // Sort bucket by relevance to content_words so the most on-topic KU
            // is selected first.  Without this, an off-topic Definition KU for
            // the right KuKind but wrong topic could win over a relevant one.
            let mut sorted_bucket: Vec<&&ActivatedNode> = bucket.iter().copied().collect();
            if !content_words.is_empty() {
                sorted_bucket.sort_by(|a, b| {
                    let sa = relevance_score(a.knowledge_unit.raw_text(), content_words);
                    let sb = relevance_score(b.knowledge_unit.raw_text(), content_words);
                    sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
                });
            }

            // Examples use max_examples quota — with the same relevance gate
            // as other bucket types so off-topic Example KUs are skipped.
            if bucket_idx == 5 {
                let mut examples_added = 0usize;
                let take = hints.max_examples.min(sorted_bucket.len());
                for node in sorted_bucket.iter().take(take) {
                    // Relevance gate: skip Example KU if it scores below 0.08
                    // (generous — prefix matching means even "neural" in the node
                    // earns full weight for a "neurologically" query word).
                    if !content_words.is_empty() {
                        let node_score = relevance_score(node.knowledge_unit.raw_text(), content_words);
                        if node_score < 0.08 {
                            continue; // skip off-topic example
                        }
                    }
                    let cleaned = clean_text(node.knowledge_unit.raw_text());
                    let best = if !content_words.is_empty() {
                        extract_best_sentences(&cleaned, content_words, 2)
                    } else {
                        cleaned
                    };
                    sentences.push(ensure_period(&best));
                    examples_added += 1;
                }
                if examples_added > 0 {
                    layers_added += 1;
                }
            } else {
                for node in sorted_bucket.iter().take(1) {
                    // Relevance gate: skip typed KU if even the best node in this
                    // bucket scores very low — means the bucket is off-topic.
                    // Threshold 0.08: with prefix matching, even a single relevant
                    // node word (e.g. "neural" for "neurologically") earns meaningful
                    // weight, so any on-topic node clears this bar.
                    if !content_words.is_empty() {
                        let node_score = relevance_score(node.knowledge_unit.raw_text(), content_words);
                        if node_score < 0.08 {
                            // Don't increment layers_added — try next bucket.
                            break;
                        }
                    }
                    let cleaned = clean_text(node.knowledge_unit.raw_text());
                    let best = if !content_words.is_empty() {
                        extract_best_sentences(&cleaned, content_words, 3)
                    } else {
                        cleaned
                    };
                    sentences.push(ensure_period(&best));
                    layers_added += 1;
                }
            }
        }

        // Contrast (always at most 1, only in verbose mode).
        if hints.max_ku_layers >= 6 {
            for node in contrasts.iter().take(1) {
                let text = ensure_period(node.knowledge_unit.raw_text());
                if !sentences.iter().any(|s| s == &text) {
                    sentences.push(text);
                }
            }
        }

        // Raw nodes — fill in when sentences are thin or in verbose mode.
        // Quality filter: skip nodes that look like table-of-contents, course catalog
        // entries, citations, or individual bullet-list items.
        let is_quality_raw = |text: &str| -> bool {
            let t = text.trim();
            // Reject anything with embedded newlines — these are multi-entry catalog/ToC blocks.
            if t.contains('\n') {
                return false;
            }
            // Reject table-like content (pipe-delimited or tab-delimited).
            if t.matches('|').count() >= 3 || t.matches('\t').count() >= 2 {
                return false;
            }
            // Reject very short fragments (< 30 chars) that aren't real sentences.
            if t.len() < 30 {
                return false;
            }
            // Reject bullet/list-marker prefixes (•, -, *, ·, numbered items like "1.")
            let stripped = t.trim_start_matches(['•', '·', '-', '*', '–', '—']);
            if stripped.len() < t.len() {
                return false;
            }
            let first_chars: String = t.chars().take(4).collect();
            if first_chars.len() >= 2 {
                let fc = first_chars.chars().next().unwrap();
                let sc = first_chars.chars().nth(1).unwrap_or(' ');
                let tc = first_chars.chars().nth(2).unwrap_or(' ');
                if fc.is_ascii_digit() && (sc == '.' || sc == ')') {
                    return false; // numbered list item "1. ..." or "1) ..."
                }
                // Also catch chapter/section titles like "1 What Is the Internet"
                if fc.is_ascii_digit() && sc == ' ' && tc.is_uppercase() {
                    return false;
                }
            }
            // Reject lines that are overwhelmingly non-alphabetic (e.g. pure table data).
            let alpha: usize = t.chars().filter(|c| c.is_alphabetic()).count();
            let total: usize = t.chars().count();
            if total > 0 && (alpha as f32 / total as f32) < 0.5 {
                return false;
            }
            // Reject citation-heavy fragments (lots of parenthesized years/references).
            let paren_count = t.matches('(').count();
            let word_count = t.split_whitespace().count();
            if word_count > 0 && paren_count as f32 / word_count as f32 > 0.15 {
                return false; // more than 15% of words are parenthesized refs
            }
            // Reject fragments that look like bare references: "Journal of X, 5:1127"
            let tl = t.to_lowercase();
            if tl.starts_with("journal of") || tl.starts_with("proceedings of")
                || tl.starts_with("vol.") || tl.starts_with("pp.")
            {
                return false;
            }
            // Reject Q:/A: training data artifacts — both at start and embedded.
            // Embedded "Q:" (e.g., "43 x 10^18 Q: Is the following...") signals Q&A dumps.
            if tl.starts_with("q:") || tl.starts_with("a:") {
                return false;
            }
            if t.contains(" Q:") || t.contains("\nQ:") {
                return false; // embedded Q: pattern — Q&A dump fragment
            }
            if tl.contains(" q:") || tl.contains("\nq:") {
                return false; // embedded lowercase q: — also a Q&A dump marker
            }
            // Reject other training-data artifact prefixes.
            if tl.starts_with("task:") || tl.starts_with("solution:")
                || tl.starts_with("user:") || tl.starts_with("assistant:")
            {
                return false;
            }
            // Reject contrastive text in raw nodes — these describe what something
            // is NOT and should never appear as a primary definition answer.
            let contrastive_raw_markers = [
                "opposite of", "contrary to", "not to be confused", "contrast with",
                "in contrast to", "as opposed to", "not the same as", "unlike",
                "differs from", "different from",
            ];
            if contrastive_raw_markers.iter().any(|m| tl.contains(m)) {
                return false;
            }
            // Note: category-tag prefixed nodes ("science_general: ...") are NOT rejected here.
            // clean_text() strips the tag; the content after it may be high quality.
            true
        };

        // Sort raw nodes by relevance to content_words before selecting any.
        // This ensures the most on-topic raw node is used when typed KUs are absent,
        // and prevents a high-strength but off-topic Raw from crowding out relevant ones.
        let mut sorted_raws: Vec<&&ActivatedNode> = raws.iter().copied().collect();
        if !content_words.is_empty() {
            sorted_raws.sort_by(|a, b| {
                let sa = relevance_score(a.knowledge_unit.raw_text(), content_words);
                let sb = relevance_score(b.knowledge_unit.raw_text(), content_words);
                sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        if sentences.is_empty() {
            for node in sorted_raws.iter() {
                let text = node.knowledge_unit.raw_text();
                if is_quality_raw(text) {
                    // Only add raw nodes that have at least some relevance when we have
                    // content words to judge against.  This prevents off-topic raws
                    // (e.g. country music trivia when asking about neuroscience) from
                    // filling an otherwise empty response.
                    if !content_words.is_empty() {
                        let score = relevance_score(text, content_words);
                        if score < 0.10 {
                            continue; // skip off-topic raws when fallback pool exists
                        }
                    }
                    let cleaned = clean_text(text);
                    // Also reject if clean_text output is short or starts with '|'
                    // (category-tag prefix stripped away, leaving a bare table row fragment).
                    if cleaned.len() < 30 || cleaned.trim_start().starts_with('|') {
                        continue;
                    }
                    sentences.push(ensure_period(&cleaned));
                    if sentences.len() >= 3 {
                        break;
                    }
                }
            }
            // Last-resort: if all raw nodes failed the quality/relevance filter,
            // check whether even the best raw node has any relevance before using it.
            // If not, return empty string — the caller's safety net will produce a
            // "I don't have strong knowledge about…" response, which is much better
            // than outputting an unrelated fragment (e.g. TIA study text).
            if sentences.is_empty() {
                if !content_words.is_empty() {
                    let best_raw_score = sorted_raws
                        .iter()
                        .map(|n| relevance_score(n.knowledge_unit.raw_text(), content_words))
                        .fold(0.0f32, f32::max);
                    if best_raw_score < 0.08 {
                        return String::new(); // no relevant raw node — admit knowledge gap
                    }
                }
                for node in sorted_raws.iter() {
                    let text = node.knowledge_unit.raw_text();
                    if !text.is_empty() && !is_artifact_text(text) {
                        let out = clean_text(text);
                        // Skip clean_text results that are short or pipe-prefixed.
                        if out.len() < 30 || out.trim_start().starts_with('|') {
                            continue;
                        }
                        sentences.push(ensure_period(&out));
                        break;
                    }
                }
            }
        } else if hints.max_ku_layers >= 5 {
            let covered = sentences.join(" ");
            let mut added = 0usize;
            for node in sorted_raws.iter() {
                let text = node.knowledge_unit.raw_text();
                if text.len() > 15 && !covered.contains(text) && is_quality_raw(text) {
                    // In verbose mode, still filter raws that have zero topic overlap.
                    if !content_words.is_empty() && relevance_score(text, content_words) < 0.10 {
                        continue;
                    }
                    sentences.push(ensure_period(&clean_text(text)));
                    added += 1;
                    if added >= 2 {
                        break;
                    }
                }
            }
        }

        // Join sentences with semantic connectors based on KU kind transitions.
        // Plain space between sentences that share a subject; a connector when
        // the relationship changes (definition→example, definition→mechanism, etc.).
        if sentences.is_empty() {
            return String::new();
        }
        let mut out = sentences[0].clone();
        // Join sentences with connectors. Limit connectors to avoid
        // "Additionally, X. Additionally, Y. Additionally, Z." spam.
        // After 1 connector, subsequent sentences just get a plain join.
        let mut connectors_used = 0usize;
        for i in 1..sentences.len() {
            let prev = &sentences[i - 1];
            let curr = &sentences[i];
            let connector = if connectors_used < 1 {
                choose_connector(prev, curr)
            } else {
                "" // No more connectors — just join with space
            };
            if connector.is_empty() {
                // Plain join: ensure period then space.
                if !out.ends_with(['.', '!', '?']) {
                    out.push('.');
                }
                out.push(' ');
                out.push_str(curr);
            } else {
                if !out.ends_with(['.', '!', '?']) {
                    out.push('.');
                }
                out.push(' ');
                out.push_str(connector);
                out.push(' ');
                out.push_str(&lowercase_unless_acronym(curr));
                connectors_used += 1;
            }
        }
        out
    }

    fn no_knowledge_response(&self, topic: &str) -> String {
        if topic.is_empty() {
            "I don't have enough information about that topic yet.".to_string()
        } else {
            format!(
                "I don't have enough information about '{}' yet. \
                 You can teach me with the /teach command.",
                topic
            )
        }
    }
}

// ── Free helpers ──────────────────────────────────────────────────────────────

/// Choose a semantic connector based on the content of consecutive sentences.
/// Returns an empty string when no connector is needed (plain continuation).
fn choose_connector(prev: &str, curr: &str) -> &'static str {
    let prev_l = prev.to_lowercase();
    let curr_l = curr.to_lowercase();

    // Contrast signal in current sentence
    if curr_l.contains("however") || curr_l.contains("unlike ") || curr_l.contains("in contrast")
        || curr_l.contains("whereas ") || curr_l.starts_with("but ")
    {
        return "In contrast,";
    }

    // Example / instance
    if curr_l.contains("for example") || curr_l.contains("for instance")
        || curr_l.contains("such as") || curr_l.starts_with("one ")
        || curr_l.starts_with("an example")
    {
        return "For example,";
    }

    // Current sentence is much more detailed than previous — only trigger
    // "More specifically" when the jump is significant (>2x length).
    let prev_wc = prev.split_whitespace().count();
    let curr_wc = curr.split_whitespace().count();
    if prev_wc > 5 && curr_wc > prev_wc * 2 {
        return "More specifically,";
    }

    // Historical / dated fact
    let has_date = (curr_l.contains(" in 19") || curr_l.contains(" in 20"))
        && (curr_l.contains("introduc") || curr_l.contains("develop")
            || curr_l.contains("creat") || curr_l.contains("publish"));
    if has_date {
        return "Historically,";
    }

    // Previous sentence was a definition, current adds a property/mechanism.
    // But only use "Specifically," if the current sentence genuinely narrows scope.
    if (prev_l.contains(" is a ") || prev_l.contains(" is an ") || prev_l.contains(" is the "))
        && !curr_l.contains(" is a ") && !curr_l.contains(" is an ")
        && curr_wc > 8 // Only if current sentence has enough substance
    {
        return "Specifically,";
    }

    // Default: no connector — let sentences flow naturally with just a period-space.
    // Overusing connectors like "Additionally," makes responses feel robotic.
    ""
}

/// Lowercase the first character of `s`, but preserve acronyms and camelCase words.
fn lowercase_unless_acronym(s: &str) -> String {
    let first_word = s.split_whitespace().next().unwrap_or("");
    // All-uppercase with len ≥ 2 → acronym (TCP, UDP, RNA …)
    if first_word.len() >= 2 && first_word.chars().all(|c| c.is_uppercase() || !c.is_alphabetic()) {
        return s.to_string();
    }
    // CamelCase (FileVault, GitHub …)
    if first_word.len() >= 2 {
        let mut ci = first_word.chars();
        let fc = ci.next().unwrap_or('a');
        if fc.is_uppercase() && ci.any(|c| c.is_uppercase()) {
            return s.to_string();
        }
    }
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_lowercase().collect::<String>() + chars.as_str(),
    }
}

/// Strip leading bullet markers and trailing list cruft, then ensure a period.
/// Handles: • · – — - * and numbered items like "1." / "1)" / "• 3 T cells".
fn ensure_period(s: &str) -> String {
    let trimmed = s.trim();
    // Strip leading bullet / list markers and any stray leading digits+punctuation.
    let cleaned = strip_bullet_prefix(trimmed);
    // If stripping left nothing, return empty so callers can skip it.
    if cleaned.is_empty() {
        return String::new();
    }
    if cleaned.ends_with(['.', '!', '?', ':']) {
        cleaned
    } else {
        format!("{}.", cleaned)
    }
}

/// Remove leading bullet chars and short numeric prefixes (e.g. "• 3 ", "1. ", "a) ").
fn strip_bullet_prefix(s: &str) -> String {
    let mut t = s.trim();
    // Strip unicode bullet chars
    loop {
        let before = t;
        t = t.trim_start_matches(['•', '·', '–', '—', '*']).trim_start();
        if t == before {
            break;
        }
    }
    // Strip leading "-" only if followed by a space (list marker, not hyphen in word)
    if t.starts_with("- ") {
        t = &t[2..];
    }
    // Strip leading short numeric prefix like "1. ", "12. ", "3) "
    let mut chars = t.chars();
    if let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            let rest: &str = chars.as_str();
            if rest.starts_with(". ") || rest.starts_with(") ") {
                t = &rest[2..];
            } else if rest.len() >= 2 {
                // Two-digit prefix: "12. "
                let mut rc = rest.chars();
                let c2 = rc.next().unwrap_or(' ');
                if c2.is_ascii_digit() {
                    let rest2 = rc.as_str();
                    if rest2.starts_with(". ") || rest2.starts_with(") ") {
                        t = &rest2[2..];
                    }
                }
            }
        }
    }
    t.trim().to_string()
}
