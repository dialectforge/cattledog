#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
"""Generate a self-contained HTML codebase browser for Lu v2."""
import os, json, html
from pathlib import Path

V2_ROOT = Path("/sessions/jolly-wonderful-brahmagupta/mnt/ai_factory/v2")
SKIP_DIRS = {"node_modules", "target", ".git", "dist", "gen"}
EXTENSIONS = {".rs", ".toml", ".jsx", ".tsx", ".js", ".ts", ".css", ".html", ".json"}
MAX_SIZE = 500_000

def should_skip(path: Path) -> bool:
    for part in path.parts:
        if part in SKIP_DIRS:
            return True
    return False

def collect_files():
    files = {}
    for ext in EXTENSIONS:
        for fp in V2_ROOT.rglob(f"*{ext}"):
            if should_skip(fp.relative_to(V2_ROOT)):
                continue
            if fp.stat().st_size > MAX_SIZE:
                continue
            rel = str(fp.relative_to(V2_ROOT)).replace("\\", "/")
            try:
                content = fp.read_text(encoding="utf-8", errors="replace")
                files[rel] = {
                    "content": content,
                    "size": fp.stat().st_size,
                    "lines": content.count("\n") + 1
                }
            except:
                pass
    return dict(sorted(files.items()))

def lang_class(path):
    ext = path.rsplit(".", 1)[-1] if "." in path else ""
    return {
        "rs": "rust", "toml": "toml", "jsx": "jsx", "tsx": "tsx",
        "js": "javascript", "ts": "typescript", "css": "css",
        "html": "xml", "json": "json"
    }.get(ext, "plaintext")

def build_tree(files):
    tree = {}
    for path in files:
        parts = path.split("/")
        node = tree
        for p in parts[:-1]:
            node = node.setdefault(p, {})
        node[parts[-1]] = {"__file__": path, "size": files[path]["size"], "lines": files[path]["lines"]}
    return tree

def render_tree_html(tree, depth=0):
    html_parts = []
    dirs = sorted([k for k in tree if isinstance(tree[k], dict) and "__file__" not in tree[k]])
    file_entries = sorted([k for k in tree if isinstance(tree[k], dict) and "__file__" in tree[k]])
    
    for d in dirs:
        html_parts.append(f'<div class="tree-dir" style="padding-left:{depth*16}px">')
        html_parts.append(f'<span class="dir-toggle" onclick="toggleDir(this)">&#9654;</span>')
        html_parts.append(f'<span class="dir-name">{html.escape(d)}/</span>')
        html_parts.append(f'<div class="dir-children" style="display:none">')
        html_parts.append(render_tree_html(tree[d], depth+1))
        html_parts.append('</div></div>')
    
    for f in file_entries:
        info = tree[f]
        path = info["__file__"]
        sz = info["size"]
        lines = info["lines"]
        sz_str = f"{sz:,}B" if sz < 1024 else f"{sz/1024:.1f}KB"
        html_parts.append(
            f'<div class="tree-file" style="padding-left:{(depth)*16+20}px" '
            f'onclick="showFile(\'{html.escape(path, quote=True)}\')" data-path="{html.escape(path, quote=True)}">'
            f'<span class="file-name">{html.escape(f)}</span>'
            f'<span class="file-meta">{lines}L &middot; {sz_str}</span>'
            f'</div>'
        )
    return "\n".join(html_parts)

files = collect_files()
tree = build_tree(files)
tree_html = render_tree_html(tree)

# Encode file contents as JSON for JS access
files_json = json.dumps({k: v["content"] for k, v in files.items()})

total_files = len(files)
total_lines = sum(v["lines"] for v in files.values())
total_size = sum(v["size"] for v in files.values())

output = f'''<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Lu Brain v2 — Codebase Browser</title>
<link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/styles/github-dark.min.css">
<script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/highlight.min.js"></script>
<script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/languages/rust.min.js"></script>
<script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/languages/toml.min.js"></script>
<script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/languages/typescript.min.js"></script>
<style>
  * {{ margin:0; padding:0; box-sizing:border-box; }}
  body {{ font-family: 'Segoe UI', system-ui, -apple-system, sans-serif; background:#0d1117; color:#c9d1d9; display:flex; height:100vh; overflow:hidden; }}
  
  /* Sidebar */
  #sidebar {{ width:320px; min-width:280px; background:#161b22; border-right:1px solid #30363d; display:flex; flex-direction:column; }}
  #sidebar-header {{ padding:16px; border-bottom:1px solid #30363d; }}
  #sidebar-header h1 {{ font-size:16px; color:#58a6ff; margin-bottom:4px; }}
  #sidebar-header .stats {{ font-size:11px; color:#8b949e; }}
  #search-box {{ width:100%; padding:8px 12px; background:#0d1117; border:1px solid #30363d; border-radius:6px; color:#c9d1d9; font-size:13px; margin-top:8px; outline:none; }}
  #search-box:focus {{ border-color:#58a6ff; }}
  #file-tree {{ flex:1; overflow-y:auto; padding:8px 0; }}
  #file-tree::-webkit-scrollbar {{ width:8px; }}
  #file-tree::-webkit-scrollbar-track {{ background:#161b22; }}
  #file-tree::-webkit-scrollbar-thumb {{ background:#30363d; border-radius:4px; }}
  
  .tree-dir {{ cursor:pointer; }}
  .dir-toggle {{ display:inline-block; width:16px; color:#8b949e; font-size:10px; transition:transform 0.15s; user-select:none; }}
  .dir-toggle.open {{ transform:rotate(90deg); }}
  .dir-name {{ color:#58a6ff; font-size:13px; font-weight:500; }}
  .dir-name:hover {{ text-decoration:underline; }}
  
  .tree-file {{ padding:3px 8px; cursor:pointer; display:flex; align-items:center; justify-content:space-between; border-radius:4px; }}
  .tree-file:hover {{ background:#1f2937; }}
  .tree-file.active {{ background:#1f6feb33; border-left:2px solid #58a6ff; }}
  .file-name {{ font-size:13px; color:#c9d1d9; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }}
  .file-meta {{ font-size:10px; color:#484f58; white-space:nowrap; margin-left:8px; }}
  
  /* Main content */
  #content {{ flex:1; display:flex; flex-direction:column; overflow:hidden; }}
  #file-header {{ padding:12px 20px; background:#161b22; border-bottom:1px solid #30363d; display:flex; align-items:center; gap:12px; min-height:48px; }}
  #file-path {{ font-size:13px; color:#8b949e; font-family:'Cascadia Code','Fira Code',monospace; }}
  #file-path span {{ color:#c9d1d9; }}
  
  #code-container {{ flex:1; overflow:auto; padding:0; }}
  #code-container::-webkit-scrollbar {{ width:10px; height:10px; }}
  #code-container::-webkit-scrollbar-track {{ background:#0d1117; }}
  #code-container::-webkit-scrollbar-thumb {{ background:#30363d; border-radius:4px; }}
  
  #welcome {{ display:flex; align-items:center; justify-content:center; height:100%; text-align:center; }}
  #welcome h2 {{ color:#58a6ff; font-size:24px; margin-bottom:8px; }}
  #welcome p {{ color:#8b949e; font-size:14px; }}
  
  pre {{ margin:0; }}
  pre code {{ font-family:'Cascadia Code','Fira Code','JetBrains Mono',monospace; font-size:13px; line-height:1.5; padding:16px 20px !important; display:block; }}
  
  .line-numbers {{ display:flex; }}
  .line-nums {{ text-align:right; padding:16px 12px 16px 16px; color:#484f58; font-size:13px; line-height:1.5; font-family:'Cascadia Code','Fira Code',monospace; user-select:none; border-right:1px solid #21262d; min-width:50px; }}
  .code-content {{ flex:1; overflow-x:auto; }}
  
  .hidden {{ display:none !important; }}
</style>
</head>
<body>

<div id="sidebar">
  <div id="sidebar-header">
    <h1>Lu Brain v2</h1>
    <div class="stats">{total_files} files &middot; {total_lines:,} lines &middot; {total_size/1024:.0f} KB</div>
    <input type="text" id="search-box" placeholder="Search files..." oninput="filterTree(this.value)">
  </div>
  <div id="file-tree">
    {tree_html}
  </div>
</div>

<div id="content">
  <div id="file-header">
    <div id="file-path">Select a file from the tree</div>
  </div>
  <div id="code-container">
    <div id="welcome">
      <div>
        <h2>Lu Brain v2 Codebase</h2>
        <p>Click any file in the sidebar to view its source code.</p>
        <p style="margin-top:12px;color:#484f58">{total_files} source files across 8 crates</p>
      </div>
    </div>
  </div>
</div>

<script>
const FILES = {files_json};

function getLangClass(path) {{
  const ext = path.split('.').pop();
  return {{rs:'rust',toml:'toml',jsx:'javascript',tsx:'typescript',js:'javascript',ts:'typescript',css:'css',html:'xml',json:'json'}}[ext] || 'plaintext';
}}

function showFile(path) {{
  document.querySelectorAll('.tree-file').forEach(el => el.classList.remove('active'));
  const el = document.querySelector(`.tree-file[data-path="${{path}}"]`);
  if (el) el.classList.add('active');
  
  const content = FILES[path];
  if (!content) return;
  
  const parts = path.split('/');
  const fileName = parts.pop();
  const dirPath = parts.join('/');
  document.getElementById('file-path').innerHTML = dirPath ? `<span style="color:#8b949e">${{dirPath}}/</span><span>${{fileName}}</span>` : `<span>${{fileName}}</span>`;
  
  const lines = content.split('\\n');
  const lineNums = lines.map((_, i) => i + 1).join('\\n');
  
  const lang = getLangClass(path);
  const escaped = content.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
  
  document.getElementById('code-container').innerHTML = `
    <div class="line-numbers">
      <div class="line-nums">${{lineNums}}</div>
      <div class="code-content">
        <pre><code class="language-${{lang}}">${{escaped}}</code></pre>
      </div>
    </div>`;
  
  document.querySelectorAll('pre code').forEach(block => hljs.highlightElement(block));
}}

function toggleDir(el) {{
  el.classList.toggle('open');
  const children = el.parentElement.querySelector('.dir-children');
  children.style.display = children.style.display === 'none' ? 'block' : 'none';
}}

function filterTree(query) {{
  query = query.toLowerCase().trim();
  document.querySelectorAll('.tree-file').forEach(el => {{
    const path = el.dataset.path.toLowerCase();
    el.classList.toggle('hidden', query && !path.includes(query));
  }});
  // Auto-expand dirs that have visible children
  document.querySelectorAll('.tree-dir').forEach(dir => {{
    const hasVisible = dir.querySelector('.tree-file:not(.hidden)');
    const children = dir.querySelector('.dir-children');
    if (query && hasVisible) {{
      children.style.display = 'block';
      const toggle = dir.querySelector('.dir-toggle');
      if (toggle) toggle.classList.add('open');
    }}
  }});
}}

// Auto-expand first level
document.querySelectorAll('#file-tree > .tree-dir > .dir-children').forEach(el => {{
  el.style.display = 'block';
  el.parentElement.querySelector('.dir-toggle').classList.add('open');
}});
</script>
</body>
</html>'''

out_path = "/sessions/jolly-wonderful-brahmagupta/mnt/ai_factory/v2_codebase_browser.html"
Path(out_path).write_text(output, encoding="utf-8")
print(f"Written to {out_path}")
print(f"Size: {os.path.getsize(out_path) / 1024:.0f} KB")
print(f"Files embedded: {total_files}")
print(f"Total lines: {total_lines:,}")
