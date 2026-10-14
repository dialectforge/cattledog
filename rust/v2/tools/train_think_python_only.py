#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""
Single-book training launcher: Think Python 2e only.
Starts HTTP server on port 8080, writes progress JSON, then runs train_with_progress.py.
"""

import json, os, subprocess, sys, threading, time, webbrowser
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

TOOLS_DIR      = Path(__file__).parent
DASHBOARD_HTML = TOOLS_DIR / "training_dashboard.html"
PROGRESS_FILE  = TOOLS_DIR / "training_progress.json"
TRAIN_SCRIPT   = TOOLS_DIR / "train_with_progress.py"
LOCK_FILE      = TOOLS_DIR / "training.lock"
PORT           = 8080

THINK_PYTHON_PDF = r"<AI_FACTORY_ROOT>\tools\trainer\uploads\Think_Python_2e.pdf"


class DashboardHandler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args): pass

    def do_GET(self):
        path = self.path.split("?")[0]
        if path in ("/", "/index.html"):
            try:
                content = DASHBOARD_HTML.read_bytes()
                self.send_response(200)
                self.send_header("Content-Type", "text/html; charset=utf-8")
                self.send_header("Content-Length", str(len(content)))
                self.end_headers()
                self.wfile.write(content)
            except FileNotFoundError:
                self.send_response(404); self.end_headers()
        elif path == "/progress":
            try:
                content = PROGRESS_FILE.read_bytes() if PROGRESS_FILE.exists() else \
                          json.dumps({"status": "waiting", "books": []}).encode()
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Access-Control-Allow-Origin", "*")
                self.send_header("Cache-Control", "no-cache")
                self.send_header("Content-Length", str(len(content)))
                self.end_headers()
                self.wfile.write(content)
            except Exception as e:
                self.send_response(500); self.end_headers()
                self.wfile.write(str(e).encode())
        else:
            self.send_response(404); self.end_headers()


_server_ready = threading.Event()
_server_error: list[str] = []

def _run_server():
    try:
        server = HTTPServer(("localhost", PORT), DashboardHandler)
    except OSError as e:
        _server_error.append(str(e))
        _server_ready.set()
        return
    server.daemon_threads = True
    _server_ready.set()
    server.serve_forever()


def main():
    print("=" * 65)
    print("  LU BRAIN v2 — THINK PYTHON 2e TRAINING (single-book test)")
    print("=" * 65)

    # Lock file guard
    if LOCK_FILE.exists():
        try:
            pid = int(LOCK_FILE.read_text().strip())
            import ctypes
            handle = ctypes.windll.kernel32.OpenProcess(0x400, False, pid)
            alive = handle != 0
            if alive: ctypes.windll.kernel32.CloseHandle(handle)
        except Exception:
            alive = False
        if alive:
            print(f"\nERROR: Training already running (PID {pid}). Kill it first.")
            sys.exit(1)
        else:
            print(f"  Stale lock (PID {pid} dead). Removing.")
            LOCK_FILE.unlink(missing_ok=True)

    LOCK_FILE.write_text(str(os.getpid()))

    if not os.path.isfile(THINK_PYTHON_PDF):
        print(f"ERROR: PDF not found: {THINK_PYTHON_PDF}")
        LOCK_FILE.unlink(missing_ok=True)
        sys.exit(1)

    size_mb = os.path.getsize(THINK_PYTHON_PDF) / (1024 * 1024)
    print(f"\n[1/4] PDF: Think_Python_2e.pdf ({size_mb:.1f} MB)  [OK]")

    # Write initial progress state
    initial_state = {
        "status":       "starting",
        "started_at":   time.time(),
        "current_book": 0,
        "books": [{
            "name":       "Think_Python_2e.pdf",
            "path":       THINK_PYTHON_PDF,
            "status":     "queued",
            "paragraphs": 0,
            "ku_count":   0,
            "errors":     0,
            "rate":       0.0,
            "page":       0,
        }],
    }
    with open(PROGRESS_FILE, "w", encoding="utf-8") as f:
        json.dump(initial_state, f, indent=2)
    print(f"\n[2/4] Progress file written: {PROGRESS_FILE}")

    # Start HTTP server
    print(f"\n[3/4] Starting HTTP server on http://localhost:{PORT} ...")
    t = threading.Thread(target=_run_server, daemon=True)
    t.start()
    _server_ready.wait(timeout=5.0)
    if _server_error:
        print(f"\nERROR: Port {PORT} busy: {_server_error[0]}")
        LOCK_FILE.unlink(missing_ok=True)
        sys.exit(1)
    print("  Server up.")
    webbrowser.open(f"http://localhost:{PORT}")
    time.sleep(1.0)

    # Run training
    print(f"\n[4/4] Training on Think_Python_2e.pdf ...")
    cmd = [sys.executable, str(TRAIN_SCRIPT),
           "--progress-file", str(PROGRESS_FILE),
           THINK_PYTHON_PDF]
    try:
        result = subprocess.run(cmd)
    except KeyboardInterrupt:
        print("\nInterrupted.")
    finally:
        LOCK_FILE.unlink(missing_ok=True)

    print(f"\nDone. Dashboard still at http://localhost:{PORT}")
    print("Press Ctrl+C to stop.\n")
    try:
        while True: time.sleep(1)
    except KeyboardInterrupt:
        print("Server stopped.")


if __name__ == "__main__":
    main()
