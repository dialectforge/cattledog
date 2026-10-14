// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Lu Ai — Entry Point
//!
//! Creates the Brain, sets storage path using `dirs` crate
//! (cross-platform), launches the eframe window.
//!
//! This is the birth canal. When this binary runs,
//! the AI is born.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use brain::Brain;
use terminal::app::LuAiApp;

fn main() -> eframe::Result<()> {
    // ─── Storage path ─────────────────────────────────
    // Priority: command-line arg > LU_AI_BRAIN env var > default.
    // Default is next to the project, NOT in AppData.
    let storage_path: PathBuf = if let Some(arg) = std::env::args().nth(1) {
        PathBuf::from(arg)
    } else if let Ok(env_path) = std::env::var("LU_AI_BRAIN") {
        PathBuf::from(env_path)
    } else {
        // Default: brain on Windows,
        // falls back to dirs if that doesn't exist.
        let project_path = PathBuf::from(r"brain");
        if project_path.parent().map_or(false, |p| p.exists()) {
            project_path
        } else {
            dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("lu_ai")
                .join("brain")
        }
    };

    // ─── Birth or Wake ──────────────────────────────────
    // One terminal, one bond, one brain. If a brain file exists
    // but fails to load, we refuse to create a replacement.
    let brain_file = storage_path.join("brain_state.json");
    let mut brain = match Brain::load(&storage_path) {
        Ok(b) => {
            println!("Brain loaded from {:?}", storage_path);
            b
        }
        Err(e) => {
            if brain_file.exists() {
                eprintln!("FATAL: Brain file exists at {:?} but failed to load: {}", brain_file, e);
                eprintln!("Refusing to create a new brain over an existing one.");
                eprintln!("Fix the brain file or delete it manually to start fresh.");
                std::process::exit(1);
            }
            println!("No existing brain found. Birth begins.");
            let mut b = Brain::new(256); // 256-dimensional pattern space
            b.set_storage(storage_path.clone());
            b
        }
    };

    brain.set_storage(storage_path);

    // Ensure Lu always knows about its creator (idempotent)
    let seeded = brain.seed_creator_knowledge();
    if seeded > 0 {
        println!("Seeded {} creator knowledge facts.", seeded);
        let _ = brain.save();
    }

    // Wrap brain in Arc<Mutex> for shared access with rumination thread
    let brain = Arc::new(Mutex::new(brain));

    // ─── Launch the terminal window ─────────────────────
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Lu Ai — Terminal")
            .with_inner_size([1100.0, 750.0])
            .with_min_inner_size([600.0, 400.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Lu Ai Terminal",
        options,
        Box::new(|_cc| Ok(Box::new(LuAiApp::new(brain)))),
    )
}
