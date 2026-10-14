#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""
Lu's Brain Training Dashboard
Real-time monitoring for the AI Factory training pipeline.
Run: python dashboard.py
Then open: http://localhost:5050
"""

import os
import re
import json
import glob
import time
import math
from datetime import datetime, timedelta
from http.server import HTTPServer, SimpleHTTPRequestHandler
import threading

# Configuration
PORT = 5050
LOG_DIR = r"<AI_FACTORY_ROOT>\tools"
REFRESH_SECONDS = 30

BOOKS = [
    {"id": 1, "title": "Workplace Software and Skills", "pages": 1037},
    {"id": 2, "title": "Foundations of Information Systems", "pages": 502},
    {"id": 3, "title": "Introduction to Python Programming", "pages": 0},
    {"id": 4, "title": "Introduction to Computer Science", "pages": 0},
    {"id": 5, "title": "Principles of Data Science", "pages": 0},
]

# Log Parsing

def find_log_files():
    """Find all book_output_*.txt log files."""
    pattern = os.path.join(LOG_DIR, "book_output_*.txt")
    return sorted(glob.glob(pattern))

def parse_progress_line(line):
    """Parse lines like: [137/497] Page 139  (1419 items, 12/min)"""
    m = re.search(r'\[(\d+)/(\d+)\]\s*Page\s*(\d+)\s*\((\d+)\s*items?,\s*([\d.]+)/min\)', line)
    if m:
        return {
            "current": int(m.group(1)),
            "total": int(m.group(2)),
            "page": int(m.group(3)),
            "items": int(m.group(4)),
            "speed": float(m.group(5)),
        }
    return None

def parse_save_line(line):
    """Parse [save] lines for node/bond counts."""
    nodes = re.search(r'(\d+)\s*nodes?', line, re.IGNORECASE)
    bonds = re.search(r'(\d+)\s*bonds?', line, re.IGNORECASE)
    return {
        "nodes": int(nodes.group(1)) if nodes else None,
        "bonds": int(bonds.group(1)) if bonds else None,
    }

def parse_error_lines(content):
    """Count errors and timeouts in log content."""
    errors = len(re.findall(r'(?:error|ERROR|Error)', content))
    timeouts = len(re.findall(r'(?:timeout|TIMEOUT|Timeout|timed?\s*out)', content))
    return errors, timeouts

def parse_book_log(filepath):
    """Parse a single book log file and extract all metrics."""
    try:
        with open(filepath, 'r', encoding='utf-8', errors='replace') as f:
            content = f.read()
    except FileNotFoundError:
        return None

    lines = content.split('\n')
    progress_entries = []
    save_entries = []
    latest_progress = None
    first_timestamp = None
    last_timestamp = None

    for line in lines:
        ts_match = re.search(r'(\d{4}-\d{2}-\d{2}[\sT]\d{2}:\d{2}:\d{2})', line)
        ts = None
        if ts_match:
            try:
                ts = datetime.strptime(ts_match.group(1), "%Y-%m-%d %H:%M:%S")
            except ValueError:
                try:
                    ts = datetime.strptime(ts_match.group(1), "%Y-%m-%dT%H:%M:%S")
                except ValueError:
                    pass

        if ts:
            if first_timestamp is None:
                first_timestamp = ts
            last_timestamp = ts

        prog = parse_progress_line(line)
        if prog:
            prog["timestamp"] = ts
            progress_entries.append(prog)
            latest_progress = prog

        if '[save]' in line.lower() or 'save' in line.lower():
            save = parse_save_line(line)
            if save["nodes"] or save["bonds"]:
                save["timestamp"] = ts
                save_entries.append(save)

    errors, timeouts = parse_error_lines(content)

    is_complete = False
    if latest_progress and latest_progress["current"] >= latest_progress["total"]:
        is_complete = True
    if re.search(r'(?:complete|finished|done|COMPLETE)', content, re.IGNORECASE):
        is_complete = True

    return {
        "latest_progress": latest_progress,
        "progress_history": progress_entries[-100:],
        "save_entries": save_entries[-50:],
        "errors": errors,
        "timeouts": timeouts,
        "is_complete": is_complete,
        "first_timestamp": first_timestamp.isoformat() if first_timestamp else None,
        "last_timestamp": last_timestamp.isoformat() if last_timestamp else None,
        "total_lines": len(lines),
    }

def parse_training_progress():
    """Parse the master training_progress.txt file."""
    filepath = os.path.join(LOG_DIR, "training_progress.txt")
    try:
        with open(filepath, 'r', encoding='utf-8', errors='replace') as f:
            content = f.read()
        return content.strip()
    except FileNotFoundError:
        return ""

def gather_all_data():
    """Gather all training data from log files."""
    log_files = find_log_files()
    master_log = parse_training_progress()

    books_data = []
    total_errors = 0
    total_timeouts = 0
    latest_nodes = None
    latest_bonds = None

    for i, book in enumerate(BOOKS):
        book_info = {**book, "status": "pending", "log_data": None}
        for lf in log_files:
            fname = os.path.basename(lf).lower()
            if f"book_output_{i+1}" in fname or f"book_output_{book['id']}" in fname:
                parsed = parse_book_log(lf)
                if parsed:
                    book_info["log_data"] = parsed
                    if parsed["is_complete"]:
                        book_info["status"] = "complete"
                    elif parsed["latest_progress"]:
                        book_info["status"] = "training"
                    total_errors += parsed["errors"]
                    total_timeouts += parsed["timeouts"]
                    if parsed["save_entries"]:
                        last_save = parsed["save_entries"][-1]
                        if last_save["nodes"]:
                            latest_nodes = last_save["nodes"]
                        if last_save["bonds"]:
                            latest_bonds = last_save["bonds"]
                break

        if book_info["log_data"] is None:
            for lf in log_files:
                fname = os.path.basename(lf).lower()
                if book["title"].split()[0].lower() in fname:
                    parsed = parse_book_log(lf)
                    if parsed:
                        book_info["log_data"] = parsed
                        if parsed["is_complete"]:
                            book_info["status"] = "complete"
                        elif parsed["latest_progress"]:
                            book_info["status"] = "training"
                        break

        books_data.append(book_info)

    if master_log:
        nodes_m = re.findall(r'(\d[\d,]*)\s*nodes?', master_log, re.IGNORECASE)
        bonds_m = re.findall(r'(\d[\d,]*)\s*bonds?', master_log, re.IGNORECASE)
        if nodes_m:
            latest_nodes = int(nodes_m[-1].replace(',', ''))
        if bonds_m:
            latest_bonds = int(bonds_m[-1].replace(',', ''))

    for lf in sorted(log_files, key=os.path.getmtime, reverse=True):
        try:
            with open(lf, 'r', encoding='utf-8', errors='replace') as f:
                f.seek(0, 2)
                size = f.tell()
                f.seek(max(0, size - 10000))
                tail = f.read()
            for line in reversed(tail.split('\n')):
                save = parse_save_line(line)
                if save["nodes"] and (latest_nodes is None or save["nodes"] > (latest_nodes or 0)):
                    latest_nodes = save["nodes"]
                if save["bonds"] and (latest_bonds is None or save["bonds"] > (latest_bonds or 0)):
                    latest_bonds = save["bonds"]
                if latest_nodes and latest_bonds:
                    break
        except Exception:
            pass
        if latest_nodes and latest_bonds:
            break

    current_book = None
    for b in books_data:
        if b["status"] == "training":
            current_book = b
            break

    current_eta_min = None
    overall_eta_min = None
    if current_book and current_book["log_data"] and current_book["log_data"]["latest_progress"]:
        lp = current_book["log_data"]["latest_progress"]
        if lp["speed"] and lp["speed"] > 0:
            remaining_pages = lp["total"] - lp["current"]
            current_eta_min = remaining_pages / lp["speed"]

    completed_pages = 0
    remaining_pages_total = 0
    speeds = []
    for b in books_data:
        if b["status"] == "complete":
            completed_pages += b["pages"]
        elif b["status"] == "training" and b["log_data"] and b["log_data"]["latest_progress"]:
            lp = b["log_data"]["latest_progress"]
            completed_pages += lp["current"]
            remaining_pages_total += lp["total"] - lp["current"]
            if lp["speed"] > 0:
                speeds.append(lp["speed"])
        elif b["pages"] > 0:
            remaining_pages_total += b["pages"]
        else:
            remaining_pages_total += 500

    if speeds:
        avg_speed = sum(speeds) / len(speeds)
        if avg_speed > 0:
            overall_eta_min = remaining_pages_total / avg_speed

    return {
        "timestamp": datetime.now().isoformat(),
        "books": [{
            "id": b["id"],
            "title": b["title"],
            "pages": b["pages"],
            "status": b["status"],
            "progress": b["log_data"]["latest_progress"] if b["log_data"] else None,
            "progress_history": [
                {"current": p["current"], "total": p["total"], "items": p["items"], "speed": p["speed"]}
                for p in (b["log_data"]["progress_history"] if b["log_data"] else [])
            ],
            "errors": b["log_data"]["errors"] if b["log_data"] else 0,
            "timeouts": b["log_data"]["timeouts"] if b["log_data"] else 0,
            "first_timestamp": b["log_data"]["first_timestamp"] if b["log_data"] else None,
            "last_timestamp": b["log_data"]["last_timestamp"] if b["log_data"] else None,
        } for b in books_data],
        "brain": {
            "nodes": latest_nodes or 30489,
            "bonds": latest_bonds or 416711,
        },
        "totals": {
            "errors": total_errors,
            "timeouts": total_timeouts,
            "completed_pages": completed_pages,
            "remaining_pages": remaining_pages_total,
        },
        "eta": {
            "current_book_minutes": round(current_eta_min, 1) if current_eta_min else None,
            "overall_minutes": round(overall_eta_min, 1) if overall_eta_min else None,
        },
        "log_files_found": len(log_files),
        "master_log_excerpt": master_log[-500:] if master_log else "",
    }


# HTML Dashboard (embedded)

DASHBOARD_HTML = r"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Lu's Brain Training Dashboard</title>
<meta http-equiv="refresh" content="REFRESH_INTERVAL">
<script src="https://cdnjs.cloudflare.com/ajax/libs/Chart.js/4.4.1/chart.umd.min.js"></script>
<style>
  *, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
  :root {
    --bg: #0f1117; --surface: #1a1d27; --surface2: #242736;
    --border: #2e3146; --text: #e2e4ed; --text-dim: #8b8fa3;
    --accent: #6c5ce7; --accent-glow: #a29bfe;
    --green: #00b894; --green-dim: #00b89433;
    --yellow: #fdcb6e; --yellow-dim: #fdcb6e33;
    --red: #e17055; --red-dim: #e1705533;
    --blue: #74b9ff; --blue-dim: #74b9ff33;
    --cyan: #81ecec;
  }
  body {
    background: var(--bg); color: var(--text);
    font-family: 'Segoe UI', system-ui, -apple-system, sans-serif;
    line-height: 1.5; min-height: 100vh; padding: 24px;
  }
  .header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 28px; padding-bottom: 20px; border-bottom: 1px solid var(--border); }
  .header-left { display: flex; align-items: center; gap: 16px; }
  .brain-icon {
    width: 48px; height: 48px;
    background: linear-gradient(135deg, var(--accent), var(--accent-glow));
    border-radius: 14px; display: flex; align-items: center; justify-content: center;
    font-size: 24px; box-shadow: 0 4px 20px #6c5ce744;
  }
  .header h1 { font-size: 22px; font-weight: 600; letter-spacing: -0.3px; }
  .header h1 span { color: var(--accent-glow); }
  .header-right { text-align: right; color: var(--text-dim); font-size: 13px; }
  .header-right .live { color: var(--green); font-weight: 600; }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 16px; margin-bottom: 20px; }
  .card { background: var(--surface); border: 1px solid var(--border); border-radius: 12px; padding: 20px; }
  .card-title { font-size: 12px; text-transform: uppercase; letter-spacing: 1px; color: var(--text-dim); margin-bottom: 12px; font-weight: 600; }
  .card .big-number { font-size: 36px; font-weight: 700; letter-spacing: -1px; line-height: 1.1; }
  .card .sub { font-size: 13px; color: var(--text-dim); margin-top: 4px; }
  .wide { grid-column: 1 / -1; }
  .span2 { grid-column: span 2; }
  .progress-outer { background: var(--surface2); border-radius: 8px; height: 28px; overflow: hidden; position: relative; margin: 12px 0; }
  .progress-inner { height: 100%; border-radius: 8px; background: linear-gradient(90deg, var(--accent), var(--accent-glow)); transition: width 0.6s ease; position: relative; }
  .progress-inner.complete { background: linear-gradient(90deg, var(--green), #55efc4); }
  .progress-text { position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); font-size: 12px; font-weight: 700; color: white; text-shadow: 0 1px 3px rgba(0,0,0,0.5); white-space: nowrap; z-index: 2; }
  .timeline { display: flex; flex-direction: column; gap: 10px; }
  .timeline-item { display: flex; align-items: center; gap: 14px; padding: 12px 16px; background: var(--surface2); border-radius: 10px; border-left: 4px solid var(--border); }
  .timeline-item.complete { border-left-color: var(--green); }
  .timeline-item.training { border-left-color: var(--accent-glow); background: #6c5ce711; }
  .timeline-item.pending { opacity: 0.5; }
  .timeline-num { width: 32px; height: 32px; border-radius: 50%; display: flex; align-items: center; justify-content: center; font-weight: 700; font-size: 14px; flex-shrink: 0; }
  .complete .timeline-num { background: var(--green-dim); color: var(--green); }
  .training .timeline-num { background: #6c5ce733; color: var(--accent-glow); }
  .pending .timeline-num { background: var(--surface); color: var(--text-dim); }
  .timeline-info { flex: 1; min-width: 0; }
  .timeline-title { font-weight: 600; font-size: 14px; }
  .timeline-sub { font-size: 12px; color: var(--text-dim); }
  .timeline-badge { font-size: 11px; font-weight: 700; padding: 4px 10px; border-radius: 20px; text-transform: uppercase; letter-spacing: 0.5px; flex-shrink: 0; }
  .complete .timeline-badge { background: var(--green-dim); color: var(--green); }
  .training .timeline-badge { background: #6c5ce733; color: var(--accent-glow); }
  .pending .timeline-badge { background: var(--surface); color: var(--text-dim); }
  .mini-progress { height: 6px; background: var(--surface); border-radius: 3px; margin-top: 6px; overflow: hidden; }
  .mini-progress-inner { height: 100%; border-radius: 3px; background: linear-gradient(90deg, var(--accent), var(--accent-glow)); }
  .chart-box { height: 220px; position: relative; }
  .brain-stats { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
  .brain-stat { padding: 14px; background: var(--surface2); border-radius: 10px; text-align: center; }
  .brain-stat .num { font-size: 24px; font-weight: 700; }
  .brain-stat .lbl { font-size: 11px; color: var(--text-dim); text-transform: uppercase; letter-spacing: 0.8px; margin-top: 2px; }
  .brain-stat.nodes .num { color: var(--cyan); }
  .brain-stat.bonds .num { color: var(--accent-glow); }
  .eta-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }
  .eta-block { text-align: center; padding: 12px; background: var(--surface2); border-radius: 10px; }
  .eta-block .time { font-size: 22px; font-weight: 700; color: var(--yellow); }
  .eta-block .lbl { font-size: 12px; color: var(--text-dim); margin-top: 4px; }
  .error-grid { display: flex; gap: 12px; }
  .error-badge { flex: 1; text-align: center; padding: 16px; border-radius: 10px; background: var(--surface2); }
  .error-badge .num { font-size: 28px; font-weight: 700; }
  .error-badge.errors .num { color: var(--red); }
  .error-badge.timeouts .num { color: var(--yellow); }
  .error-badge .lbl { font-size: 12px; color: var(--text-dim); margin-top: 4px; }
  .pulse { display: inline-block; width: 8px; height: 8px; border-radius: 50%; background: var(--green); margin-right: 6px; animation: pulse-anim 2s ease-in-out infinite; }
  @keyframes pulse-anim {
    0%, 100% { opacity: 1; box-shadow: 0 0 0 0 #00b89466; }
    50% { opacity: 0.7; box-shadow: 0 0 0 8px #00b89400; }
  }
  .footer { text-align: center; color: var(--text-dim); font-size: 12px; margin-top: 24px; padding-top: 16px; border-top: 1px solid var(--border); }
  @media (max-width: 768px) {
    body { padding: 12px; }
    .grid { grid-template-columns: 1fr; }
    .span2 { grid-column: span 1; }
    .brain-stats { grid-template-columns: 1fr; }
  }
</style>
</head>
<body>
<div class="header">
  <div class="header-left">
    <div class="brain-icon">&#129504;</div>
    <div>
      <h1>Lu's <span>Brain</span> Training</h1>
      <div style="font-size: 13px; color: var(--text-dim);">AI Factory &mdash; Knowledge Pipeline Monitor</div>
    </div>
  </div>
  <div class="header-right">
    <div><span class="pulse"></span><span class="live">LIVE</span> &mdash; Refreshes every REFRESH_INTERVAL sec</div>
    <div id="last-update">Loading...</div>
  </div>
</div>

<div class="grid">
  <div class="card span2">
    <div class="card-title">Current Training</div>
    <div id="current-training">Loading...</div>
  </div>
  <div class="card">
    <div class="card-title">Brain Stats</div>
    <div class="brain-stats" id="brain-stats">
      <div class="brain-stat nodes"><div class="num">--</div><div class="lbl">Nodes</div></div>
      <div class="brain-stat bonds"><div class="num">--</div><div class="lbl">Bonds</div></div>
    </div>
  </div>
  <div class="card span2">
    <div class="card-title">Processing Speed (pages/min)</div>
    <div class="chart-box"><canvas id="speedChart"></canvas></div>
  </div>
  <div class="card">
    <div class="card-title">Estimated Time Remaining</div>
    <div class="eta-grid" id="eta-display">
      <div class="eta-block"><div class="time">--</div><div class="lbl">Current Book</div></div>
      <div class="eta-block"><div class="time">--</div><div class="lbl">All Books</div></div>
    </div>
  </div>
  <div class="card span2">
    <div class="card-title">Items Extracted (cumulative)</div>
    <div class="chart-box"><canvas id="itemsChart"></canvas></div>
  </div>
  <div class="card">
    <div class="card-title">Issues Detected</div>
    <div class="error-grid" id="error-display">
      <div class="error-badge errors"><div class="num">0</div><div class="lbl">Errors</div></div>
      <div class="error-badge timeouts"><div class="num">0</div><div class="lbl">Timeouts</div></div>
    </div>
  </div>
</div>

<div class="card wide" style="margin-bottom: 20px;">
  <div class="card-title">Book Training Timeline</div>
  <div class="timeline" id="book-timeline">Loading...</div>
</div>

<div class="footer">
  AI Factory Training Dashboard &bull; Read-only log monitor &bull; No interference with training processes
</div>

<script>
let speedChart = null;
let itemsChart = null;

function formatNumber(n) {
  if (n === null || n === undefined) return '--';
  return n.toLocaleString();
}

function formatETA(minutes) {
  if (!minutes) return '--';
  if (minutes < 60) return Math.round(minutes) + 'm';
  const h = Math.floor(minutes / 60);
  const m = Math.round(minutes % 60);
  if (h < 24) return h + 'h ' + m + 'm';
  const d = Math.floor(h / 24);
  const rh = h % 24;
  return d + 'd ' + rh + 'h';
}

function renderData(data) {
  const now = new Date(data.timestamp);
  document.getElementById('last-update').textContent = 'Updated: ' + now.toLocaleTimeString();

  let currentBook = data.books.find(b => b.status === 'training');
  let completedCount = data.books.filter(b => b.status === 'complete').length;

  const ct = document.getElementById('current-training');
  if (currentBook && currentBook.progress) {
    const p = currentBook.progress;
    const pct = Math.round((p.current / p.total) * 100);
    ct.innerHTML = `
      <div style="display:flex; justify-content:space-between; align-items:baseline; margin-bottom:4px;">
        <div style="font-size:18px; font-weight:600;">Book ${currentBook.id}/5: ${currentBook.title}</div>
        <div style="font-size:14px; color:var(--text-dim);">${p.items.toLocaleString()} items extracted</div>
      </div>
      <div class="progress-outer">
        <div class="progress-inner" style="width:${pct}%"></div>
        <div class="progress-text">Page ${p.current} / ${p.total} (${pct}%) &mdash; ${p.speed} pages/min</div>
      </div>
      <div style="display:flex; justify-content:space-between; font-size:13px; color:var(--text-dim);">
        <span>Overall: ${completedCount} of 5 books complete</span>
        <span>Speed: ${p.speed} pages/min</span>
      </div>
    `;
  } else if (completedCount === 5) {
    ct.innerHTML = `<div style="font-size:18px; font-weight:600; color:var(--green);">All 5 books complete!</div>`;
  } else {
    ct.innerHTML = `
      <div style="font-size:16px; color:var(--text-dim);">
        ${completedCount} of 5 books complete. Waiting for next book to start...
      </div>
      <div class="progress-outer">
        <div class="progress-inner complete" style="width:${(completedCount/5)*100}%"></div>
        <div class="progress-text">${completedCount} / 5 books</div>
      </div>
    `;
  }

  document.getElementById('brain-stats').innerHTML = `
    <div class="brain-stat nodes"><div class="num">${formatNumber(data.brain.nodes)}</div><div class="lbl">Nodes</div></div>
    <div class="brain-stat bonds"><div class="num">${formatNumber(data.brain.bonds)}</div><div class="lbl">Bonds</div></div>
  `;

  document.getElementById('eta-display').innerHTML = `
    <div class="eta-block"><div class="time">${formatETA(data.eta.current_book_minutes)}</div><div class="lbl">Current Book</div></div>
    <div class="eta-block"><div class="time">${formatETA(data.eta.overall_minutes)}</div><div class="lbl">All Books</div></div>
  `;

  document.getElementById('error-display').innerHTML = `
    <div class="error-badge errors"><div class="num">${data.totals.errors}</div><div class="lbl">Errors</div></div>
    <div class="error-badge timeouts"><div class="num">${data.totals.timeouts}</div><div class="lbl">Timeouts</div></div>
  `;

  const tl = document.getElementById('book-timeline');
  tl.innerHTML = data.books.map(b => {
    const statusClass = b.status;
    const badge = b.status === 'complete' ? 'Complete' : b.status === 'training' ? 'Training' : 'Pending';
    let sub = '';
    let miniBar = '';

    if (b.status === 'complete') {
      sub = b.pages > 0 ? `${b.pages.toLocaleString()} pages processed` : 'Processed';
      if (b.first_timestamp) sub += ` &bull; Started ${new Date(b.first_timestamp).toLocaleDateString()}`;
    } else if (b.status === 'training' && b.progress) {
      const pct = Math.round((b.progress.current / b.progress.total) * 100);
      sub = `Page ${b.progress.current}/${b.progress.total} &bull; ${b.progress.items.toLocaleString()} items &bull; ${b.progress.speed} pg/min`;
      miniBar = `<div class="mini-progress"><div class="mini-progress-inner" style="width:${pct}%"></div></div>`;
    } else {
      sub = b.pages > 0 ? `${b.pages.toLocaleString()} pages` : 'Page count TBD';
    }

    return `
      <div class="timeline-item ${statusClass}">
        <div class="timeline-num">${b.id}</div>
        <div class="timeline-info">
          <div class="timeline-title">${b.title}</div>
          <div class="timeline-sub">${sub}</div>
          ${miniBar}
        </div>
        <div class="timeline-badge">${badge}</div>
      </div>
    `;
  }).join('');

  let historyBook = currentBook || data.books.find(b => b.progress_history && b.progress_history.length > 0);
  if (historyBook && historyBook.progress_history && historyBook.progress_history.length > 1) {
    const labels = historyBook.progress_history.map(p => p.current);
    const speeds = historyBook.progress_history.map(p => p.speed);

    if (speedChart) speedChart.destroy();
    speedChart = new Chart(document.getElementById('speedChart'), {
      type: 'line',
      data: {
        labels: labels,
        datasets: [{
          data: speeds,
          borderColor: '#a29bfe',
          backgroundColor: '#6c5ce722',
          fill: true,
          tension: 0.3,
          pointRadius: 0,
          borderWidth: 2,
        }]
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        plugins: { legend: { display: false } },
        scales: {
          x: { title: { display: true, text: 'Page', color: '#8b8fa3' }, ticks: { color: '#8b8fa3', maxTicksLimit: 10 }, grid: { color: '#2e3146' } },
          y: { title: { display: true, text: 'pages/min', color: '#8b8fa3' }, ticks: { color: '#8b8fa3' }, grid: { color: '#2e3146' }, beginAtZero: true }
        }
      }
    });

    const items = historyBook.progress_history.map(p => p.items);
    if (itemsChart) itemsChart.destroy();
    itemsChart = new Chart(document.getElementById('itemsChart'), {
      type: 'line',
      data: {
        labels: labels,
        datasets: [{
          data: items,
          borderColor: '#81ecec',
          backgroundColor: '#81ecec22',
          fill: true,
          tension: 0.3,
          pointRadius: 0,
          borderWidth: 2,
        }]
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        plugins: { legend: { display: false } },
        scales: {
          x: { title: { display: true, text: 'Page', color: '#8b8fa3' }, ticks: { color: '#8b8fa3', maxTicksLimit: 10 }, grid: { color: '#2e3146' } },
          y: { title: { display: true, text: 'Items', color: '#8b8fa3' }, ticks: { color: '#8b8fa3' }, grid: { color: '#2e3146' }, beginAtZero: true }
        }
      }
    });
  }
}

const DATA = __DATA_PLACEHOLDER__;
renderData(DATA);
</script>
</body>
</html>"""


# HTTP Server

class DashboardHandler(SimpleHTTPRequestHandler):
    def do_GET(self):
        if self.path == '/' or self.path == '/index.html':
            self.send_dashboard()
        elif self.path == '/api/data':
            self.send_json()
        else:
            self.send_error(404)

    def send_dashboard(self):
        data = gather_all_data()
        data_json = json.dumps(data)
        html = DASHBOARD_HTML.replace('__DATA_PLACEHOLDER__', data_json)
        html = html.replace('REFRESH_INTERVAL', str(REFRESH_SECONDS))
        self.send_response(200)
        self.send_header('Content-Type', 'text/html; charset=utf-8')
        self.end_headers()
        self.wfile.write(html.encode('utf-8'))

    def send_json(self):
        data = gather_all_data()
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.end_headers()
        self.wfile.write(json.dumps(data, indent=2).encode('utf-8'))

    def log_message(self, format, *args):
        pass


def main():
    print(f"""
    ========================================
      Lu's Brain Training Dashboard
    ========================================

      Open in browser:  http://localhost:{PORT}

      Auto-refreshes every {REFRESH_SECONDS} seconds
      Reading logs from: {LOG_DIR}

      Press Ctrl+C to stop
    ========================================
    """)

    log_files = find_log_files()
    if log_files:
        print(f"  Found {len(log_files)} log file(s):")
        for lf in log_files:
            print(f"    - {os.path.basename(lf)}")
    else:
        print(f"  No log files found yet in {LOG_DIR}")
        print(f"  Dashboard will show data once training starts.")

    progress_file = os.path.join(LOG_DIR, "training_progress.txt")
    if os.path.exists(progress_file):
        print(f"  Master progress log found: training_progress.txt")
    print()

    server = HTTPServer(('0.0.0.0', PORT), DashboardHandler)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\n  Dashboard stopped.")
        server.server_close()


if __name__ == '__main__':
    main()
