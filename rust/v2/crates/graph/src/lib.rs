// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! graph v2 — typed knowledge graph.
//!
//! Key differences from v1:
//! - Nodes store typed `KnowledgeUnit` instead of a raw pattern `Vec<f32>`
//! - Bonds carry a `BondType` enum instead of a free-form label string
//! - `semantic_embedding: Option<Vec<f32>>` lives alongside the KU for similarity
//! - `typed_recall()` lets callers filter traversal by relation kind

pub mod bond;
pub mod knowledge_unit;
pub mod network;
pub mod node;

pub use bond::{BondType, TypedBond};
pub use knowledge_unit::{KnowledgeUnit, KuKind};
pub use network::{ActivatedNode, AssociativeNetwork, RecallResult, TraversalHop};
pub use node::{Confidence, KnowledgeNode};
