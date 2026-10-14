// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # DialectForge Core
//!
//! The nervous system. The first line of code.
//!
//! DF is not added to AI Factory — DF IS AI Factory.
//! Every node comes into existence through a seed.
//! Every bond is a DF negotiation.
//! Every pattern is generated through DF.
//!
//! This crate is imported by everything else.
//! Nothing in this system exists without DF.
//!
//! Created by John Dean Martin

pub mod seed;
pub mod gate;
pub mod dialect;
pub mod bond;
pub mod timestamp;

// Re-export core types at crate root
pub use seed::Seed;
pub use gate::{SeedGated, GatedData};
pub use dialect::Dialect;
pub use bond::{BondProtocol, BondState, TerminalBond};
pub use timestamp::DfTimestamp;
