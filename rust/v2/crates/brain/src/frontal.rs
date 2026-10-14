// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Frontal lobe — intent classification and response planning.
//!
//! Classifies the query intent and builds a ResponsePlan that tells
//! the narration engine which KU types to pull and which template to use.

use graph::knowledge_unit::KuKind;
use serde::{Deserialize, Serialize};

// ── QueryIntent ───────────────────────────────────────────────────────────────

/// What the user is actually asking for.  Drives template selection and
/// the set of KU kinds the temporal lobe should surface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryIntent {
    /// "What is X?" / "Define X"
    Definition { topic: String },
    /// "How does X work?"
    Mechanism { topic: String },
    /// "X vs Y" / "Compare X and Y"
    Comparison { topic_a: String, topic_b: String },
    /// "Why does X cause Y?" / "What causes X?"
    Causal { cause: Option<String>, effect: Option<String> },
    /// "How do I do X?" / "Steps to X"
    Procedure { task: String },
    /// "Tell me about X" — open-ended retrieval
    OpenEnded { topic: String },
    /// "Who is X?" / "Who made X?"
    Who { subject: String },
    /// Free-form creative or generative request
    Creative { prompt: String },
    /// Multi-faceted query requiring decomposition into focused sub-questions.
    /// Detected when a single input contains multiple "wh-" question clauses
    /// joined by conjunctions ("and how", "and what", etc.).
    MultiPart { parts: Vec<String> },
    /// Complex question benefiting from a multi-step thinking chain even though
    /// it reads as a single question (e.g., contains "comprehensively",
    /// "all aspects of", "in detail").
    Complex { topic: String },
}

impl QueryIntent {
    pub fn topic(&self) -> &str {
        match self {
            QueryIntent::Definition { topic }       => topic,
            QueryIntent::Mechanism { topic }        => topic,
            QueryIntent::Comparison { topic_a, .. } => topic_a,
            QueryIntent::Causal { effect, .. }      => effect.as_deref().unwrap_or(""),
            QueryIntent::Procedure { task }         => task,
            QueryIntent::OpenEnded { topic }        => topic,
            QueryIntent::Who { subject }            => subject,
            QueryIntent::Creative { prompt }        => prompt,
            QueryIntent::MultiPart { parts }        => parts.first().map(|s| s.as_str()).unwrap_or(""),
            QueryIntent::Complex { topic }          => topic,
        }
    }

    pub fn tag(&self) -> &'static str {
        match self {
            QueryIntent::Definition { .. }  => "definition",
            QueryIntent::Mechanism { .. }   => "mechanism",
            QueryIntent::Comparison { .. }  => "comparison",
            QueryIntent::Causal { .. }      => "causal",
            QueryIntent::Procedure { .. }   => "procedure",
            QueryIntent::OpenEnded { .. }   => "open_ended",
            QueryIntent::Who { .. }         => "who",
            QueryIntent::Creative { .. }    => "creative",
            QueryIntent::MultiPart { .. }   => "multi_part",
            QueryIntent::Complex { .. }     => "complex",
        }
    }
}

// ── ResponseConfidence ────────────────────────────────────────────────────────

/// How confident the brain is in a given response.
///
/// Derived from: number of KUs recalled, their average activation strength,
/// and the bond weights of nodes in the recall set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResponseConfidence {
    /// Many relevant KUs recalled with high activation.
    Strong,
    /// A moderate number of KUs recalled with reasonable strength.
    Moderate,
    /// Few KUs, low activation, or low bond weights.
    Weak,
    /// No KUs recalled — cannot produce a grounded answer.
    Empty,
}

impl ResponseConfidence {
    pub fn label(&self) -> &'static str {
        match self {
            ResponseConfidence::Strong   => "strong",
            ResponseConfidence::Moderate => "moderate",
            ResponseConfidence::Weak     => "weak",
            ResponseConfidence::Empty    => "empty",
        }
    }
}

/// Compute response confidence from recall statistics.
///
/// Relaxed thresholds: 4+ sources with any activation strength should be
/// at least Moderate, not a gap. 3+ sources at strength ≥ 0.3 → Strong.
pub fn compute_response_confidence(ku_count: usize, avg_strength: f32) -> ResponseConfidence {
    match (ku_count, avg_strength) {
        (0, _)                              => ResponseConfidence::Empty,
        (1..=2, s) if s < 0.3              => ResponseConfidence::Weak,
        (1..=2, _)                         => ResponseConfidence::Moderate,
        (3..=6, s) if s >= 0.3             => ResponseConfidence::Strong,
        (3..=6, _)                         => ResponseConfidence::Moderate,
        (n, s) if n > 6 && s >= 0.3       => ResponseConfidence::Strong,
        _                                   => ResponseConfidence::Moderate,
    }
}

// ── StyleHints ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum BrevityLevel {
    Terse,
    #[default]
    Normal,
    Verbose,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StyleHints {
    pub formal: bool,
    pub technical: bool,
    pub enumerate_steps: bool,
    pub include_examples: bool,
    pub brevity: BrevityLevel,
}

// ── ResponsePlan ──────────────────────────────────────────────────────────────

/// What the narration engine needs to produce a well-structured response.
#[derive(Debug, Clone)]
pub struct ResponsePlan {
    pub intent: QueryIntent,
    /// Named output slots the template expects to fill (e.g., "subject", "steps").
    pub template_slots: Vec<String>,
    /// KU kinds the temporal lobe should prioritise when searching the graph.
    pub required_ku_kinds: Vec<KuKind>,
    pub style_hints: StyleHints,
    pub max_response_length: Option<usize>,
}

// ── FrontalLobe ───────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct FrontalLobe {
    pub last_intent: Option<QueryIntent>,
    pub intent_history: Vec<String>,
}

impl FrontalLobe {
    pub fn new() -> Self {
        FrontalLobe::default()
    }

    /// Classify the query into a QueryIntent using simple lexical heuristics.
    /// A real implementation will use the embedding engine for semantic routing.
    pub fn classify_intent(&mut self, query: &str) -> QueryIntent {
        let q = query.to_lowercase();

        let intent = if is_multi_part(&q) {
            // Check multi-part FIRST — a query like "What causes X and how does Y"
            // must not be hijacked by single-intent patterns (e.g. Causal).
            let parts = extract_multi_parts(&q);
            QueryIntent::MultiPart { parts }
        } else if is_complex_query(&q) {
            // Single question that explicitly requests comprehensive treatment.
            let topic = strip_prefix(&q, &[
                "comprehensively explain", "tell me everything about",
                "explain in detail", "explain thoroughly",
                "describe in detail", "describe comprehensively",
                "tell me about",
            ]);
            QueryIntent::Complex { topic }
        } else if q.starts_with("what is")
            || q.starts_with("what are")
            || q.starts_with("define ")
            || q.starts_with("definition of")
        {
            let topic = strip_prefix(&q, &["what is", "what are", "define", "definition of"]);
            // "What is the difference between X and Y?" → Comparison, not Definition.
            let topic_trimmed = topic.trim_start_matches("the ").trim_start_matches("a ");
            if topic_trimmed.starts_with("difference between ") {
                let rest = topic_trimmed.trim_start_matches("difference between ");
                if let Some((a, b)) = rest.split_once(" and ") {
                    let b_clean = b.trim_end_matches('?').trim_end_matches('.').trim();
                    QueryIntent::Comparison {
                        topic_a: a.trim().to_string(),
                        topic_b: b_clean.to_string(),
                    }
                } else {
                    QueryIntent::Comparison {
                        topic_a: rest.trim().to_string(),
                        topic_b: String::new(),
                    }
                }
            } else {
                QueryIntent::Definition { topic }
            }
        } else if q.contains(" vs ") || q.contains(" versus ") || q.starts_with("compare ")
            || q.contains(" differ") || q.contains("difference between ")
        {
            // Comparison intent — parse out the two topics.
            if q.contains("difference between ") {
                let rest = q.splitn(2, "difference between ").nth(1).unwrap_or("");
                if let Some((a, b)) = rest.split_once(" and ") {
                    QueryIntent::Comparison {
                        topic_a: a.trim().to_string(),
                        topic_b: b.trim().to_string(),
                    }
                } else {
                    QueryIntent::Comparison {
                        topic_a: rest.trim().to_string(),
                        topic_b: String::new(),
                    }
                }
            } else if q.starts_with("compare ") {
                let rest = strip_prefix(&q, &["compare"]);
                let (a, b) = rest.split_once(" and ")
                    .or_else(|| rest.split_once(" with "))
                    .or_else(|| rest.split_once(" to "))
                    .or_else(|| rest.split_once(" vs "))
                    .unwrap_or((&rest, ""));
                QueryIntent::Comparison {
                    topic_a: a.trim().to_string(),
                    topic_b: b.trim().to_string(),
                }
            } else if q.contains(" differ") {
                // "how do X and Y differ" / "how does X differ from Y"
                let stripped = strip_prefix(&q, &[
                    "how do", "how does", "how are", "how is",
                ]);
                if let Some(pos) = stripped.find(" differ") {
                    let subjects = &stripped[..pos];
                    if let Some((a, b)) = subjects.split_once(" and ") {
                        QueryIntent::Comparison {
                            topic_a: a.trim().to_string(),
                            topic_b: b.trim().to_string(),
                        }
                    } else if let Some(b_rest) = stripped.get(pos..).and_then(|s| s.strip_prefix(" differ from ")) {
                        QueryIntent::Comparison {
                            topic_a: subjects.trim().to_string(),
                            topic_b: b_rest.trim().to_string(),
                        }
                    } else {
                        QueryIntent::Comparison {
                            topic_a: subjects.trim().to_string(),
                            topic_b: String::new(),
                        }
                    }
                } else {
                    QueryIntent::OpenEnded { topic: q.clone() }
                }
            } else {
                // "X vs Y" / "X versus Y"
                let (a, b) = q.split_once(" vs ")
                    .or_else(|| q.split_once(" versus "))
                    .unwrap_or((&q, ""));
                QueryIntent::Comparison {
                    topic_a: a.trim().to_string(),
                    topic_b: b.trim().to_string(),
                }
            }
        } else if q.starts_with("how does") || (q.starts_with("how do") && !q.contains(" i ")) {
            let topic = strip_prefix(&q, &["how does", "how do"]);
            QueryIntent::Mechanism { topic }
        } else if q.starts_with("why") || q.contains(" cause") || q.contains(" leads to") {
            QueryIntent::Causal {
                cause: None,
                effect: Some(query.to_string()),
            }
        } else if q.starts_with("how to")
            || q.starts_with("steps to")
            || q.starts_with("how do i")
        {
            let task = strip_prefix(&q, &["how to", "steps to", "how do i"]);
            QueryIntent::Procedure { task }
        } else if q.starts_with("who") {
            QueryIntent::Who {
                subject: strip_prefix(&q, &["who is", "who was", "who"]),
            }
        } else {
            // Strip common OpenEnded prefixes so the topic is just the subject.
            let topic = strip_prefix(&q, &[
                "tell me about", "explain", "describe", "what do you know about",
                "information about", "info about",
            ]);
            QueryIntent::OpenEnded { topic }
        };

        self.intent_history.push(intent.tag().to_string());
        self.last_intent = Some(intent.clone());
        intent
    }

    /// Build a ResponsePlan from a classified intent.
    pub fn build_plan(&self, intent: QueryIntent) -> ResponsePlan {
        let (slots, kinds) = match &intent {
            QueryIntent::Definition { .. } => (
                vec!["subject", "parent_type", "properties"],
                vec![KuKind::Definition],
            ),
            QueryIntent::Mechanism { .. } => (
                vec!["subject", "steps", "purpose"],
                vec![KuKind::Mechanism, KuKind::Procedure],
            ),
            QueryIntent::Comparison { .. } => (
                vec!["subject_a", "subject_b", "dimension", "comparison"],
                vec![KuKind::Contrast, KuKind::Definition],
            ),
            QueryIntent::Causal { .. } => (
                vec!["cause", "effect", "condition"],
                vec![KuKind::Causal, KuKind::Mechanism],
            ),
            QueryIntent::Procedure { .. } => (
                vec!["name", "steps", "prerequisites", "outcome"],
                vec![KuKind::Procedure, KuKind::Mechanism],
            ),
            QueryIntent::Who { .. } => (
                vec!["subject", "role", "context"],
                vec![KuKind::Definition, KuKind::Purpose],
            ),
            QueryIntent::Creative { .. } => (
                vec!["prompt"],
                vec![KuKind::Raw, KuKind::Example],
            ),
            QueryIntent::OpenEnded { .. } => (
                vec!["content"],
                vec![
                    KuKind::Definition,
                    KuKind::Mechanism,
                    KuKind::Raw,
                ],
            ),
            // Complex and MultiPart queries are routed through the thinking loop;
            // this plan is the fallback used for sub-question narration.
            QueryIntent::Complex { .. } | QueryIntent::MultiPart { .. } => (
                vec!["content"],
                vec![
                    KuKind::Definition,
                    KuKind::Mechanism,
                    KuKind::Causal,
                    KuKind::Raw,
                ],
            ),
        };

        ResponsePlan {
            intent,
            template_slots: slots.into_iter().map(str::to_string).collect(),
            required_ku_kinds: kinds,
            style_hints: StyleHints::default(),
            max_response_length: None,
        }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn strip_prefix(s: &str, prefixes: &[&str]) -> String {
    for p in prefixes {
        if let Some(rest) = s.strip_prefix(p) {
            return rest.trim().to_string();
        }
    }
    s.trim().to_string()
}

/// True when the query contains multiple question-clause conjunctions such as
/// "and how", "and what", "and why" that signal separate retrievals are needed.
fn is_multi_part(q: &str) -> bool {
    const CONJUNCTIONS: &[&str] = &[
        " and what ", " and how ", " and why ", " and where ",
        " also what ", " also how ", " also why ",
    ];
    let has_conjunction = CONJUNCTIONS.iter().any(|c| q.contains(c));
    if !has_conjunction {
        return false;
    }
    // Require at least one leading question word so we don't split declaratives.
    q.starts_with("what")
        || q.starts_with("how")
        || q.starts_with("why")
        || q.starts_with("where")
        || q.starts_with("when")
        || q.starts_with("who")
}

/// True when the query contains explicit signals that the user wants a
/// comprehensive, multi-angle answer (even as a single question).
fn is_complex_query(q: &str) -> bool {
    const SIGNALS: &[&str] = &[
        "comprehensively",
        "in detail",
        "all aspects",
        "thoroughly",
        "everything about",
        "full explanation",
    ];
    SIGNALS.iter().any(|s| q.contains(s))
}

/// Split a multi-part query into its constituent questions using the same
/// conjunction markers as `is_multi_part`.
fn extract_multi_parts(q: &str) -> Vec<String> {
    const SPLITS: &[&str] = &[
        " and what ", " and how ", " and why ", " and where ",
        " also what ", " also how ", " also why ",
    ];
    // Iterative split — process each piece with the same set of markers.
    let mut parts: Vec<String> = vec![q.to_string()];
    for &splitter in SPLITS {
        let mut next: Vec<String> = Vec::new();
        for part in parts {
            if let Some(pos) = part.to_lowercase().find(splitter) {
                let first = part[..pos].trim().to_string();
                let rest  = part[pos + splitter.len()..].trim().to_string();
                if first.len() > 3 {
                    next.push(first);
                }
                if rest.len() > 3 {
                    next.push(rest);
                }
            } else {
                next.push(part);
            }
        }
        parts = next;
    }
    parts
}
