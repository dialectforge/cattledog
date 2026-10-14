// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Code Execution Sandbox — run Python, Rust, and PowerShell in an isolated
//! temporary directory with path canonicalization and configurable timeouts.
//!
//! All executed code runs inside `{TEMP}/lu_ai_sandbox/`.  The sandbox
//! validates that any file-system paths remain within that directory to
//! prevent escape attacks (e.g., `../../etc/passwd`).

use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, Instant};

// ── Constants ─────────────────────────────────────────────────────────────────

pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;

// ── Request / Result ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CodeExecutionRequest {
    /// Language to execute: "python", "rust", or "powershell".
    pub language: String,
    /// Source code to run.
    pub code: String,
    /// Maximum wall-clock time before the process is killed (ms).
    pub timeout_ms: u64,
    /// Optional subdirectory inside the sandbox (relative name only).
    pub working_dir: Option<String>,
}

impl CodeExecutionRequest {
    pub fn new(language: impl Into<String>, code: impl Into<String>) -> Self {
        CodeExecutionRequest {
            language: language.into(),
            code: code.into(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            working_dir: None,
        }
    }

    pub fn with_timeout(mut self, ms: u64) -> Self {
        self.timeout_ms = ms;
        self
    }
}

#[derive(Debug, Clone)]
pub struct CodeExecutionResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    /// Files present in the working directory after execution (relative names).
    pub files_created: Vec<String>,
    pub timed_out: bool,
    pub duration_ms: u64,
}

// ── CodeSandbox ───────────────────────────────────────────────────────────────

pub struct CodeSandbox {
    /// Canonicalized absolute path to the sandbox root.
    pub sandbox_dir: PathBuf,
}

impl CodeSandbox {
    /// Create (or open) the sandbox directory and return a `CodeSandbox`.
    pub fn new() -> Result<Self, String> {
        let temp = std::env::temp_dir();
        let sandbox = temp.join("lu_ai_sandbox");
        std::fs::create_dir_all(&sandbox)
            .map_err(|e| format!("failed to create sandbox: {}", e))?;
        // Canonicalize resolves symlinks and normalises separators.
        let canonical = sandbox
            .canonicalize()
            .map_err(|e| format!("failed to canonicalize sandbox: {}", e))?;
        Ok(CodeSandbox { sandbox_dir: canonical })
    }

    // ── Path validation ───────────────────────────────────────────────────────

    /// Validate that `path` stays within the sandbox.
    ///
    /// For paths that already exist, `canonicalize` is used (resolves symlinks).
    /// For paths that don't exist yet, the path is normalized by stripping `..`
    /// components and the result is checked against the sandbox prefix.
    pub fn validate_path(&self, path: &Path) -> Result<PathBuf, String> {
        if path.exists() {
            let canonical = path
                .canonicalize()
                .map_err(|e| format!("canonicalize failed: {}", e))?;
            if canonical.starts_with(&self.sandbox_dir) {
                return Ok(canonical);
            }
            return Err(format!(
                "path '{}' is outside sandbox '{}'",
                canonical.display(),
                self.sandbox_dir.display()
            ));
        }

        // Path doesn't exist yet — normalize manually.
        let abs = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.sandbox_dir.join(path)
        };

        let normalized = normalize_path_components(&abs);
        if normalized.starts_with(&self.sandbox_dir) {
            Ok(normalized)
        } else {
            Err(format!(
                "path '{}' would escape sandbox '{}'",
                normalized.display(),
                self.sandbox_dir.display()
            ))
        }
    }

    // ── Execution entry point ─────────────────────────────────────────────────

    pub fn execute(&self, req: &CodeExecutionRequest) -> CodeExecutionResult {
        let start = Instant::now();

        // Resolve (or create) the working directory inside the sandbox.
        let work_dir = match &req.working_dir {
            Some(sub) => {
                // Strip any path separators so callers can't sneak in sub-paths.
                let safe_name: String = sub
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                    .collect();
                let dir = self.sandbox_dir.join(safe_name);
                if std::fs::create_dir_all(&dir).is_err() {
                    self.sandbox_dir.clone()
                } else {
                    dir
                }
            }
            None => self.sandbox_dir.clone(),
        };

        match req.language.to_lowercase().as_str() {
            "python" | "python3" => self.run_python(req, &work_dir, start),
            "rust"               => self.run_rust(req, &work_dir, start),
            "powershell" | "ps" | "pwsh" => self.run_powershell(req, &work_dir, start),
            other => CodeExecutionResult {
                stdout: String::new(),
                stderr: format!(
                    "unsupported language '{}'; supported: python, rust, powershell",
                    other
                ),
                exit_code: Some(-1),
                files_created: vec![],
                timed_out: false,
                duration_ms: start.elapsed().as_millis() as u64,
            },
        }
    }

    // ── Internal runners ──────────────────────────────────────────────────────

    fn run_python(&self, req: &CodeExecutionRequest, work_dir: &Path, start: Instant) -> CodeExecutionResult {
        let script_name = format!("script_{}.py", short_id());
        let script_path = work_dir.join(&script_name);

        if let Err(e) = std::fs::write(&script_path, &req.code) {
            return error_result(format!("failed to write script: {}", e), start);
        }

        // Snapshot directory before execution to detect new files.
        let before = dir_files(work_dir);

        // Try python3 first; fall back to python.
        let result = self
            .try_spawn_and_wait("python3", &[script_path.as_os_str()], work_dir, req.timeout_ms, start)
            .unwrap_or_else(|_| {
                self.try_spawn_and_wait("python", &[script_path.as_os_str()], work_dir, req.timeout_ms, start)
                    .unwrap_or_else(|e| error_result(format!("python not found: {}", e), start))
            });

        let _ = std::fs::remove_file(&script_path);
        let after = dir_files(work_dir);
        let created = after.into_iter().filter(|f| !before.contains(f)).collect();

        CodeExecutionResult { files_created: created, ..result }
    }

    fn run_rust(&self, req: &CodeExecutionRequest, work_dir: &Path, start: Instant) -> CodeExecutionResult {
        let id = short_id();
        let src_path = work_dir.join(format!("prog_{}.rs", id));
        #[cfg(windows)]
        let bin_path = work_dir.join(format!("prog_{}.exe", id));
        #[cfg(not(windows))]
        let bin_path = work_dir.join(format!("prog_{}", id));

        if let Err(e) = std::fs::write(&src_path, &req.code) {
            return error_result(format!("failed to write Rust source: {}", e), start);
        }

        // Compile synchronously (compilation doesn't need a timeout on its own).
        let compile = Command::new("rustc")
            .arg(&src_path)
            .arg("-o")
            .arg(&bin_path)
            .current_dir(work_dir)
            .output();

        let _ = std::fs::remove_file(&src_path);

        match compile {
            Err(e) => error_result(format!("rustc not found: {}", e), start),
            Ok(out) if !out.status.success() => CodeExecutionResult {
                stdout: String::new(),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
                exit_code: out.status.code(),
                files_created: vec![],
                timed_out: false,
                duration_ms: start.elapsed().as_millis() as u64,
            },
            Ok(_) => {
                // Run the compiled binary.
                let result = self
                    .try_spawn_and_wait(
                        bin_path.to_str().unwrap_or(""),
                        &[],
                        work_dir,
                        req.timeout_ms,
                        start,
                    )
                    .unwrap_or_else(|e| error_result(format!("failed to run binary: {}", e), start));
                let _ = std::fs::remove_file(&bin_path);
                result
            }
        }
    }

    fn run_powershell(&self, req: &CodeExecutionRequest, work_dir: &Path, start: Instant) -> CodeExecutionResult {
        let script_name = format!("script_{}.ps1", short_id());
        let script_path = work_dir.join(&script_name);

        if let Err(e) = std::fs::write(&script_path, &req.code) {
            return error_result(format!("failed to write script: {}", e), start);
        }

        let before = dir_files(work_dir);

        // On Windows use powershell.exe; on other platforms try pwsh.
        #[cfg(windows)]
        let ps = "powershell";
        #[cfg(not(windows))]
        let ps = "pwsh";

        let script_os = script_path.as_os_str();
        let result = self
            .try_spawn_and_wait_with_args(
                ps,
                &[
                    std::ffi::OsStr::new("-NonInteractive"),
                    std::ffi::OsStr::new("-NoProfile"),
                    std::ffi::OsStr::new("-ExecutionPolicy"),
                    std::ffi::OsStr::new("Bypass"),
                    std::ffi::OsStr::new("-File"),
                    script_os,
                ],
                work_dir,
                req.timeout_ms,
                start,
            )
            .unwrap_or_else(|e| error_result(format!("powershell not found: {}", e), start));

        let _ = std::fs::remove_file(&script_path);
        let after = dir_files(work_dir);
        let created = after.into_iter().filter(|f| !before.contains(f)).collect();

        CodeExecutionResult { files_created: created, ..result }
    }

    // ── Spawn helpers ─────────────────────────────────────────────────────────

    fn try_spawn_and_wait(
        &self,
        program: &str,
        args: &[&std::ffi::OsStr],
        cwd: &Path,
        timeout_ms: u64,
        start: Instant,
    ) -> Result<CodeExecutionResult, String> {
        let mut cmd = Command::new(program);
        for a in args {
            cmd.arg(a);
        }
        let child = cmd
            .current_dir(cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(wait_for_child(child, timeout_ms, start))
    }

    fn try_spawn_and_wait_with_args(
        &self,
        program: &str,
        args: &[&std::ffi::OsStr],
        cwd: &Path,
        timeout_ms: u64,
        start: Instant,
    ) -> Result<CodeExecutionResult, String> {
        let mut cmd = Command::new(program);
        for a in args {
            cmd.arg(a);
        }
        let child = cmd
            .current_dir(cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(wait_for_child(child, timeout_ms, start))
    }
}

// ── Free-standing helpers ─────────────────────────────────────────────────────

/// Wait for a child process with a timeout, killing it if it exceeds the limit.
///
/// The child is moved to a background thread that calls `wait_with_output()`.
/// The calling thread blocks on a channel with `recv_timeout`.  On timeout,
/// the child's PID is used to force-kill the process via an OS command.
fn wait_for_child(
    child: std::process::Child,
    timeout_ms: u64,
    start: Instant,
) -> CodeExecutionResult {
    let child_pid = child.id();
    let (tx, rx) = mpsc::channel::<std::io::Result<std::process::Output>>();

    std::thread::spawn(move || {
        tx.send(child.wait_with_output()).ok();
    });

    match rx.recv_timeout(Duration::from_millis(timeout_ms)) {
        Ok(Ok(output)) => CodeExecutionResult {
            stdout:        String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr:        String::from_utf8_lossy(&output.stderr).into_owned(),
            exit_code:     output.status.code(),
            files_created: vec![],
            timed_out:     false,
            duration_ms:   start.elapsed().as_millis() as u64,
        },
        Ok(Err(e)) => CodeExecutionResult {
            stdout:        String::new(),
            stderr:        format!("process wait error: {}", e),
            exit_code:     Some(-1),
            files_created: vec![],
            timed_out:     false,
            duration_ms:   start.elapsed().as_millis() as u64,
        },
        Err(_) => {
            // Timeout — force-kill the child process.
            kill_by_pid(child_pid);
            CodeExecutionResult {
                stdout:        String::new(),
                stderr:        "execution timed out".to_string(),
                exit_code:     None,
                files_created: vec![],
                timed_out:     true,
                duration_ms:   timeout_ms,
            }
        }
    }
}

/// Kill a process by PID using an OS-level command (platform-aware).
fn kill_by_pid(pid: u32) {
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .output();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("kill")
            .args(["-9", &pid.to_string()])
            .output();
    }
}

/// List files in a directory, returning relative name strings.
fn dir_files(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            if e.path().is_file() {
                e.file_name().into_string().ok()
            } else {
                None
            }
        })
        .collect()
}

/// Normalize path components by consuming `..` and ignoring `.`.
/// Does NOT call `canonicalize` so it works for paths that don't exist yet.
fn normalize_path_components(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => { result.pop(); }
            Component::CurDir    => {}
            other                => result.push(other),
        }
    }
    result
}

/// Short non-cryptographic ID derived from subsecond nanos — good enough for
/// temp-file naming where collisions are harmless.
fn short_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!("{:08x}", nanos)
}

fn error_result(msg: impl Into<String>, start: Instant) -> CodeExecutionResult {
    CodeExecutionResult {
        stdout:        String::new(),
        stderr:        msg.into(),
        exit_code:     Some(-1),
        files_created: vec![],
        timed_out:     false,
        duration_ms:   start.elapsed().as_millis() as u64,
    }
}
