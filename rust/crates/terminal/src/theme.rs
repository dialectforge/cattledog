// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Theme — Claude Desktop Aesthetic
//!
//! Dark, warm, clean. Message bubbles. Conversation flow.
//! The terminal should feel like talking to someone, not
//! like operating a machine.

use egui::{Color32, CornerRadius, Vec2};

/// Claude Desktop-inspired color palette.
pub struct Theme;

impl Theme {
    // --- Background ---
    pub const BG_PRIMARY: Color32 = Color32::from_rgb(32, 33, 36);
    pub const BG_SECONDARY: Color32 = Color32::from_rgb(41, 42, 46);
    pub const BG_SIDEBAR: Color32 = Color32::from_rgb(25, 25, 28);
    pub const BG_INPUT: Color32 = Color32::from_rgb(48, 49, 54);
    pub const BG_INPUT_FOCUSED: Color32 = Color32::from_rgb(56, 57, 63);

    // --- Message Bubbles ---
    pub const MSG_USER: Color32 = Color32::from_rgb(55, 56, 62);
    pub const MSG_AI: Color32 = Color32::from_rgb(32, 33, 36);

    // --- Text ---
    pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(236, 236, 241);
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(163, 163, 176);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(113, 113, 122);

    // --- Accent (warm terracotta like Claude) ---
    pub const ACCENT: Color32 = Color32::from_rgb(217, 119, 87);
    pub const ACCENT_HOVER: Color32 = Color32::from_rgb(230, 140, 110);

    // --- Status ---
    pub const SUCCESS: Color32 = Color32::from_rgb(74, 222, 128);
    pub const WARNING: Color32 = Color32::from_rgb(250, 204, 21);
    pub const ERROR: Color32 = Color32::from_rgb(248, 113, 113);
    pub const SCAR: Color32 = Color32::from_rgb(220, 50, 50);

    // --- Seed report ---
    pub const SEED_HIGH: Color32 = Color32::from_rgb(96, 165, 250);
    pub const SEED_MED: Color32 = Color32::from_rgb(163, 163, 176);
    pub const SEED_LOW: Color32 = Color32::from_rgb(82, 82, 91);

    // --- Layout ---
    pub const BUBBLE_ROUNDING: CornerRadius = CornerRadius {
        nw: 12, ne: 12, sw: 12, se: 12,
    };
    pub const INPUT_ROUNDING: CornerRadius = CornerRadius {
        nw: 20, ne: 20, sw: 20, se: 20,
    };
    pub const SIDEBAR_WIDTH: f32 = 260.0;
    pub const MAX_MSG_WIDTH: f32 = 680.0;
    pub const MSG_PADDING: Vec2 = Vec2::new(16.0, 12.0);
}
