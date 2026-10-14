// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Typed Knowledge Units — the semantic atoms of Lu's understanding.
//!
//! Every piece of knowledge stored in the graph is one of these 10 structured
//! variants rather than a raw text blob.  The type tells the narration engine
//! *how* to use the content, not just *what* it says.

use serde::{Deserialize, Serialize};

// ── KuKind ────────────────────────────────────────────────────────────────────

/// Discriminant-only tag for a KnowledgeUnit variant.
/// Used in ResponsePlan::required_ku_kinds and filter predicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KuKind {
    Definition,
    Mechanism,
    Composition,
    Purpose,
    Contrast,
    Causal,
    Example,
    Procedure,
    Code,
    Raw,
    /// A visual perception encoded by the occipital lobe.
    Image,
}

// ── KnowledgeUnit ─────────────────────────────────────────────────────────────

/// The 10 semantic forms knowledge can take inside Lu's graph.
///
/// Each variant carries the fields that make it useful for template-guided
/// narration — the `raw_text` field always holds the original verbatim content
/// so the KU is fully self-describing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KnowledgeUnit {
    /// "X is a Y that Ps." — defines something in terms of its type and properties.
    Definition {
        subject: String,
        /// What broader category this belongs to (e.g., "data structure").
        parent_type: Option<String>,
        /// Key distinguishing properties.
        properties: Vec<String>,
        raw_text: String,
    },

    /// "X works by doing [steps] in order to achieve P."
    Mechanism {
        subject: String,
        /// Ordered list of steps that constitute the mechanism.
        steps: Vec<String>,
        purpose: String,
        raw_text: String,
    },

    /// "X is made of [parts]." — structural/compositional knowledge.
    Composition {
        whole: String,
        parts: Vec<String>,
        /// Optional proportions or roles for each part.
        part_roles: Vec<String>,
        raw_text: String,
    },

    /// "The purpose/goal/role of X is Y [in context C]."
    Purpose {
        subject: String,
        goal: String,
        context: Option<String>,
        raw_text: String,
    },

    /// "X differs from Y along dimension D."
    Contrast {
        subject_a: String,
        subject_b: String,
        /// What dimension is being contrasted (speed, safety, cost…).
        dimension: String,
        /// How A and B compare on that dimension.
        comparison: String,
        raw_text: String,
    },

    /// "Because of C [under condition K], E results."
    Causal {
        cause: String,
        effect: String,
        /// Optional condition that modulates the causal link.
        condition: Option<String>,
        raw_text: String,
    },

    /// "For instance, I is an example of C."
    Example {
        concept: String,
        instance: String,
        /// Why this instance is representative.
        explanation: Option<String>,
        raw_text: String,
    },

    /// "To accomplish T: step 1, step 2, …"
    Procedure {
        name: String,
        steps: Vec<String>,
        prerequisites: Vec<String>,
        expected_outcome: Option<String>,
        raw_text: String,
    },

    /// A code snippet with language tag and description.
    Code {
        language: String,
        snippet: String,
        description: String,
        /// Concepts or APIs this snippet demonstrates.
        demonstrates: Vec<String>,
        raw_text: String,
    },

    /// Unstructured text that hasn't been typed yet — fallback variant.
    Raw {
        text: String,
        source: Option<String>,
    },

    /// A visual perception: an image file encoded by the occipital lobe.
    /// The 384-dim embedding lives in `KnowledgeNode::semantic_embedding`.
    Image {
        /// Absolute file path or unique identifier for the image.
        path: String,
        /// Optional human-readable description or caption.
        #[serde(default)]
        caption: Option<String>,
        /// Text extracted from the image via OCR (Tesseract).
        /// Included in keyword search indexing so image nodes are
        /// findable by their text content.
        #[serde(default)]
        ocr_text: Option<String>,
    },
}

impl KnowledgeUnit {
    /// Returns the kind tag without borrowing the whole variant.
    pub fn kind(&self) -> KuKind {
        match self {
            KnowledgeUnit::Definition { .. }  => KuKind::Definition,
            KnowledgeUnit::Mechanism { .. }   => KuKind::Mechanism,
            KnowledgeUnit::Composition { .. } => KuKind::Composition,
            KnowledgeUnit::Purpose { .. }     => KuKind::Purpose,
            KnowledgeUnit::Contrast { .. }    => KuKind::Contrast,
            KnowledgeUnit::Causal { .. }      => KuKind::Causal,
            KnowledgeUnit::Example { .. }     => KuKind::Example,
            KnowledgeUnit::Procedure { .. }   => KuKind::Procedure,
            KnowledgeUnit::Code { .. }        => KuKind::Code,
            KnowledgeUnit::Raw { .. }         => KuKind::Raw,
            KnowledgeUnit::Image { .. }       => KuKind::Image,
        }
    }

    /// The verbatim source text from which this KU was extracted.
    pub fn raw_text(&self) -> &str {
        match self {
            KnowledgeUnit::Definition  { raw_text, .. } => raw_text,
            KnowledgeUnit::Mechanism   { raw_text, .. } => raw_text,
            KnowledgeUnit::Composition { raw_text, .. } => raw_text,
            KnowledgeUnit::Purpose     { raw_text, .. } => raw_text,
            KnowledgeUnit::Contrast    { raw_text, .. } => raw_text,
            KnowledgeUnit::Causal      { raw_text, .. } => raw_text,
            KnowledgeUnit::Example     { raw_text, .. } => raw_text,
            KnowledgeUnit::Procedure   { raw_text, .. } => raw_text,
            KnowledgeUnit::Code        { raw_text, .. } => raw_text,
            KnowledgeUnit::Raw         { text, .. }     => text,
            KnowledgeUnit::Image       { path, caption, ocr_text } => {
                ocr_text.as_deref()
                    .or_else(|| caption.as_deref())
                    .unwrap_or(path.as_str())
            }
        }
    }

    /// Primary subject of this KU, if it has one.
    pub fn subject(&self) -> Option<&str> {
        match self {
            KnowledgeUnit::Definition  { subject, .. } => Some(subject),
            KnowledgeUnit::Mechanism   { subject, .. } => Some(subject),
            KnowledgeUnit::Composition { whole,   .. } => Some(whole),
            KnowledgeUnit::Purpose     { subject, .. } => Some(subject),
            KnowledgeUnit::Contrast    { subject_a, .. } => Some(subject_a),
            KnowledgeUnit::Causal      { cause,   .. } => Some(cause),
            KnowledgeUnit::Example     { concept, .. } => Some(concept),
            KnowledgeUnit::Procedure   { name,    .. } => Some(name),
            KnowledgeUnit::Code        { description, .. } => Some(description),
            KnowledgeUnit::Raw         { .. }          => None,
            KnowledgeUnit::Image       { caption, .. } => caption.as_deref(),
        }
    }

    /// Human-readable one-liner summary for labels and search indices.
    pub fn label(&self) -> String {
        match self {
            KnowledgeUnit::Definition { subject, parent_type, .. } => {
                if let Some(pt) = parent_type {
                    format!("{} ({})", subject, pt)
                } else {
                    subject.clone()
                }
            }
            KnowledgeUnit::Mechanism { subject, purpose, .. } =>
                format!("{}: {}", subject, purpose),
            KnowledgeUnit::Composition { whole, parts, .. } =>
                format!("{} [{} parts]", whole, parts.len()),
            KnowledgeUnit::Purpose { subject, goal, .. } =>
                format!("{} → {}", subject, goal),
            KnowledgeUnit::Contrast { subject_a, subject_b, dimension, .. } =>
                format!("{} vs {} ({})", subject_a, subject_b, dimension),
            KnowledgeUnit::Causal { cause, effect, .. } =>
                format!("{} → {}", cause, effect),
            KnowledgeUnit::Example { concept, instance, .. } =>
                format!("{} e.g. {}", concept, instance),
            KnowledgeUnit::Procedure { name, steps, .. } =>
                format!("{} [{} steps]", name, steps.len()),
            KnowledgeUnit::Code { language, description, .. } =>
                format!("[{}] {}", language, description),
            KnowledgeUnit::Raw { text, .. } =>
                text.chars().take(60).collect::<String>(),
            KnowledgeUnit::Image { path, caption, ocr_text } => {
                let fname = std::path::Path::new(path).file_name()
                    .map(|f| f.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.clone());
                let ocr_hint = ocr_text.as_deref()
                    .map(|t| format!(" [ocr: {}]", t.chars().take(40).collect::<String>()))
                    .unwrap_or_default();
                if let Some(cap) = caption {
                    format!("[image] {} ({}){}", cap, fname, ocr_hint)
                } else {
                    format!("[image] {}{}", fname, ocr_hint)
                }
            }
        }
    }
}
