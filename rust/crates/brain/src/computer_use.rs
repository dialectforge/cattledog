// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Computer Use Engine
//!
//! Gives Lu the ability to execute code, read/write files, and interact
//! with the local system in a sandboxed way. All execution happens in
//! a temporary directory with strict timeout enforcement.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

// ─── Structs ────────────────────────────────────────────────────

/// A block of code ready for execution or display.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CodeBlock {
    /// Language: "python", "rust", "powershell"
    pub language: String,
    /// The source code
    pub code: String,
    /// Human description of what the code does
    pub description: String,
    /// Whether to auto-execute (based on autonomy level)
    pub auto_execute: bool,
}

/// Result of executing a code block.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub files_created: Vec<String>,
    pub timed_out: bool,
}

/// A record of a past execution (for history).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub id: String,
    pub code: CodeBlock,
    pub result: ExecutionResult,
    pub executed_at: DateTime<Utc>,
}

/// The computer use engine — manages sandboxed execution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComputerUseEngine {
    pub enabled: bool,
    pub sandbox_dir: PathBuf,
    pub execution_timeout_secs: u32,
    pub allowed_languages: Vec<String>,
    #[serde(skip)]
    pub history: Vec<ExecutionRecord>,
    #[serde(skip)]
    execution_counter: u64,
}

impl Default for ComputerUseEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputerUseEngine {
    pub fn new() -> Self {
        let sandbox = std::env::temp_dir().join("lu_ai_sandbox");
        Self {
            enabled: true,
            sandbox_dir: sandbox,
            execution_timeout_secs: 30,
            allowed_languages: vec![
                "python".to_string(),
                "rust".to_string(),
                "powershell".to_string(),
            ],
            history: Vec::new(),
            execution_counter: 0,
        }
    }

    /// Ensure the sandbox directory exists.
    fn ensure_sandbox(&self) -> Result<(), String> {
        fs::create_dir_all(&self.sandbox_dir)
            .map_err(|e| format!("Failed to create sandbox dir: {}", e))
    }

    /// Validate that a path is inside the sandbox.
    fn validate_path(&self, path: &Path) -> Result<PathBuf, String> {
        let canonical_sandbox = fs::canonicalize(&self.sandbox_dir)
            .map_err(|e| format!("Cannot resolve sandbox path: {}", e))?;
        let canonical_target = fs::canonicalize(path)
            .or_else(|_| {
                // File may not exist yet — canonicalize the parent
                if let Some(parent) = path.parent() {
                    let parent_canon = fs::canonicalize(parent)
                        .map_err(|e| format!("Cannot resolve parent path: {}", e))?;
                    Ok(parent_canon.join(path.file_name().unwrap_or_default()))
                } else {
                    Err(format!("Invalid path: {:?}", path))
                }
            })?;

        if canonical_target.starts_with(&canonical_sandbox) {
            Ok(canonical_target)
        } else {
            Err(format!(
                "Path escapes sandbox: {:?} is not inside {:?}",
                canonical_target, canonical_sandbox
            ))
        }
    }

    /// Resolve a user-supplied path relative to the sandbox.
    fn resolve_sandbox_path(&self, filename: &str) -> PathBuf {
        let path = Path::new(filename);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.sandbox_dir.join(filename)
        }
    }

    // ─── Code Execution ─────────────────────────────────────

    /// Execute a code block in the sandbox.
    pub fn execute(&mut self, code: &CodeBlock) -> Result<ExecutionResult, String> {
        if !self.enabled {
            return Err("Computer use is disabled".to_string());
        }

        let lang = code.language.to_lowercase();
        if !self.allowed_languages.contains(&lang) {
            return Err(format!(
                "Language '{}' is not allowed. Allowed: {:?}",
                lang, self.allowed_languages
            ));
        }

        self.ensure_sandbox()?;

        let result = match lang.as_str() {
            "python" => self.execute_python(&code.code)?,
            "rust" => self.execute_rust(&code.code)?,
            "powershell" => self.execute_powershell(&code.code)?,
            _ => return Err(format!("Unsupported language: {}", lang)),
        };

        // Record in history
        self.execution_counter += 1;
        let record = ExecutionRecord {
            id: format!("exec-{}", self.execution_counter),
            code: code.clone(),
            result: result.clone(),
            executed_at: Utc::now(),
        };
        self.history.push(record);

        Ok(result)
    }

    fn execute_python(&self, code: &str) -> Result<ExecutionResult, String> {
        let script_path = self.sandbox_dir.join("_lu_script.py");
        fs::write(&script_path, code)
            .map_err(|e| format!("Failed to write Python script: {}", e))?;

        let files_before = self.list_dir_files()?;
        let start = Instant::now();

        let output = Command::new("python")
            .arg(&script_path)
            .current_dir(&self.sandbox_dir)
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .output();

        let duration = start.elapsed();
        let timed_out = duration.as_secs() > self.execution_timeout_secs as u64;

        // Clean up script
        let _ = fs::remove_file(&script_path);

        match output {
            Ok(out) => {
                let files_after = self.list_dir_files()?;
                let new_files = self.diff_files(&files_before, &files_after);

                Ok(ExecutionResult {
                    stdout: String::from_utf8_lossy(&out.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&out.stderr).to_string(),
                    exit_code: out.status.code().unwrap_or(-1),
                    duration_ms: duration.as_millis() as u64,
                    files_created: new_files,
                    timed_out,
                })
            }
            Err(e) => Err(format!("Failed to execute Python: {}", e)),
        }
    }

    fn execute_rust(&self, code: &str) -> Result<ExecutionResult, String> {
        let src_path = self.sandbox_dir.join("_lu_script.rs");
        let exe_name = if cfg!(windows) { "_lu_script.exe" } else { "_lu_script" };
        let exe_path = self.sandbox_dir.join(exe_name);

        fs::write(&src_path, code)
            .map_err(|e| format!("Failed to write Rust source: {}", e))?;

        // Compile
        let compile = Command::new("rustc")
            .arg(&src_path)
            .arg("-o")
            .arg(&exe_path)
            .current_dir(&self.sandbox_dir)
            .output()
            .map_err(|e| format!("Failed to invoke rustc: {}", e))?;

        if !compile.status.success() {
            let stderr = String::from_utf8_lossy(&compile.stderr).to_string();
            let _ = fs::remove_file(&src_path);
            return Ok(ExecutionResult {
                stdout: String::new(),
                stderr: format!("Compilation failed:\n{}", stderr),
                exit_code: compile.status.code().unwrap_or(-1),
                duration_ms: 0,
                files_created: vec![],
                timed_out: false,
            });
        }

        // Run
        let files_before = self.list_dir_files()?;
        let start = Instant::now();

        let output = Command::new(&exe_path)
            .current_dir(&self.sandbox_dir)
            .output();

        let duration = start.elapsed();
        let timed_out = duration.as_secs() > self.execution_timeout_secs as u64;

        // Clean up
        let _ = fs::remove_file(&src_path);
        let _ = fs::remove_file(&exe_path);

        match output {
            Ok(out) => {
                let files_after = self.list_dir_files()?;
                let new_files = self.diff_files(&files_before, &files_after);

                Ok(ExecutionResult {
                    stdout: String::from_utf8_lossy(&out.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&out.stderr).to_string(),
                    exit_code: out.status.code().unwrap_or(-1),
                    duration_ms: duration.as_millis() as u64,
                    files_created: new_files,
                    timed_out,
                })
            }
            Err(e) => Err(format!("Failed to execute Rust binary: {}", e)),
        }
    }

    fn execute_powershell(&self, code: &str) -> Result<ExecutionResult, String> {
        let script_path = self.sandbox_dir.join("_lu_script.ps1");
        fs::write(&script_path, code)
            .map_err(|e| format!("Failed to write PowerShell script: {}", e))?;

        let files_before = self.list_dir_files()?;
        let start = Instant::now();

        let output = Command::new("powershell")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-NoProfile")
            .arg("-File")
            .arg(&script_path)
            .current_dir(&self.sandbox_dir)
            .output();

        let duration = start.elapsed();
        let timed_out = duration.as_secs() > self.execution_timeout_secs as u64;

        // Clean up
        let _ = fs::remove_file(&script_path);

        match output {
            Ok(out) => {
                let files_after = self.list_dir_files()?;
                let new_files = self.diff_files(&files_before, &files_after);

                Ok(ExecutionResult {
                    stdout: String::from_utf8_lossy(&out.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&out.stderr).to_string(),
                    exit_code: out.status.code().unwrap_or(-1),
                    duration_ms: duration.as_millis() as u64,
                    files_created: new_files,
                    timed_out,
                })
            }
            Err(e) => Err(format!("Failed to execute PowerShell: {}", e)),
        }
    }

    // ─── File Operations (Sandboxed) ────────────────────────

    /// Read a file from the sandbox directory.
    pub fn file_read(&self, filename: &str) -> Result<String, String> {
        self.ensure_sandbox()?;
        let path = self.resolve_sandbox_path(filename);
        let validated = self.validate_path(&path)?;

        let mut file = fs::File::open(&validated)
            .map_err(|e| format!("Cannot read file: {}", e))?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(|e| format!("Cannot read file contents: {}", e))?;
        Ok(contents)
    }

    /// Write a file to the sandbox directory.
    pub fn file_write(&self, filename: &str, content: &str) -> Result<String, String> {
        self.ensure_sandbox()?;
        let path = self.resolve_sandbox_path(filename);
        let validated = self.validate_path(&path)?;

        let mut file = fs::File::create(&validated)
            .map_err(|e| format!("Cannot create file: {}", e))?;
        file.write_all(content.as_bytes())
            .map_err(|e| format!("Cannot write file: {}", e))?;

        Ok(validated.to_string_lossy().to_string())
    }

    /// List files in a directory (sandbox only).
    pub fn file_list(&self, subdir: Option<&str>) -> Result<Vec<FileEntry>, String> {
        self.ensure_sandbox()?;
        let dir = match subdir {
            Some(sub) => {
                let path = self.resolve_sandbox_path(sub);
                self.validate_path(&path)?
            }
            None => self.sandbox_dir.clone(),
        };

        let entries = fs::read_dir(&dir)
            .map_err(|e| format!("Cannot list directory: {}", e))?;

        let mut result = Vec::new();
        for entry in entries.flatten() {
            let meta = entry.metadata().ok();
            result.push(FileEntry {
                name: entry.file_name().to_string_lossy().to_string(),
                is_dir: meta.as_ref().map_or(false, |m| m.is_dir()),
                size_bytes: meta.as_ref().map_or(0, |m| m.len()),
            });
        }
        result.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(result)
    }

    // ─── Code Generation ────────────────────────────────────

    /// Detect whether a query is asking for code generation.
    pub fn is_code_request(query: &str) -> bool {
        let q = query.to_lowercase();
        let code_indicators = [
            "write a function",
            "write a script",
            "write a program",
            "write code",
            "create a function",
            "create a script",
            "create a program",
            "create code",
            "implement a",
            "code that",
            "code to",
            "fix this code",
            "fix the code",
            "debug this",
            "how would i implement",
            "how to implement",
            "generate code",
            "generate a function",
            "run this code",
            "execute this",
            "write me a",
            "can you code",
            "python script",
            "rust program",
            "powershell script",
        ];
        code_indicators.iter().any(|ind| q.contains(ind))
    }

    /// Detect the target language from a query.
    pub fn detect_language(query: &str) -> String {
        let q = query.to_lowercase();
        if q.contains("python") || q.contains(".py") {
            "python".to_string()
        } else if q.contains("rust") || q.contains(".rs") {
            "rust".to_string()
        } else if q.contains("powershell") || q.contains(".ps1") || q.contains("ps script") {
            "powershell".to_string()
        } else {
            // Default to Python as most accessible
            "python".to_string()
        }
    }

    /// Build a CodeBlock from a description. The actual code content
    /// should be filled in by the Brain using its knowledge.
    pub fn scaffold_code_block(description: &str, language: &str, code: &str) -> CodeBlock {
        CodeBlock {
            language: language.to_string(),
            code: code.to_string(),
            description: description.to_string(),
            auto_execute: false,
        }
    }

    // ─── History ────────────────────────────────────────────

    /// Get execution history.
    pub fn execution_history(&self) -> &[ExecutionRecord] {
        &self.history
    }

    /// Clear history.
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    // ─── Cleanup ────────────────────────────────────────────

    /// Clean up the sandbox directory.
    pub fn cleanup(&self) -> Result<(), String> {
        if self.sandbox_dir.exists() {
            fs::remove_dir_all(&self.sandbox_dir)
                .map_err(|e| format!("Failed to clean sandbox: {}", e))?;
        }
        Ok(())
    }

    // ─── Internal Helpers ───────────────────────────────────

    fn list_dir_files(&self) -> Result<Vec<String>, String> {
        if !self.sandbox_dir.exists() {
            return Ok(Vec::new());
        }
        let entries = fs::read_dir(&self.sandbox_dir)
            .map_err(|e| format!("Cannot list sandbox: {}", e))?;
        let mut files = Vec::new();
        for entry in entries.flatten() {
            if entry.metadata().map_or(false, |m| m.is_file()) {
                files.push(entry.file_name().to_string_lossy().to_string());
            }
        }
        files.sort();
        Ok(files)
    }

    fn diff_files(&self, before: &[String], after: &[String]) -> Vec<String> {
        let before_set: std::collections::HashSet<&String> = before.iter().collect();
        after
            .iter()
            .filter(|f| !before_set.contains(f))
            // Filter out temp script files
            .filter(|f| !f.starts_with("_lu_script"))
            .cloned()
            .collect()
    }
}

/// A file entry in a directory listing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size_bytes: u64,
}
