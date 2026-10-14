// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Thinking loop — ReAct-style multi-step reasoning for complex queries.
//!
//! Implements the Thought → Action → Observation cycle (ReAct pattern).
//! When QueryIntent is Complex or MultiPart the Brain routes through this
//! engine instead of direct single-pass recall.
//!
//! ## Phase 7: Multi-Agent Thinking (dual-process theory)
//!
//! Three specialised agents replace the single-pass synthesis:
//!
//! - **RecallAgent** — tight focused search (depth 2, top-5) for a single
//!   sub-question, wrapping the observations from Brain into a result struct.
//! - **SynthesisAgent** — weaves multiple RecallAgent results into a coherent
//!   unified answer; checks for contradictions between sub-answers.
//! - **CriticAgent** — reviews the synthesised answer against quality
//!   criteria and returns a `CritiqueReport { score, issues, suggestions }`.
//!
//! `Brain::think_through()` wires them into the pipeline:
//! ```text
//! decompose → RecallAgent per sub-Q → SynthesisAgent → CriticAgent
//!   → if score < 0.5, retry with suggestions → return final
//! ```
//!
//! A hard `max_steps` cap (default 5) prevents runaway chains.

// ── ThinkingStepKind ──────────────────────────────────────────────────────────

/// The three kinds of step in a ReAct chain.
#[derive(Debug, Clone)]
pub enum ThinkingStepKind {
    /// Internal reasoning about what to retrieve or do next.
    Thought { reasoning: String },
    /// A recall query or tool invocation dispatched to a brain region.
    Action  { query: String, action_type: String },
    /// The result observed after executing an Action.
    Observation { result: String, node_count: usize },
}

// ── ThinkingStep ──────────────────────────────────────────────────────────────

/// A single step in a reasoning chain.
#[derive(Debug, Clone)]
pub struct ThinkingStep {
    /// Sequential index within the chain (0-based).
    pub index: usize,
    pub kind:  ThinkingStepKind,
}

// ── ThinkingChain ─────────────────────────────────────────────────────────────

/// A complete ReAct trace from an initial query to a synthesised answer.
#[derive(Debug, Clone, Default)]
pub struct ThinkingChain {
    /// The original complex / multi-part query.
    pub query: String,
    /// Ordered trace of Thought / Action / Observation steps.
    pub steps: Vec<ThinkingStep>,
    /// Sub-questions decomposed from the original query.
    pub sub_questions: Vec<String>,
    /// The synthesised final answer (None while the chain is still in progress).
    pub final_answer: Option<String>,
}

// ── ThinkingEngine ────────────────────────────────────────────────────────────

/// Stateless reasoning coordinator.
///
/// `Brain::think_through()` owns the mutable state and calls back here
/// only for pure decomposition and synthesis logic.
#[derive(Debug)]
pub struct ThinkingEngine {
    /// Hard upper bound on the number of steps in a single chain.
    pub max_steps: usize,
}

impl Default for ThinkingEngine {
    fn default() -> Self {
        ThinkingEngine { max_steps: 5 }
    }
}

impl ThinkingEngine {
    pub fn new() -> Self {
        ThinkingEngine::default()
    }

    // ── Decomposition ─────────────────────────────────────────────────────────

    /// Break a complex / multi-part query into focused sub-questions.
    pub fn decompose(&self, query: &str) -> Vec<String> {
        let lower = query.to_lowercase();
        let parts = split_at_conjunctions(query, &lower);

        let filtered: Vec<String> = parts
            .into_iter()
            .map(|p| p.trim().to_string())
            .filter(|p| p.len() > 3)
            .collect();

        if filtered.len() > 1 {
            let mut out = filtered;
            out.truncate(self.max_steps);
            return out;
        }

        vec![query.trim().to_string()]
    }

    // ── Synthesis (legacy — used when multi_agent_thinking is off) ────────────

    /// Weave per-sub-question observations into a single coherent answer.
    pub fn synthesize(&self, query: &str, observations: &[(String, String)]) -> String {
        const EMPTY_MARKERS: &[&str] = &[
            "I don't have information",
            "I don't have enough information",
            "I don't have sufficient",
        ];

        let useful: Vec<&(String, String)> = observations
            .iter()
            .filter(|(_, obs)| {
                !obs.is_empty() && !EMPTY_MARKERS.iter().any(|m| obs.starts_with(m))
            })
            .collect();

        if useful.is_empty() {
            return format!(
                "I don't have sufficient information to answer comprehensively: {}",
                query
            );
        }

        if useful.len() == 1 {
            return useful[0].1.clone();
        }

        useful
            .iter()
            .map(|(sub_q, obs)| {
                let trimmed = sub_q
                    .trim_end_matches('?')
                    .trim_end_matches('.')
                    .trim();
                format!("**{}**: {}", capitalise(trimmed), obs)
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

// ── RecallAgentResult ─────────────────────────────────────────────────────────

/// Output from one RecallAgent run.
#[derive(Debug, Clone)]
pub struct RecallAgentResult {
    /// The sub-question this agent was answering.
    pub sub_question: String,
    /// Top node labels and activation strengths (up to 5).
    pub top_nodes: Vec<(String, f32)>,
    /// Narrated summary of what was found.
    pub summary: String,
}

impl RecallAgentResult {
    /// Construct from a sub-question and the narrated observation string.
    ///
    /// In the current implementation the Brain does the actual retrieval and
    /// narration; this constructor wraps the results for the agent pipeline.
    pub fn new(
        sub_question: impl Into<String>,
        summary: impl Into<String>,
        top_nodes: Vec<(String, f32)>,
    ) -> Self {
        RecallAgentResult {
            sub_question: sub_question.into(),
            top_nodes,
            summary: summary.into(),
        }
    }

    /// True if the result contains no useful information.
    fn is_empty(&self) -> bool {
        const EMPTY_MARKERS: &[&str] = &[
            "I don't have information",
            "I don't have enough information",
            "I don't have sufficient",
        ];
        self.summary.is_empty()
            || EMPTY_MARKERS.iter().any(|m| self.summary.starts_with(m))
    }
}

// ── RecallAgent ───────────────────────────────────────────────────────────────

/// Focused knowledge retrieval for a single sub-question.
///
/// Performs keyword search + spread activation with tight parameters
/// (depth 2, top-5 KUs, no hub-penalty relaxation).
///
/// In this architecture the Brain does the network calls; RecallAgent
/// provides the parameter contract and result wrapping.
pub struct RecallAgent;

impl RecallAgent {
    /// Recommended search depth for tight recall.
    pub const DEPTH: usize = 2;
    /// Recommended top-K for tight recall.
    pub const TOP_K: usize = 5;

    /// Build a RecallAgentResult from a sub-question and its narrated
    /// observation (produced by `Brain::recall_for_sub_q()`).
    pub fn run(
        sub_question: impl Into<String>,
        observation: impl Into<String>,
        top_nodes: Vec<(String, f32)>,
    ) -> RecallAgentResult {
        RecallAgentResult::new(sub_question, observation, top_nodes)
    }
}

// ── SynthesisAgent ────────────────────────────────────────────────────────────

/// Weaves multiple RecallAgent results into a unified answer.
///
/// Also checks for contradictions between sub-answers (simple heuristic:
/// look for negation words appearing next to shared key terms).
pub struct SynthesisAgent;

impl SynthesisAgent {
    /// Compose a unified answer from a list of RecallAgent results.
    pub fn synthesize(
        query: &str,
        results: &[RecallAgentResult],
        engine: &ThinkingEngine,
    ) -> String {
        // Reuse the legacy synthesize path via (sub_q, obs) pairs.
        let pairs: Vec<(String, String)> = results
            .iter()
            .map(|r| (r.sub_question.clone(), r.summary.clone()))
            .collect();
        engine.synthesize(query, &pairs)
    }

    /// Check for inter-result contradictions.
    ///
    /// Returns a list of human-readable conflict descriptions.
    /// Currently uses a light heuristic: if two summaries are both non-empty
    /// and one contains negation of a key term present in the other, flag it.
    pub fn detect_inter_contradictions(results: &[RecallAgentResult]) -> Vec<String> {
        let mut issues = Vec::new();

        for i in 0..results.len() {
            for j in (i + 1)..results.len() {
                let a = &results[i].summary.to_lowercase();
                let b = &results[j].summary.to_lowercase();

                if a.is_empty() || b.is_empty() {
                    continue;
                }

                // Simple: look for "not X" in one answer where X appears in the other.
                let negation_conflict = extract_negated_terms(a)
                    .iter()
                    .any(|term| b.contains(term.as_str()));
                let reverse_conflict = extract_negated_terms(b)
                    .iter()
                    .any(|term| a.contains(term.as_str()));

                if negation_conflict || reverse_conflict {
                    issues.push(format!(
                        "Possible conflict between sub-answers for '{}' and '{}'",
                        results[i].sub_question, results[j].sub_question
                    ));
                }
            }
        }

        issues
    }
}

// ── CritiqueReport ────────────────────────────────────────────────────────────

/// Assessment of a synthesised answer.
#[derive(Debug, Clone)]
pub struct CritiqueReport {
    /// Overall quality score: 0.0 = very poor, 1.0 = excellent.
    pub score: f32,
    /// Specific issues found (unsupported claims, contradictions, gaps).
    pub issues: Vec<String>,
    /// Concrete suggestions for improvement (fed back into the retry loop).
    pub suggestions: Vec<String>,
}

// ── CriticAgent ───────────────────────────────────────────────────────────────

/// Reviews a synthesised answer against quality criteria.
///
/// Criteria checked:
/// 1. Does the answer address the original query? (topic coverage)
/// 2. Are there unsupported claims? (claims with no backing in recall results)
/// 3. Are there contradictions? (via SynthesisAgent::detect_inter_contradictions)
pub struct CriticAgent;

impl CriticAgent {
    /// Critique `answer` against the original `query` and the supporting
    /// `recall_results`.  Returns a `CritiqueReport`.
    pub fn critique(
        query: &str,
        answer: &str,
        recall_results: &[RecallAgentResult],
    ) -> CritiqueReport {
        let mut issues = Vec::new();
        let mut suggestions = Vec::new();
        let mut score: f32 = 1.0;

        // ── Criterion 1: topic coverage ───────────────────────────────────────
        let query_words = significant_words(query);
        let answer_lower = answer.to_lowercase();
        let uncovered: Vec<&str> = query_words
            .iter()
            .filter(|w| !answer_lower.contains(**w))
            .copied()
            .collect();

        if !uncovered.is_empty() {
            let penalty = 0.1 * uncovered.len() as f32;
            score -= penalty.min(0.4);
            issues.push(format!(
                "Answer does not address these query terms: {}",
                uncovered.join(", ")
            ));
            suggestions.push(format!(
                "Search specifically for: {}",
                uncovered.join(", ")
            ));
        }

        // ── Criterion 2: unsupported claims ───────────────────────────────────
        let all_recall_text: String = recall_results
            .iter()
            .filter(|r| !r.is_empty())
            .map(|r| r.summary.to_lowercase())
            .collect::<Vec<_>>()
            .join(" ");

        if all_recall_text.is_empty() && !answer.is_empty() {
            score -= 0.3;
            issues.push("Answer has no supporting recall evidence".to_string());
            suggestions.push("Try broader search terms for the original query".to_string());
        } else {
            // Check for sentences in the answer that contain no shared terms
            // with any recall result.
            let unsupported = count_unsupported_sentences(answer, &all_recall_text);
            if unsupported > 2 {
                let penalty = 0.05 * unsupported as f32;
                score -= penalty.min(0.2);
                issues.push(format!(
                    "{} sentences in the answer appear unsupported by recall results",
                    unsupported
                ));
                suggestions.push("Cross-check answer sentences against retrieved knowledge".to_string());
            }
        }

        // ── Criterion 3: contradictions ───────────────────────────────────────
        let contradictions = SynthesisAgent::detect_inter_contradictions(recall_results);
        if !contradictions.is_empty() {
            score -= 0.1 * contradictions.len() as f32;
            issues.extend(contradictions.clone());
            suggestions.push("Resolve contradictions between sub-answers before synthesising".to_string());
        }

        // ── Criterion 4: answer completeness ─────────────────────────────────
        let word_count = answer.split_whitespace().count();
        if word_count < 10 {
            score -= 0.2;
            issues.push("Answer is very short — may be incomplete".to_string());
            suggestions.push("Expand recall depth or add more sub-questions".to_string());
        }

        CritiqueReport {
            score: score.clamp(0.0, 1.0),
            issues,
            suggestions,
        }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Recursively split a query on recognised multi-part conjunctions.
fn split_at_conjunctions(original: &str, lower: &str) -> Vec<String> {
    const SPLITS: &[&str] = &[
        " and what ",
        " and how ",
        " and why ",
        " and where ",
        " and when ",
        " also what ",
        " also how ",
        " also why ",
    ];

    for &splitter in SPLITS {
        if let Some(pos) = lower.find(splitter) {
            let first = original[..pos].trim().to_string();
            let rest  = original[pos + splitter.len()..].trim().to_string();
            if first.len() > 3 && rest.len() > 3 {
                let rest_lower = rest.to_lowercase();
                let mut result = vec![first];
                result.extend(split_at_conjunctions(&rest, &rest_lower));
                return result;
            }
        }
    }

    vec![original.to_string()]
}

fn capitalise(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None    => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

/// Extract terms that appear negated ("not X", "no X", "isn't X") in text.
fn extract_negated_terms(text: &str) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut negated = Vec::new();
    const NEGATORS: &[&str] = &["not", "no", "isn't", "aren't", "doesn't", "don't", "never"];

    for (i, word) in words.iter().enumerate() {
        if NEGATORS.contains(word) {
            if let Some(&next) = words.get(i + 1) {
                let term = next.trim_matches(|c: char| !c.is_alphabetic());
                if term.len() >= 4 {
                    negated.push(term.to_string());
                }
            }
        }
    }
    negated
}

/// Words from the query that matter for coverage checking.
fn significant_words(query: &str) -> Vec<&str> {
    const STOP: &[&str] = &[
        "the", "a", "an", "is", "are", "was", "were", "what", "how",
        "why", "who", "when", "where", "does", "do", "did", "will",
        "can", "could", "would", "should", "tell", "me", "about",
        "explain", "describe", "give", "list",
    ];
    query
        .split(|c: char| !c.is_alphabetic())
        .map(|w| w.trim())
        .filter(|w| w.len() >= 4 && !STOP.contains(w))
        .collect()
}

/// Count sentences in `answer` whose content words do not appear in
/// `recall_text` (very rough: overlap < 1 shared word of length ≥ 5).
fn count_unsupported_sentences(answer: &str, recall_text: &str) -> usize {
    answer
        .split(|c| c == '.' || c == '!' || c == '?')
        .filter(|sentence| {
            let s = sentence.trim().to_lowercase();
            if s.len() < 20 {
                return false; // too short to evaluate
            }
            let shared = s
                .split_whitespace()
                .filter(|w| w.len() >= 5 && recall_text.contains(*w))
                .count();
            shared == 0
        })
        .count()
}
