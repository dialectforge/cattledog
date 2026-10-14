// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Timestamps for the DF system.

use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

/// A DF-aware timestamp.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DfTimestamp {
    pub utc: DateTime<Utc>,
}

impl DfTimestamp {
    pub fn now() -> Self {
        Self { utc: Utc::now() }
    }

    pub fn elapsed_secs(&self) -> f64 {
        (Utc::now() - self.utc).num_milliseconds() as f64 / 1000.0
    }
}

impl Default for DfTimestamp {
    fn default() -> Self {
        Self::now()
    }
}
