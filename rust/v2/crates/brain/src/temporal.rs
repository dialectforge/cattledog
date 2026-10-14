// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Temporal lobe — KU extraction pipeline and knowledge recall coordination.
//!
//! The temporal lobe is responsible for two things:
//! 1. Extracting typed KnowledgeUnits from raw text (teach pipeline)
//! 2. Translating a ResponsePlan's required KU kinds into graph recall seeds
//!
//! Phase 2: Robust case-insensitive extractors for all 8 typed KU kinds.
//! Extraction order (most specific first):
//!   Definition → Mechanism → Composition → Purpose → Example → Contrast → Causal → Raw

use graph::knowledge_unit::{KnowledgeUnit, KuKind};
use graph::network::RecallResult;

// ── Extraction ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ExtractionContext {
    pub source_text: String,
    pub domain_hint: Option<String>,
    /// Discard candidate KUs below this confidence score.
    pub confidence_floor: f32,
}

impl ExtractionContext {
    pub fn new(text: impl Into<String>) -> Self {
        ExtractionContext {
            source_text: text.into(),
            domain_hint: None,
            confidence_floor: 0.4,
        }
    }

    pub fn with_domain(mut self, domain: impl Into<String>) -> Self {
        self.domain_hint = Some(domain.into());
        self
    }
}

#[derive(Debug, Clone)]
pub struct ExtractionResult {
    pub units: Vec<KnowledgeUnit>,
    pub extraction_confidence: f32,
    pub tokens_processed: usize,
    pub sentences_seen: usize,
}

// ── TemporalLobe ──────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct TemporalLobe {
    pub extraction_history: Vec<ExtractionResult>,
    /// Stemmed word → seed hexes that mention it (built lazily from graph).
    pub word_index: std::collections::HashMap<String, Vec<String>>,
    pub word_index_valid: bool,
}

impl TemporalLobe {
    pub fn new() -> Self {
        TemporalLobe::default()
    }

    /// Extract KnowledgeUnits from raw text using heuristic sentence patterns.
    ///
    /// Applies patterns in priority order so the most specific type wins.
    /// Falls back to Raw for any sentence that doesn't match a typed pattern.
    pub fn extract(&mut self, ctx: &ExtractionContext) -> ExtractionResult {
        let text = &ctx.source_text;
        let sentences: Vec<&str> = text
            .split(['.', '!', '?'])
            .filter(|s| !s.trim().is_empty())
            .collect();
        let mut units: Vec<KnowledgeUnit> = Vec::new();

        for sentence in &sentences {
            let s = sentence.trim();
            if s.is_empty() {
                continue;
            }

            let ku = try_extract_definition(s)
                .or_else(|| try_extract_mechanism(s))
                .or_else(|| try_extract_composition(s))
                .or_else(|| try_extract_purpose(s))
                .or_else(|| try_extract_example(s))
                .or_else(|| try_extract_contrast(s))
                .or_else(|| try_extract_causal(s))
                .unwrap_or_else(|| KnowledgeUnit::Raw {
                    text: s.to_string(),
                    source: ctx.domain_hint.clone(),
                });

            units.push(ku);
        }

        let typed_count = units
            .iter()
            .filter(|u| u.kind() != KuKind::Raw)
            .count();
        let confidence = if units.is_empty() {
            0.0
        } else {
            0.5 + 0.5 * (typed_count as f32 / units.len() as f32)
        };

        let result = ExtractionResult {
            extraction_confidence: confidence,
            tokens_processed: text.split_whitespace().count(),
            sentences_seen: sentences.len(),
            units,
        };

        self.extraction_history.push(result.clone());
        result
    }

    /// Filter already-recalled results to only those matching required KU kinds.
    pub fn filter_by_kinds<'a>(
        results: &'a RecallResult,
        required: &[KuKind],
    ) -> Vec<&'a graph::network::ActivatedNode> {
        results
            .activated_nodes
            .iter()
            .filter(|n| required.contains(&n.knowledge_unit.kind()))
            .collect()
    }

    pub fn invalidate_word_index(&mut self) {
        self.word_index_valid = false;
    }
}

// ── Extraction heuristics ─────────────────────────────────────────────────────
//
// All matchers work on the lowercase copy of the sentence for case-insensitive
// pattern matching, but extract field values from the original `s` to preserve
// capitalisation in structured fields.

/// Words that indicate a subordinate clause or preposition — stop the
/// parent_type capture before them so we don't include trailing phrases.
const TYPE_STOP_WORDS: &[&str] = &[
    "that", "which", "who", "where", "when", "used", "with", "by", "for",
    "from", "in", "on", "at", "called", "known", "inspired", "based",
    "made", "built", "including", "through", "via", "using",
];

/// Pronouns that should not be used as subject anchors for shared-subject bonds.
pub const PRONOUN_SUBJECTS: &[&str] = &[
    "it", "they", "this", "these", "that", "those", "he", "she", "we",
    "one", "each",
];

/// Extract a concise parent_type from the rest of a definition sentence.
/// Takes up to 3 words, stopping before TYPE_STOP_WORDS.
fn extract_parent_type(rest: &str) -> Option<String> {
    let words: Vec<&str> = rest
        .split_whitespace()
        .take_while(|w| !TYPE_STOP_WORDS.contains(&w.to_lowercase().as_str()))
        .take(3)
        .collect();
    if words.is_empty() {
        None
    } else {
        Some(words.join(" "))
    }
}

/// Split a list phrase like "A, B, and C" into individual items.
fn split_list_items(s: &str) -> Vec<String> {
    s.split(&[',', ';'][..])
        .flat_map(|chunk| chunk.split(" and "))
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

// ── Definition ────────────────────────────────────────────────────────────────

fn try_extract_definition(s: &str) -> Option<KnowledgeUnit> {
    let lower = s.to_lowercase();

    // Patterns ordered most-specific first; each entry is (pattern, pattern_len).
    let patterns: &[(&str, usize)] = &[
        (" is defined as ", 15),
        (" refers to ",     11),
        (" is a ",           6),
        (" is an ",          7),
        (" is the ",         8),
    ];

    for &(pattern, pat_len) in patterns {
        if let Some(pos) = lower.find(pattern) {
            let subject = s[..pos].trim().to_string();
            // Skip empty or single-char subjects (likely mid-sentence matches)
            if subject.len() < 2 {
                continue;
            }
            let rest_raw = s[pos + pat_len..].trim();
            // Strip a leading article from the rest so "a process" → "process"
            let rest_stripped = rest_raw
                .strip_prefix("a ")
                .or_else(|| rest_raw.strip_prefix("an "))
                .unwrap_or(rest_raw);
            let parent_type = extract_parent_type(rest_stripped);
            return Some(KnowledgeUnit::Definition {
                subject,
                parent_type,
                properties: vec![],
                raw_text: s.to_string(),
            });
        }
    }
    None
}

// ── Mechanism ─────────────────────────────────────────────────────────────────

fn try_extract_mechanism(s: &str) -> Option<KnowledgeUnit> {
    let lower = s.to_lowercase();

    let patterns: &[(&str, usize)] = &[
        (" works by ",        10),
        (" work by ",          9),
        (" operates by ",     13),
        (" functions by ",    14),
        (" works through ",   15),
        (" operates through ", 19),
    ];

    for &(pattern, pat_len) in patterns {
        if let Some(pos) = lower.find(pattern) {
            let subject = s[..pos].trim().to_string();
            if subject.len() < 2 {
                continue;
            }
            let rest = s[pos + pat_len..].trim().to_string();

            // Extract a purpose clause after " to " (e.g., "works by X to achieve Y")
            let purpose = lower[pos + pat_len..]
                .find(" to ")
                .map(|p| s[pos + pat_len + p + 4..].trim().to_string())
                .unwrap_or_default();

            return Some(KnowledgeUnit::Mechanism {
                subject,
                steps: vec![rest],
                purpose,
                raw_text: s.to_string(),
            });
        }
    }
    None
}

// ── Composition ───────────────────────────────────────────────────────────────

fn try_extract_composition(s: &str) -> Option<KnowledgeUnit> {
    let lower = s.to_lowercase();

    let patterns: &[(&str, usize)] = &[
        (" consists of ",     13),
        (" is made of ",      11),
        (" is composed of ",  16),
        (" is made up of ",   15),
        (" comprises ",       11),
        (" is built from ",   15),
        (" is built of ",     13),
    ];

    for &(pattern, pat_len) in patterns {
        if let Some(pos) = lower.find(pattern) {
            let whole = s[..pos].trim().to_string();
            if whole.len() < 2 {
                continue;
            }
            let parts_str = s[pos + pat_len..].trim();
            let parts = split_list_items(parts_str);
            return Some(KnowledgeUnit::Composition {
                whole,
                parts,
                part_roles: vec![],
                raw_text: s.to_string(),
            });
        }
    }
    None
}

// ── Purpose ───────────────────────────────────────────────────────────────────

fn try_extract_purpose(s: &str) -> Option<KnowledgeUnit> {
    let lower = s.to_lowercase();

    let patterns: &[(&str, usize)] = &[
        (" is used to ",      12),
        (" is used for ",     13),
        (" is designed to ",  16),
        (" are used to ",     13),
        (" are used for ",    14),
        (" enables ",          9),
        (" allows ",           8),
    ];

    for &(pattern, pat_len) in patterns {
        if let Some(pos) = lower.find(pattern) {
            let subject = s[..pos].trim().to_string();
            if subject.len() < 2 {
                continue;
            }
            let goal = s[pos + pat_len..].trim().to_string();
            return Some(KnowledgeUnit::Purpose {
                subject,
                goal,
                context: None,
                raw_text: s.to_string(),
            });
        }
    }

    // "The purpose of X is Y" / "The role of X is Y"
    for prefix in &["the purpose of ", "the role of ", "the goal of "] {
        if lower.starts_with(prefix) {
            if let Some(is_pos) = lower[prefix.len()..].find(" is ") {
                let subj = s[prefix.len()..prefix.len() + is_pos].trim().to_string();
                let goal = s[prefix.len() + is_pos + 4..].trim().to_string();
                if !subj.is_empty() && !goal.is_empty() {
                    return Some(KnowledgeUnit::Purpose {
                        subject: subj,
                        goal,
                        context: None,
                        raw_text: s.to_string(),
                    });
                }
            }
        }
    }

    None
}

// ── Example ───────────────────────────────────────────────────────────────────

fn try_extract_example(s: &str) -> Option<KnowledgeUnit> {
    let lower = s.to_lowercase();

    // "For example, ..." / "For instance, ..."
    for prefix in &["for example, ", "for instance, ", "e.g., ", "e.g. "] {
        if lower.starts_with(prefix) {
            let instance = s[prefix.len()..].trim().to_string();
            return Some(KnowledgeUnit::Example {
                concept: String::new(),
                instance,
                explanation: None,
                raw_text: s.to_string(),
            });
        }
    }

    // "X is an example of Y"
    if let Some(pos) = lower.find(" is an example of ") {
        let instance = s[..pos].trim().to_string();
        let concept = s[pos + 18..].trim().to_string();
        return Some(KnowledgeUnit::Example {
            concept,
            instance,
            explanation: None,
            raw_text: s.to_string(),
        });
    }

    // "X, such as Y" / "X such as Y"
    if let Some(pos) = lower.find(" such as ") {
        let concept = s[..pos].trim().to_string();
        let instance = s[pos + 9..].trim().to_string();
        if !concept.is_empty() && !instance.is_empty() {
            return Some(KnowledgeUnit::Example {
                concept,
                instance,
                explanation: None,
                raw_text: s.to_string(),
            });
        }
    }

    None
}

// ── Contrast ──────────────────────────────────────────────────────────────────

fn try_extract_contrast(s: &str) -> Option<KnowledgeUnit> {
    let lower = s.to_lowercase();

    // "X differs from Y [by/in Z]"
    if let Some(pos) = lower.find(" differs from ") {
        let subject_a = s[..pos].trim().to_string();
        let after = &s[pos + 14..]; // len(" differs from ") = 14
        let after_lower = after.to_lowercase();

        let (subject_b, dimension) = if let Some(by_pos) = after_lower.find(" by ") {
            (after[..by_pos].trim().to_string(), after[by_pos + 4..].trim().to_string())
        } else if let Some(in_pos) = after_lower.find(" in ") {
            (after[..in_pos].trim().to_string(), after[in_pos + 4..].trim().to_string())
        } else {
            (after.trim().to_string(), "general".to_string())
        };

        if !subject_a.is_empty() && !subject_b.is_empty() {
            return Some(KnowledgeUnit::Contrast {
                subject_a,
                subject_b,
                dimension,
                comparison: s.to_string(),
                raw_text: s.to_string(),
            });
        }
    }

    // "unlike X, Y" — subject_a is after "unlike", subject_b is before
    if let Some(pos) = lower.find(" unlike ") {
        let subject_b = s[..pos].trim().to_string();
        let after = &s[pos + 8..]; // len(" unlike ") = 8
        // subject_a is the first phrase before a comma
        let subject_a = after
            .split(',')
            .next()
            .unwrap_or(after)
            .trim()
            .to_string();
        return Some(KnowledgeUnit::Contrast {
            subject_a,
            subject_b,
            dimension: "general".to_string(),
            comparison: s.to_string(),
            raw_text: s.to_string(),
        });
    }

    // "X, whereas Y" / "whereas X, Y"
    if let Some(pos) = lower.find(" whereas ") {
        let subject_a = s[..pos].trim().to_string();
        let subject_b = s[pos + 9..].split(',').next().unwrap_or("").trim().to_string();
        return Some(KnowledgeUnit::Contrast {
            subject_a,
            subject_b,
            dimension: "general".to_string(),
            comparison: s.to_string(),
            raw_text: s.to_string(),
        });
    }

    // "however," — single-subject contrast (no subjects extractable)
    if lower.contains(" however,") || lower.starts_with("however,") {
        return Some(KnowledgeUnit::Contrast {
            subject_a: String::new(),
            subject_b: String::new(),
            dimension: "general".to_string(),
            comparison: s.to_string(),
            raw_text: s.to_string(),
        });
    }

    None
}

// ── Causal ────────────────────────────────────────────────────────────────────

fn try_extract_causal(s: &str) -> Option<KnowledgeUnit> {
    let lower = s.to_lowercase();

    // Forward-causal: "X causes Y" / "X leads to Y" etc.
    let forward_patterns: &[(&str, usize)] = &[
        (" causes ",      8),
        (" leads to ",   10),
        (" results in ", 12),
        (" produces ",    9),
        (" triggers ",   10),
        (" creates ",     9),
    ];
    for &(pattern, pat_len) in forward_patterns {
        if let Some(pos) = lower.find(pattern) {
            let cause = s[..pos].trim().to_string();
            let effect = s[pos + pat_len..].trim().to_string();
            if !cause.is_empty() && !effect.is_empty() {
                return Some(KnowledgeUnit::Causal {
                    cause,
                    effect,
                    condition: None,
                    raw_text: s.to_string(),
                });
            }
        }
    }

    // "therefore X" / "thus X" / "consequently X" — effect follows
    for marker in &[" therefore ", " thus ", " consequently ", " hence "] {
        if let Some(pos) = lower.find(marker) {
            let cause = s[..pos].trim().to_string();
            let effect = s[pos + marker.len()..].trim().to_string();
            if !cause.is_empty() && !effect.is_empty() {
                return Some(KnowledgeUnit::Causal {
                    cause,
                    effect,
                    condition: None,
                    raw_text: s.to_string(),
                });
            }
        }
    }

    // "due to X, Y" / "because of X, Y"
    for prefix in &["due to ", "because of "] {
        if lower.starts_with(prefix) {
            if let Some(comma_pos) = s.find(',') {
                let cause = s[prefix.len()..comma_pos].trim().to_string();
                let effect = s[comma_pos + 1..].trim().to_string();
                if !cause.is_empty() && !effect.is_empty() {
                    return Some(KnowledgeUnit::Causal {
                        cause,
                        effect,
                        condition: None,
                        raw_text: s.to_string(),
                    });
                }
            }
        }
    }

    None
}
