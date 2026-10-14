// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Associative Memory Graph
//!
//! The knowledge web. Nodes are DF-gated atoms. Bonds are DF-negotiated
//! connections that are themselves atoms. Recall works through spreading
//! activation — not search, not computation. Traversal.
//!
//! "Where is the hammer?" → hammer activates → red hammer next to box →
//! look for box → box near vanity → hammer on floor. Three hops. No index.

pub mod node;
pub mod network;

pub use node::{KnowledgeNode, NodeType, Confidence, Scar};
pub use network::AssociativeNetwork;
