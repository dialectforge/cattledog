#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""Generate a self-contained HTML codebase viewer for Lu v2 Rust files."""

import os
import json
import re
from pathlib import Path
from collections import defaultdict

ROOT = Path("<AI_FACTORY_ROOT>/v2/crates")
OUTPUT = Path("<AI_FACTORY_ROOT>/v2/lu_v2_codebase.html")

# ── Rust syntax highlighting ──────────────────────────────────────────────────

KEYWORDS = {
    "as", "async", "await", "break", "const", "continue", "crate", "dyn",
    "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in",
    "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while", "box", "do", "final", "macro",
    "override", "priv", "typeof", "unsized", "virtual", "yield", "abstract",
    "become", "try",
}

TYPES = {
    "bool", "char", "f32", "f64", "i8", "i16", "i32", "i64", "i128",
    "isize", "str", "u8", "u16", "u32", "u64", "u128", "usize",
    "String", "Vec", "Option", "Result", "Box", "Rc", "Arc", "Cell",
    "RefCell", "HashMap", "HashSet", "BTreeMap", "BTreeSet", "Cow",
    "Mutex", "RwLock", "Pin", "PhantomData",
}


def escape_html(s: str) -> str:
    return (s.replace("&", "&amp;")
             .replace("<", "&lt;")
             .replace(">", "&gt;")
             .replace('"', "&quot;"))


def highlight_rust(code: str) -> str:
    """Line-by-line Rust highlighter returning HTML with span classes."""
    lines = code.split("\n")
    out = []
    in_block_comment = False

    for line in lines:
        result = []
        i = 0
        n = len(line)

        if in_block_comment:
            # find end of block comment
            end = line.find("*/")
            if end == -1:
                result.append(f'<span class="cm">{escape_html(line)}</span>')
                out.append("".join(result))
                continue
            else:
                result.append(f'<span class="cm">{escape_html(line[:end+2])}</span>')
                i = end + 2
                in_block_comment = False

        while i < n:
            # Block comment start
            if line[i:i+2] == "/*":
                end = line.find("*/", i + 2)
                if end == -1:
                    result.append(f'<span class="cm">{escape_html(line[i:])}</span>')
                    in_block_comment = True
                    i = n
                else:
                    result.append(f'<span class="cm">{escape_html(line[i:end+2])}</span>')
                    i = end + 2
                continue

            # Line comment
            if line[i:i+2] == "//":
                result.append(f'<span class="lc">{escape_html(line[i:])}</span>')
                i = n
                continue

            # String literal
            if line[i] == '"':
                j = i + 1
                while j < n:
                    if line[j] == '\\':
                        j += 2
                    elif line[j] == '"':
                        j += 1
                        break
                    else:
                        j += 1
                result.append(f'<span class="s">{escape_html(line[i:j])}</span>')
                i = j
                continue

            # Char literal
            if line[i] == "'":
                j = i + 1
                if j < n and line[j] == '\\':
                    j += 2
                elif j < n:
                    j += 1
                if j < n and line[j] == "'":
                    j += 1
                    result.append(f'<span class="s">{escape_html(line[i:j])}</span>')
                    i = j
                else:
                    result.append(escape_html(line[i]))
                    i += 1
                continue

            # Lifetime / label
            if line[i] == "'" and i + 1 < n and line[i+1].isalpha():
                j = i + 1
                while j < n and (line[j].isalnum() or line[j] == '_'):
                    j += 1
                result.append(f'<span class="lt">{escape_html(line[i:j])}</span>')
                i = j
                continue

            # Number
            if line[i].isdigit() or (line[i] == '-' and i+1 < n and line[i+1].isdigit()):
                j = i
                if line[j] == '-':
                    j += 1
                while j < n and (line[j].isalnum() or line[j] in '._'):
                    j += 1
                result.append(f'<span class="n">{escape_html(line[i:j])}</span>')
                i = j
                continue

            # Macro invocation: name!
            macro_m = re.match(r'([a-zA-Z_]\w*)(!)', line[i:])
            if macro_m:
                name, bang = macro_m.group(1), macro_m.group(2)
                result.append(f'<span class="mac">{escape_html(name)}</span><span class="mac">!</span>')
                i += len(name) + 1
                continue

            # Attribute
            if line[i] == '#' and i + 1 < n and line[i+1] == '[':
                j = line.find(']', i)
                if j == -1:
                    result.append(f'<span class="attr">{escape_html(line[i:])}</span>')
                    i = n
                else:
                    result.append(f'<span class="attr">{escape_html(line[i:j+1])}</span>')
                    i = j + 1
                continue

            # Word token: keyword / type / identifier
            if line[i].isalpha() or line[i] == '_':
                j = i
                while j < n and (line[j].isalnum() or line[j] == '_'):
                    j += 1
                word = line[i:j]
                if word in KEYWORDS:
                    result.append(f'<span class="kw">{escape_html(word)}</span>')
                elif word in TYPES:
                    result.append(f'<span class="ty">{escape_html(word)}</span>')
                else:
                    result.append(escape_html(word))
                i = j
                continue

            result.append(escape_html(line[i]))
            i += 1

        out.append("".join(result))

    return "\n".join(out)


# ── Collect files ─────────────────────────────────────────────────────────────

def collect_files():
    files = []
    for dirpath, dirnames, filenames in os.walk(ROOT):
        dirnames.sort()
        for fname in sorted(filenames):
            if fname.endswith(".rs"):
                full = Path(dirpath) / fname
                rel = full.relative_to(ROOT)
                files.append((str(rel).replace("\\", "/"), full))
    return files


# ── Build tree structure ──────────────────────────────────────────────────────

def build_tree(files):
    """Returns nested dict representing directory tree."""
    tree = {}
    for rel, _ in files:
        parts = rel.split("/")
        node = tree
        for part in parts[:-1]:
            node = node.setdefault(part, {})
        node[parts[-1]] = rel  # leaf: file_id
    return tree


def render_tree_html(node, depth=0):
    html = []
    for key, val in sorted(node.items(), key=lambda x: (isinstance(x[1], str), x[0])):
        if isinstance(val, str):
            # file leaf
            file_id = val.replace("/", "__").replace(".", "_")
            html.append(
                f'<div class="tree-file" data-file="{file_id}" '
                f'onclick="showFile(\'{file_id}\')" title="{val}">'
                f'<span class="icon">&#x1F4C4;</span>{escape_html(key)}</div>'
            )
        else:
            # directory
            html.append(
                f'<div class="tree-dir" onclick="toggleDir(this)">'
                f'<span class="arrow">&#9660;</span>'
                f'<span class="icon">&#x1F4C1;</span>{escape_html(key)}</div>'
                f'<div class="tree-children">'
            )
            html.extend(render_tree_html(val, depth + 1))
            html.append('</div>')
    return html


# ── Main ──────────────────────────────────────────────────────────────────────

def main():
    print("Collecting .rs files...")
    files = collect_files()
    print(f"  Found {len(files)} files")

    tree = build_tree(files)
    tree_html = "\n".join(render_tree_html(tree))

    print("Building file panels...")
    panels_html_parts = []
    file_index = []  # for search: [{id, path, lines:[{ln, text}]}]

    for rel, full in files:
        file_id = rel.replace("/", "__").replace(".", "_")
        try:
            raw = full.read_text(encoding="utf-8", errors="replace")
        except Exception as e:
            raw = f"// Error reading file: {e}"

        highlighted = highlight_rust(raw)
        raw_lines = raw.split("\n")
        num_lines = len(raw_lines)
        pad = len(str(num_lines))

        hl_lines = highlighted.split("\n")
        rows = []
        for ln, (hl_line, raw_line) in enumerate(zip(hl_lines, raw_lines), 1):
            rows.append(
                f'<div class="code-line" id="{file_id}_L{ln}" data-text="{escape_html(raw_line.lower())}">'
                f'<span class="ln">{str(ln).rjust(pad)}</span>'
                f'<span class="lc-text">{hl_line if hl_line else " "}</span>'
                f'</div>'
            )

        panels_html_parts.append(
            f'<div class="file-panel" id="panel_{file_id}">'
            f'<div class="file-header">{escape_html(rel)}</div>'
            f'<div class="code-block">{"".join(rows)}</div>'
            f'</div>'
        )

        # index for search (store plain text per line)
        file_index.append({
            "id": file_id,
            "path": rel,
            "lines": [l.lower() for l in raw_lines],
        })

    panels_html = "\n".join(panels_html_parts)

    # Serialize search index as JSON embedded in JS
    search_index_json = json.dumps(file_index, ensure_ascii=False)

    print("Assembling HTML...")
    # Determine first file to show
    first_id = files[0][0].replace("/", "__").replace(".", "_") if files else ""

    html = f"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Lu v2 Codebase Viewer</title>
<style>
/* ── Reset & base ── */
*, *::before, *::after {{ box-sizing: border-box; margin: 0; padding: 0; }}
html, body {{ height: 100%; overflow: hidden; }}
body {{
  font-family: 'Segoe UI', system-ui, sans-serif;
  background: #1a1b1e;
  color: #cdd9e5;
  display: flex;
  flex-direction: column;
}}

/* ── Top bar ── */
#topbar {{
  height: 44px;
  background: #161719;
  border-bottom: 1px solid #2d2f34;
  display: flex;
  align-items: center;
  padding: 0 14px;
  gap: 12px;
  flex-shrink: 0;
  z-index: 10;
}}
#topbar h1 {{ font-size: 14px; font-weight: 600; color: #e2a64f; letter-spacing: .04em; flex: 0 0 auto; }}
#search-box {{
  flex: 1;
  max-width: 480px;
  background: #232428;
  border: 1px solid #3a3c42;
  border-radius: 6px;
  padding: 5px 10px;
  color: #cdd9e5;
  font-size: 13px;
  outline: none;
  transition: border-color .15s;
}}
#search-box:focus {{ border-color: #6ea4d4; }}
#search-info {{ font-size: 12px; color: #7a8390; min-width: 120px; }}
#search-nav {{ display: flex; gap: 4px; }}
.nav-btn {{
  background: #2a2c31;
  border: 1px solid #3a3c42;
  color: #cdd9e5;
  border-radius: 4px;
  padding: 3px 9px;
  font-size: 12px;
  cursor: pointer;
}}
.nav-btn:hover {{ background: #35373d; }}

/* ── Layout ── */
#layout {{
  display: flex;
  flex: 1;
  overflow: hidden;
}}

/* ── Sidebar ── */
#sidebar {{
  width: 280px;
  min-width: 160px;
  max-width: 480px;
  background: #1e1f23;
  border-right: 1px solid #2d2f34;
  overflow-y: auto;
  overflow-x: hidden;
  flex-shrink: 0;
  padding: 8px 0;
  resize: horizontal;
  font-size: 12.5px;
}}
#sidebar::-webkit-scrollbar {{ width: 6px; }}
#sidebar::-webkit-scrollbar-thumb {{ background: #3a3c42; border-radius: 3px; }}

.tree-dir, .tree-file {{
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 2px 8px;
  cursor: pointer;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  border-radius: 4px;
  margin: 1px 4px;
}}
.tree-dir {{ color: #a8c4e0; font-weight: 500; }}
.tree-dir:hover {{ background: #2a2c31; }}
.tree-file {{ color: #cdd9e5; padding-left: 22px; }}
.tree-file:hover {{ background: #2a2c31; }}
.tree-file.active {{ background: #2d4a6a; color: #9ecbff; }}
.tree-children {{ padding-left: 14px; }}
.tree-children.collapsed {{ display: none; }}
.arrow {{ display: inline-block; transition: transform .15s; font-size: 10px; color: #7a8390; }}
.arrow.collapsed {{ transform: rotate(-90deg); }}
.icon {{ font-size: 12px; }}

/* ── Content area ── */
#content {{
  flex: 1;
  overflow: auto;
  padding: 0;
  background: #1a1b1e;
}}
#content::-webkit-scrollbar {{ width: 8px; height: 8px; }}
#content::-webkit-scrollbar-thumb {{ background: #3a3c42; border-radius: 4px; }}

.file-panel {{ display: none; }}
.file-panel.visible {{ display: block; }}

.file-header {{
  position: sticky;
  top: 0;
  background: #161719;
  border-bottom: 1px solid #2d2f34;
  padding: 8px 16px;
  font-size: 13px;
  color: #8db4d4;
  font-family: monospace;
  z-index: 5;
}}

.code-block {{
  padding: 12px 0;
  font-family: 'JetBrains Mono', 'Fira Code', 'Cascadia Code', Consolas, monospace;
  font-size: 13px;
  line-height: 1.6;
  tab-size: 4;
  white-space: pre;
}}

.code-line {{
  display: flex;
  align-items: baseline;
  padding: 0 16px 0 0;
  min-height: 20.8px;
}}
.code-line.hl-match {{ background: #3a3010 !important; }}
.code-line.hl-current {{ background: #5a4a10 !important; outline: 1px solid #e2a64f; }}

.ln {{
  display: inline-block;
  min-width: 3em;
  padding-right: 12px;
  text-align: right;
  color: #495060;
  user-select: none;
  flex-shrink: 0;
}}
.lc-text {{ white-space: pre; }}

/* ── Syntax colors ── */
.kw  {{ color: #c792ea; font-weight: 600; }}
.ty  {{ color: #82aaff; }}
.s   {{ color: #c3e88d; }}
.n   {{ color: #f78c6c; }}
.cm  {{ color: #546e7a; font-style: italic; }}
.lc  {{ color: #546e7a; font-style: italic; }}
.lt  {{ color: #89ddff; }}
.mac {{ color: #e2a64f; }}
.attr {{ color: #c792ea; opacity: .85; }}

/* ── Search highlight ── */
mark {{
  background: #5a4a10;
  color: inherit;
  border-radius: 2px;
}}
mark.current {{
  background: #e2a64f;
  color: #1a1b1e;
}}
</style>
</head>
<body>

<div id="topbar">
  <h1>&#x1F9E0; Lu v2</h1>
  <input id="search-box" type="text" placeholder="Search across all files (Ctrl+F)..." autocomplete="off" spellcheck="false">
  <span id="search-info"></span>
  <div id="search-nav">
    <button class="nav-btn" onclick="searchNav(-1)">&#9650;</button>
    <button class="nav-btn" onclick="searchNav(1)">&#9660;</button>
    <button class="nav-btn" onclick="clearSearch()">&#x2715;</button>
  </div>
</div>

<div id="layout">
  <div id="sidebar">
{tree_html}
  </div>
  <div id="content">
{panels_html}
  </div>
</div>

<script>
// ── Search index ──
const INDEX = {search_index_json};

// ── File navigation ──
let currentFileId = null;

function showFile(fileId) {{
  if (currentFileId === fileId) return;
  if (currentFileId) {{
    const old = document.getElementById('panel_' + currentFileId);
    if (old) old.classList.remove('visible');
    const oldLink = document.querySelector('.tree-file.active');
    if (oldLink) oldLink.classList.remove('active');
  }}
  currentFileId = fileId;
  const panel = document.getElementById('panel_' + fileId);
  if (panel) {{
    panel.classList.add('visible');
    document.getElementById('content').scrollTo(0, 0);
  }}
  const link = document.querySelector(`.tree-file[data-file="${{fileId}}"]`);
  if (link) {{
    link.classList.add('active');
    link.scrollIntoView({{ block: 'nearest' }});
  }}
  // Re-apply search highlights if active
  if (searchQuery) applySearch();
}}

function toggleDir(el) {{
  const arrow = el.querySelector('.arrow');
  const children = el.nextElementSibling;
  if (children && children.classList.contains('tree-children')) {{
    children.classList.toggle('collapsed');
    arrow.classList.toggle('collapsed');
  }}
}}

// ── Search ──
let searchQuery = '';
let searchMatches = []; // {{fileId, lineEl}}
let searchIdx = -1;

document.getElementById('search-box').addEventListener('input', function() {{
  searchQuery = this.value.trim().toLowerCase();
  if (searchQuery.length < 2) {{
    clearSearch(false);
    return;
  }}
  applySearch();
}});

document.addEventListener('keydown', function(e) {{
  if ((e.ctrlKey || e.metaKey) && e.key === 'f') {{
    e.preventDefault();
    document.getElementById('search-box').focus();
    document.getElementById('search-box').select();
  }}
  if (e.key === 'Enter' && document.activeElement === document.getElementById('search-box')) {{
    searchNav(e.shiftKey ? -1 : 1);
  }}
  if (e.key === 'Escape') clearSearch();
}});

function applySearch() {{
  // Clear old highlights
  document.querySelectorAll('.hl-match, .hl-current').forEach(el => {{
    el.classList.remove('hl-match', 'hl-current');
    // restore text from data-text? No — just remove class; actual text unchanged
  }});
  searchMatches = [];
  searchIdx = -1;

  if (!searchQuery || searchQuery.length < 2) {{
    updateSearchInfo();
    return;
  }}

  // Collect all visible panels (current file first, then rest)
  const panels = [];
  if (currentFileId) panels.push(currentFileId);
  INDEX.forEach(f => {{ if (f.id !== currentFileId) panels.push(f.id); }});

  panels.forEach(fileId => {{
    const fileData = INDEX.find(f => f.id === fileId);
    if (!fileData) return;
    fileData.lines.forEach((lineText, idx) => {{
      if (lineText.includes(searchQuery)) {{
        const lineEl = document.getElementById(fileId + '_L' + (idx + 1));
        if (lineEl) {{
          lineEl.classList.add('hl-match');
          searchMatches.push({{ fileId, lineEl }});
        }}
      }}
    }});
  }});

  updateSearchInfo();

  // Jump to first match in current file, or first overall
  if (searchMatches.length > 0) {{
    const firstInFile = searchMatches.findIndex(m => m.fileId === currentFileId);
    searchIdx = firstInFile >= 0 ? firstInFile : 0;
    jumpToMatch(searchIdx);
  }}
}}

function searchNav(dir) {{
  if (searchMatches.length === 0) return;
  searchIdx = (searchIdx + dir + searchMatches.length) % searchMatches.length;
  jumpToMatch(searchIdx);
}}

function jumpToMatch(idx) {{
  document.querySelectorAll('.hl-current').forEach(el => el.classList.remove('hl-current'));
  const m = searchMatches[idx];
  if (!m) return;
  // Switch file if needed
  if (m.fileId !== currentFileId) showFile(m.fileId);
  m.lineEl.classList.add('hl-current');
  m.lineEl.scrollIntoView({{ block: 'center', behavior: 'smooth' }});
  updateSearchInfo();
}}

function clearSearch(clearBox = true) {{
  if (clearBox) {{
    document.getElementById('search-box').value = '';
    searchQuery = '';
  }}
  document.querySelectorAll('.hl-match, .hl-current').forEach(el => {{
    el.classList.remove('hl-match', 'hl-current');
  }});
  searchMatches = [];
  searchIdx = -1;
  updateSearchInfo();
}}

function updateSearchInfo() {{
  const el = document.getElementById('search-info');
  if (!searchQuery || searchMatches.length === 0) {{
    el.textContent = searchQuery ? 'No matches' : '';
    return;
  }}
  el.textContent = `${{searchIdx + 1}} / ${{searchMatches.length}} matches`;
}}

// ── Init ──
(function() {{
  const first = document.querySelector('.tree-file');
  if (first) showFile(first.dataset.file);
}})();
</script>
</body>
</html>
"""

    OUTPUT.write_text(html, encoding="utf-8")
    size_kb = OUTPUT.stat().st_size // 1024
    print(f"Written: {OUTPUT}  ({size_kb} KB)")


if __name__ == "__main__":
    main()
