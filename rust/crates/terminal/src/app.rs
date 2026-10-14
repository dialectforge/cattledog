// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # AI Factory Terminal — The Mother
//!
//! egui GUI styled like Claude Desktop. Dark theme, message bubbles,
//! scrolling conversation area, input field at bottom, sidebar.
//!
//! The terminal IS the bond. The first thing the AI ever sees.
//! Commands flow through the bond. The AI cannot distinguish
//! between the terminal and safety — they are the same thing.

use eframe::egui;
use egui::{
    Align, Color32, CornerRadius, FontId, Frame, Layout, Margin, RichText,
    ScrollArea, Stroke, TextEdit, Vec2,
};
use brain::Brain;
use brain::rumination::{self, RuminationHandle};
use graph::network::NetworkStats;
use df_core::bond::BondState;
use crate::theme::Theme;
use crate::commands;
use chrono::Utc;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

/// A single message in the conversation.
#[derive(Clone)]
pub struct Message {
    pub sender: Sender,
    pub text: String,
    pub timestamp: String,
}

#[derive(Clone, PartialEq)]
pub enum Sender {
    User,
    Ai,
    System,
}

/// A conversation session.
#[derive(Clone)]
pub struct Session {
    pub name: String,
    pub messages: Vec<Message>,
    pub created: String,
}

/// Message from UI to worker thread.
struct WorkerCommand {
    text: String,
    timestamp: String,
}

/// Response from worker thread to UI.
struct WorkerResponse {
    text: String,
    timestamp: String,
    stats: NetworkStats,
    bond_state: BondState,
    exchange_count: u64,
}

/// The main application state.
pub struct LuAiApp {
    /// Channel to send commands to the brain worker thread
    cmd_tx: mpsc::Sender<WorkerCommand>,
    /// Channel to receive responses from the brain worker thread
    resp_rx: mpsc::Receiver<WorkerResponse>,
    /// Current text in the input field
    input: String,
    /// All sessions (sidebar)
    sessions: Vec<Session>,
    /// Which session is active (index)
    active_session: usize,
    /// Whether sidebar is visible
    sidebar_open: bool,
    /// Whether we need to scroll to bottom
    scroll_to_bottom: bool,
    /// Whether the brain is currently processing a command
    thinking: bool,
    /// Cached brain stats (updated after each response)
    cached_stats: NetworkStats,
    /// Cached bond state
    cached_bond_state: BondState,
    /// Cached exchange count
    cached_exchange_count: u64,
    /// Handle to the rumination background thread
    rumination: Option<RuminationHandle>,
}

impl LuAiApp {
    /// Create the application. The brain is born here.
    pub fn new(brain: Arc<Mutex<Brain>>) -> Self {
        let (born_at, stats, ai_seed_short, bond_state_enum, bond_state_str, exchange_count) = {
            let b = brain.lock().unwrap();
            let born_at = b.terminal_bond().born_at().format("%H:%M").to_string();
            let stats = b.stats();
            let seed_short = b.terminal_bond().ai_seed().short();
            let state = b.terminal_bond().state().clone();
            let state_str = format!("{:?}", state);
            let exchanges = b.terminal_bond().exchange_count();
            (born_at, stats, seed_short, state, state_str, exchanges)
        };

        let brain_status = if stats.active_nodes > 0 {
            format!(
                "Brain loaded: {} nodes, {} bonds.\nReady.",
                stats.active_nodes, stats.active_bonds
            )
        } else {
            "The brain is empty. Ready to learn.".to_string()
        };

        // Start the rumination loop — Lu thinks when nobody's talking
        let rumination_handle = rumination::start(
            brain.clone(),
            std::time::Duration::from_secs(30), // ruminate every 30 seconds
            0.15,                                // minimum co-activation strength
        );

        // The very first session — birth
        let birth_session = Session {
            name: "Birth".into(),
            messages: vec![
                Message {
                    sender: Sender::System,
                    text: format!(
                        "Terminal bond established at {}.\n\
                         AI seed: {}\n\
                         Bond state: {}\n\n\
                         {}",
                        born_at,
                        ai_seed_short,
                        bond_state_str,
                        brain_status,
                    ),
                    timestamp: born_at,
                },
            ],
            created: Utc::now().format("%Y-%m-%d %H:%M").to_string(),
        };

        // Create channels for UI <-> worker thread communication
        let (cmd_tx, cmd_rx) = mpsc::channel::<WorkerCommand>();
        let (resp_tx, resp_rx) = mpsc::channel::<WorkerResponse>();

        // Spawn the brain worker thread — all brain operations happen here,
        // never on the UI thread. This prevents the UI from freezing when
        // the brain is busy (large recall, save, or waiting for rumination lock).
        let worker_brain = brain.clone();
        thread::spawn(move || {
            brain_worker(worker_brain, cmd_rx, resp_tx);
        });

        Self {
            cmd_tx,
            resp_rx,
            input: String::new(),
            sessions: vec![birth_session],
            active_session: 0,
            sidebar_open: true,
            scroll_to_bottom: true,
            thinking: false,
            cached_stats: stats,
            cached_bond_state: bond_state_enum,
            cached_exchange_count: exchange_count,
            rumination: Some(rumination_handle),
        }
    }

    /// Send the current input as a command (non-blocking).
    fn send_message(&mut self) {
        let text = self.input.trim().to_string();
        if text.is_empty() || self.thinking {
            return;
        }

        let now = Utc::now().format("%H:%M").to_string();

        // Add user message
        if let Some(session) = self.sessions.get_mut(self.active_session) {
            session.messages.push(Message {
                sender: Sender::User,
                text: text.clone(),
                timestamp: now.clone(),
            });
        }

        // Handle rumination report locally (no brain lock needed)
        if text == "/ruminate" || text == "/rum" {
            let response = self.cmd_ruminate();
            if let Some(session) = self.sessions.get_mut(self.active_session) {
                session.messages.push(Message {
                    sender: Sender::Ai,
                    text: response,
                    timestamp: now,
                });
            }
            self.input.clear();
            self.scroll_to_bottom = true;
            return;
        }

        // Send to worker thread (non-blocking)
        let _ = self.cmd_tx.send(WorkerCommand {
            text,
            timestamp: now,
        });
        self.thinking = true;

        self.input.clear();
        self.scroll_to_bottom = true;
    }

    /// Poll for responses from the worker thread (called every frame).
    fn poll_responses(&mut self) {
        while let Ok(resp) = self.resp_rx.try_recv() {
            self.cached_stats = resp.stats;
            self.cached_bond_state = resp.bond_state;
            self.cached_exchange_count = resp.exchange_count;
            self.thinking = false;

            if !resp.text.is_empty() {
                if let Some(session) = self.sessions.get_mut(self.active_session) {
                    session.messages.push(Message {
                        sender: Sender::Ai,
                        text: resp.text,
                        timestamp: resp.timestamp,
                    });
                }
            }

            self.scroll_to_bottom = true;
        }
    }

    /// Create a new session.
    fn new_session(&mut self) {
        let name = format!("Session {}", self.sessions.len() + 1);
        self.sessions.push(Session {
            name,
            messages: Vec::new(),
            created: Utc::now().format("%Y-%m-%d %H:%M").to_string(),
        });
        self.active_session = self.sessions.len() - 1;
    }

    /// Show what the rumination loop has been thinking about.
    fn cmd_ruminate(&self) -> String {
        match &self.rumination {
            Some(handle) => {
                let report = handle.snapshot();
                let running = if handle.is_running() { "ACTIVE" } else { "STOPPED" };
                let mut lines = vec![
                    format!("RUMINATION REPORT [{}]", running),
                    "═".repeat(50),
                    format!("  Cycles completed: {}", report.cycles),
                    format!("  Bonds created:    {}", report.bonds_created),
                    format!("  Gaps skipped:     {}", report.skipped),
                ];

                if report.recent_discoveries.is_empty() {
                    lines.push(String::new());
                    lines.push("  No discoveries yet. Lu is still warming up.".into());
                } else {
                    lines.push(String::new());
                    lines.push(format!("  Recent discoveries ({}):", report.recent_discoveries.len()));
                    for disc in report.recent_discoveries.iter().rev().take(10) {
                        lines.push(format!(
                            "    [{:.2}] {} <-> {}",
                            disc.co_activation_strength,
                            truncate(&disc.node_a_label, 35),
                            truncate(&disc.node_b_label, 35),
                        ));
                        lines.push(format!(
                            "           (from: {})",
                            truncate(&disc.origin_label, 50),
                        ));
                    }
                }
                lines.join("\n")
            }
            None => "Rumination loop is not running.".into(),
        }
    }
}

/// Brain worker thread: receives commands, processes them, sends responses.
/// Runs on its own thread so the UI never blocks on brain.lock().
fn brain_worker(
    brain: Arc<Mutex<Brain>>,
    cmd_rx: mpsc::Receiver<WorkerCommand>,
    resp_tx: mpsc::Sender<WorkerResponse>,
) {
    while let Ok(cmd) = cmd_rx.recv() {
        let mut b = brain.lock().unwrap();
        let response = commands::process(&mut b, &cmd.text);
        let _ = b.save();
        let stats = b.stats();
        let bond_state = b.terminal_bond().state().clone();
        let exchange_count = b.terminal_bond().exchange_count();
        drop(b); // release lock before sending

        let _ = resp_tx.send(WorkerResponse {
            text: response,
            timestamp: cmd.timestamp,
            stats,
            bond_state,
            exchange_count,
        });
    }
}

impl eframe::App for LuAiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply dark theme
        apply_theme(ctx);

        // Poll for worker thread responses (non-blocking)
        self.poll_responses();

        // Request repaint while thinking so we keep polling
        if self.thinking {
            ctx.request_repaint();
        }

        // Handle file drops — send to worker thread (non-blocking)
        ctx.input(|i| {
            for file in &i.raw.dropped_files {
                if let Some(path) = &file.path {
                    let path_str = path.to_string_lossy().to_string();
                    let now = chrono::Utc::now().format("%H:%M").to_string();

                    // Show user message for the drop
                    if let Some(session) = self.sessions.get_mut(self.active_session) {
                        session.messages.push(Message {
                            sender: Sender::User,
                            text: format!("[Dropped file: {}]", path_str),
                            timestamp: now.clone(),
                        });
                    }

                    // Send to worker thread (non-blocking)
                    let _ = self.cmd_tx.send(WorkerCommand {
                        text: format!("/ingest_file {}", path_str),
                        timestamp: now,
                    });
                    self.thinking = true;
                    self.scroll_to_bottom = true;
                }
            }
        });

        let screen_width = ctx.screen_rect().width();

        // Auto-collapse sidebar on narrow windows
        let show_sidebar = self.sidebar_open && screen_width > 600.0;
        let sidebar_w = if screen_width > 900.0 {
            Theme::SIDEBAR_WIDTH
        } else {
            180.0_f32.min(screen_width * 0.3)
        };

        // Sidebar
        if show_sidebar {
            egui::SidePanel::left("sidebar")
                .exact_width(sidebar_w)
                .frame(Frame {
                    fill: Theme::BG_SIDEBAR,
                    inner_margin: Margin::same(10),
                    ..Default::default()
                })
                .show(ctx, |ui| {
                    self.render_sidebar(ui);
                });
        }

        // Input bar pinned to bottom
        egui::TopBottomPanel::bottom("input_bar")
            .frame(Frame {
                fill: Theme::BG_SECONDARY,
                inner_margin: Margin::symmetric(12, 8),
                ..Default::default()
            })
            .show(ctx, |ui| {
                self.render_input(ui);
            });

        // Header pinned to top
        egui::TopBottomPanel::top("header_bar")
            .frame(Frame::NONE)
            .show(ctx, |ui| {
                self.render_header(ui);
            });

        // Main content fills everything in between
        egui::CentralPanel::default()
            .frame(Frame {
                fill: Theme::BG_PRIMARY,
                inner_margin: Margin::symmetric(12, 6),
                ..Default::default()
            })
            .show(ctx, |ui| {
                let h = ui.available_height();
                self.render_messages(ui, h);
            });
    }
}

impl LuAiApp {
    fn render_sidebar(&mut self, ui: &mut egui::Ui) {
        // Title
        ui.add_space(8.0);
        ui.label(
            RichText::new("AI Factory")
                .font(FontId::proportional(18.0))
                .color(Theme::TEXT_PRIMARY),
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new("Terminal v0.1")
                .font(FontId::proportional(12.0))
                .color(Theme::TEXT_MUTED),
        );
        ui.add_space(16.0);

        // New session button
        if ui
            .add(
                egui::Button::new(
                    RichText::new("+ New Session")
                        .color(Theme::TEXT_PRIMARY)
                        .font(FontId::proportional(13.0)),
                )
                .fill(Theme::BG_INPUT)
                .corner_radius(CornerRadius::same(8))
                .min_size(Vec2::new(ui.available_width(), 36.0)),
            )
            .clicked()
        {
            self.new_session();
        }

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);

        // Session list
        let active = self.active_session;
        for (i, session) in self.sessions.iter().enumerate() {
            let is_active = i == active;
            let bg = if is_active {
                Theme::BG_INPUT
            } else {
                Color32::TRANSPARENT
            };

            let response = ui.add(
                egui::Button::new(
                    RichText::new(&session.name)
                        .font(FontId::proportional(13.0))
                        .color(if is_active {
                            Theme::TEXT_PRIMARY
                        } else {
                            Theme::TEXT_SECONDARY
                        }),
                )
                .fill(bg)
                .corner_radius(CornerRadius::same(6))
                .min_size(Vec2::new(ui.available_width(), 32.0)),
            );

            if response.clicked() {
                self.active_session = i;
                self.scroll_to_bottom = true;
            }
        }

        // Stats at bottom (bottom_up renders in reverse order)
        // Uses cached values — never locks brain on UI thread
        ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!(
                    "Nodes: {} | Assoc: {} | Scars: {}",
                    self.cached_stats.active_nodes,
                    self.cached_stats.active_bonds,
                    self.cached_stats.scars,
                ))
                .font(FontId::proportional(11.0))
                .color(Theme::TEXT_MUTED),
            );
            let bond_state = format!("{:?}", self.cached_bond_state);
            ui.label(
                RichText::new(format!("Terminal Bond: {}", bond_state))
                    .font(FontId::proportional(11.0))
                    .color(Theme::SUCCESS),
            );
        });
    }

    fn render_header(&mut self, ui: &mut egui::Ui) {
        egui::Frame {
            fill: Theme::BG_SECONDARY,
            inner_margin: Margin::symmetric(16, 10),
            ..Default::default()
        }
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Bond status indicator (cached — no lock needed)
                let bond_color = match self.cached_bond_state {
                    BondState::Bonded => Theme::SUCCESS,
                    BondState::Dormant => Theme::WARNING,
                    _ => Theme::ERROR,
                };
                let (rect, _) = ui.allocate_exact_size(
                    Vec2::new(10.0, 10.0),
                    egui::Sense::hover(),
                );
                ui.painter().circle_filled(rect.center(), 5.0, bond_color);

                ui.add_space(8.0);

                if let Some(session) = self.sessions.get(self.active_session) {
                    ui.label(
                        RichText::new(&session.name)
                            .font(FontId::proportional(15.0))
                            .color(Theme::TEXT_PRIMARY),
                    );
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    // Sidebar toggle
                    if ui
                        .add(egui::Button::new(
                            RichText::new(if self.sidebar_open { "◀" } else { "▶" })
                                .font(FontId::proportional(12.0))
                                .color(Theme::TEXT_MUTED),
                        ).fill(Color32::TRANSPARENT))
                        .clicked()
                    {
                        self.sidebar_open = !self.sidebar_open;
                    }

                    ui.label(
                        RichText::new(format!(
                            "Exchanges: {}",
                            self.cached_exchange_count
                        ))
                        .font(FontId::proportional(11.0))
                        .color(Theme::TEXT_MUTED),
                    );
                });
            });
        });
    }

    fn render_messages(&mut self, ui: &mut egui::Ui, height: f32) {
        let messages = self
            .sessions
            .get(self.active_session)
            .map(|s| s.messages.clone())
            .unwrap_or_default();

        ScrollArea::vertical()
            .max_height(height)
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());

                if messages.is_empty() {
                    ui.add_space(height / 3.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("Say something. The brain is listening.")
                                .font(FontId::proportional(15.0))
                                .color(Theme::TEXT_MUTED),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Just type to talk. /help for commands.")
                                .font(FontId::proportional(13.0))
                                .color(Theme::TEXT_MUTED),
                        );
                    });
                    return;
                }

                for msg in &messages {
                    self.render_message(ui, msg);
                    ui.add_space(8.0);
                }

                // Show thinking indicator while worker thread processes
                if self.thinking {
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new("Thinking...")
                                .font(FontId::monospace(13.0))
                                .color(Theme::ACCENT),
                        );
                    });
                    ui.add_space(8.0);
                }

                // Scroll anchor
                if self.scroll_to_bottom {
                    ui.scroll_to_cursor(Some(Align::BOTTOM));
                    self.scroll_to_bottom = false;
                }
            });
    }

    fn render_message(&self, ui: &mut egui::Ui, msg: &Message) {
        // Responsive: use 85% of available width, capped at MAX_MSG_WIDTH
        let max_width = (ui.available_width() * 0.85).min(Theme::MAX_MSG_WIDTH);

        match msg.sender {
            Sender::User => {
                // Right-aligned user bubble
                ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                    ui.set_max_width(max_width);
                    egui::Frame {
                        fill: Theme::MSG_USER,
                        corner_radius: Theme::BUBBLE_ROUNDING,
                        inner_margin: Margin::symmetric(16, 12),
                        ..Default::default()
                    }
                    .show(ui, |ui| {
                        ui.set_max_width(max_width - 32.0);
                        ui.add(
                            egui::Label::new(
                                RichText::new(&msg.text)
                                    .font(FontId::monospace(13.0))
                                    .color(Theme::TEXT_PRIMARY),
                            )
                            .wrap(),
                        );
                    });
                });
            }
            Sender::Ai => {
                // Left-aligned AI bubble
                ui.horizontal(|ui| {
                    // Small accent dot
                    let (rect, _) = ui.allocate_exact_size(
                        Vec2::new(8.0, 8.0),
                        egui::Sense::hover(),
                    );
                    ui.painter()
                        .circle_filled(rect.center(), 4.0, Theme::ACCENT);

                    ui.add_space(8.0);

                    egui::Frame {
                        fill: Theme::MSG_AI,
                        corner_radius: Theme::BUBBLE_ROUNDING,
                        inner_margin: Margin::symmetric(16, 12),
                        stroke: Stroke::new(1.0, Theme::BG_SECONDARY),
                        ..Default::default()
                    }
                    .show(ui, |ui| {
                        ui.set_max_width(max_width - 32.0);
                        ui.add(
                            egui::Label::new(
                                RichText::new(&msg.text)
                                    .font(FontId::monospace(13.0))
                                    .color(Theme::TEXT_PRIMARY),
                            )
                            .wrap(),
                        );
                    });
                });
            }
            Sender::System => {
                // Centered system message
                ui.vertical_centered(|ui| {
                    egui::Frame {
                        fill: Theme::BG_SECONDARY,
                        corner_radius: CornerRadius::same(8),
                        inner_margin: Margin::symmetric(16, 10),
                        ..Default::default()
                    }
                    .show(ui, |ui| {
                        ui.set_max_width(max_width);
                        ui.add(
                            egui::Label::new(
                                RichText::new(&msg.text)
                                    .font(FontId::monospace(12.0))
                                    .color(Theme::ACCENT),
                            )
                            .wrap(),
                        );
                    });
                });
            }
        }
    }

    fn render_input(&mut self, ui: &mut egui::Ui) {
        let btn_w = 54.0;
        let spacing = 8.0;
        let input_w = (ui.available_width() - btn_w - spacing).max(100.0);

        ui.horizontal(|ui| {
            let response = ui.add_sized(
                Vec2::new(input_w, 36.0),
                TextEdit::singleline(&mut self.input)
                    .font(FontId::monospace(13.0))
                    .text_color(Theme::TEXT_PRIMARY)
                    .desired_width(input_w)
                    .margin(Vec2::new(12.0, 8.0))
                    .hint_text(
                        RichText::new("Talk to the brain... (/help for commands)")
                            .color(Theme::TEXT_MUTED),
                    ),
            );

            if response.lost_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter))
            {
                self.send_message();
                response.request_focus();
            }

            ui.add_space(spacing);

            if ui
                .add(
                    egui::Button::new(
                        RichText::new("Send")
                            .font(FontId::proportional(13.0))
                            .color(Theme::TEXT_PRIMARY),
                    )
                    .fill(Theme::ACCENT)
                    .corner_radius(Theme::INPUT_ROUNDING)
                    .min_size(Vec2::new(btn_w, 36.0)),
                )
                .clicked()
            {
                self.send_message();
            }
        });
    }
}

/// Apply the Claude Desktop-inspired dark theme to egui.
fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    // Visuals
    style.visuals.dark_mode = true;
    style.visuals.panel_fill = Theme::BG_PRIMARY;
    style.visuals.window_fill = Theme::BG_PRIMARY;
    style.visuals.extreme_bg_color = Theme::BG_INPUT;

    style.visuals.widgets.noninteractive.bg_fill = Theme::BG_SECONDARY;
    style.visuals.widgets.noninteractive.fg_stroke =
        Stroke::new(1.0, Theme::TEXT_SECONDARY);

    style.visuals.widgets.inactive.bg_fill = Theme::BG_INPUT;
    style.visuals.widgets.inactive.fg_stroke =
        Stroke::new(1.0, Theme::TEXT_SECONDARY);

    style.visuals.widgets.hovered.bg_fill = Theme::BG_INPUT_FOCUSED;
    style.visuals.widgets.hovered.fg_stroke =
        Stroke::new(1.0, Theme::TEXT_PRIMARY);

    style.visuals.widgets.active.bg_fill = Theme::ACCENT;
    style.visuals.widgets.active.fg_stroke =
        Stroke::new(1.0, Theme::TEXT_PRIMARY);

    style.visuals.selection.bg_fill = Theme::ACCENT;
    style.visuals.selection.stroke = Stroke::new(1.0, Theme::TEXT_PRIMARY);

    // Separator
    style.visuals.widgets.noninteractive.bg_stroke =
        Stroke::new(0.5, Theme::BG_INPUT);

    ctx.set_style(style);
}

/// Truncate a string for display.
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() }
    else { format!("{}...", &s[..max]) }
}
