// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Conversation Persistence
//!
//! Stores chat history across restarts. Each conversation session
//! is saved as a separate JSON file in `brain/conversations/`.
//! An index file tracks metadata for fast listing without loading
//! every session.

use std::path::PathBuf;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

/// Roles in a conversation.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// A single chat message.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    #[serde(default)]
    pub images: Vec<String>,
    /// (seed_hex, label, contribution_pct)
    #[serde(default)]
    pub sources: Vec<(String, String, f32)>,
    pub timestamp: DateTime<Utc>,
}

/// A conversation session with its messages.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConversationSession {
    pub id: String,
    pub name: String,
    pub messages: Vec<ChatMessage>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Lightweight metadata for the index (no messages loaded).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: String,
    pub name: String,
    pub message_count: usize,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Manages conversation persistence on disk.
pub struct ConversationStore {
    /// Metadata index — always in memory.
    index: Vec<SessionMeta>,
    /// Currently active session ID.
    active_session_id: Option<String>,
    /// Root directory: `{brain_storage}/conversations/`
    storage_path: PathBuf,
}

impl Default for ConversationStore {
    fn default() -> Self {
        ConversationStore {
            index: Vec::new(),
            active_session_id: None,
            storage_path: PathBuf::new(),
        }
    }
}

impl ConversationStore {
    /// Initialize from a brain storage directory.
    /// Loads the index; sessions are lazy-loaded on access.
    pub fn new(brain_storage: &PathBuf) -> Self {
        let storage_path = brain_storage.join("conversations");
        let _ = std::fs::create_dir_all(&storage_path);

        let index = Self::load_index(&storage_path);

        ConversationStore {
            index,
            active_session_id: None,
            storage_path,
        }
    }

    // ─── Public API ─────────────────────────────────────

    /// List all conversations (metadata only, no messages).
    pub fn list(&self) -> Vec<SessionMeta> {
        let mut sorted = self.index.clone();
        sorted.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        sorted
    }

    /// Create a new conversation session. Returns its ID.
    pub fn new_session(&mut self, name: Option<&str>) -> String {
        let now = Utc::now();
        let id = format!("{}", now.timestamp_millis());

        let session = ConversationSession {
            id: id.clone(),
            name: name.unwrap_or("New Chat").to_string(),
            messages: Vec::new(),
            created_at: now,
            updated_at: now,
        };

        // Write empty session file
        let _ = self.write_session(&session);

        // Add to index
        self.index.push(SessionMeta {
            id: id.clone(),
            name: session.name.clone(),
            message_count: 0,
            created_at: now,
            updated_at: now,
        });
        let _ = self.save_index();

        self.active_session_id = Some(id.clone());
        id
    }

    /// Get a full conversation session (lazy-loads from disk).
    pub fn get_session(&self, id: &str) -> Option<ConversationSession> {
        let path = self.session_path(id);
        if !path.exists() {
            return None;
        }
        let data = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&data).ok()
    }

    /// Set the active session.
    pub fn set_active(&mut self, id: &str) {
        self.active_session_id = Some(id.to_string());
    }

    /// Get active session ID.
    pub fn active_id(&self) -> Option<&str> {
        self.active_session_id.as_deref()
    }

    /// Add a message to a session. Returns the message ID.
    pub fn add_message(
        &mut self,
        session_id: &str,
        role: MessageRole,
        content: &str,
        images: Vec<String>,
        sources: Vec<(String, String, f32)>,
    ) -> Option<String> {
        let mut session = self.get_session(session_id)?;
        let now = Utc::now();

        let msg_id = format!("{}_{}", session_id, now.timestamp_millis());
        let msg = ChatMessage {
            id: msg_id.clone(),
            role,
            content: content.to_string(),
            images,
            sources,
            timestamp: now,
        };

        session.messages.push(msg);
        session.updated_at = now;

        // Auto-name from first user message
        if session.messages.len() == 1 {
            let name: String = content.chars().take(50).collect();
            session.name = if name.len() < content.len() {
                format!("{}...", name)
            } else {
                name
            };
        }

        let _ = self.write_session(&session);

        // Update index
        if let Some(meta) = self.index.iter_mut().find(|m| m.id == session_id) {
            meta.message_count = session.messages.len();
            meta.updated_at = now;
            meta.name = session.name.clone();
        }
        let _ = self.save_index();

        Some(msg_id)
    }

    /// Rename a conversation.
    pub fn rename(&mut self, session_id: &str, new_name: &str) -> bool {
        let mut session = match self.get_session(session_id) {
            Some(s) => s,
            None => return false,
        };

        session.name = new_name.to_string();
        session.updated_at = Utc::now();
        let _ = self.write_session(&session);

        if let Some(meta) = self.index.iter_mut().find(|m| m.id == session_id) {
            meta.name = new_name.to_string();
            meta.updated_at = session.updated_at;
        }
        let _ = self.save_index();
        true
    }

    /// Delete a conversation.
    pub fn delete(&mut self, session_id: &str) -> bool {
        let path = self.session_path(session_id);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
        let before = self.index.len();
        self.index.retain(|m| m.id != session_id);
        let removed = self.index.len() < before;
        if removed {
            let _ = self.save_index();
            if self.active_session_id.as_deref() == Some(session_id) {
                self.active_session_id = None;
            }
        }
        removed
    }

    // ─── Internals ──────────────────────────────────────

    fn session_path(&self, id: &str) -> PathBuf {
        self.storage_path.join(format!("session_{}.json", id))
    }

    fn index_path(&self) -> PathBuf {
        self.storage_path.join("conversations_index.json")
    }

    fn write_session(&self, session: &ConversationSession) -> Result<(), String> {
        let path = self.session_path(&session.id);
        let json = serde_json::to_string_pretty(session)
            .map_err(|e| format!("Serialize session: {}", e))?;
        std::fs::write(&path, json)
            .map_err(|e| format!("Write session: {}", e))?;
        Ok(())
    }

    fn save_index(&self) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&self.index)
            .map_err(|e| format!("Serialize index: {}", e))?;
        std::fs::write(self.index_path(), json)
            .map_err(|e| format!("Write index: {}", e))?;
        Ok(())
    }

    fn load_index(storage_path: &PathBuf) -> Vec<SessionMeta> {
        let path = storage_path.join("conversations_index.json");
        if !path.exists() {
            return Vec::new();
        }
        let data = match std::fs::read_to_string(&path) {
            Ok(d) => d,
            Err(_) => return Vec::new(),
        };
        serde_json::from_str(&data).unwrap_or_default()
    }
}
