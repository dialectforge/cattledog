# --- Cattledog root (cross-platform). Override with the CD_ROOT env var. ---
import os as _os
CD_ROOT = _os.path.abspath(_os.environ.get('CD_ROOT') or _os.path.join(_os.path.dirname(_os.path.abspath(__file__)), '..'))
def _cd(*parts):
    return _os.path.join(CD_ROOT, *parts)
def _lu_exe(name):
    d = _os.environ.get('LU_BIN_DIR') or _cd('rust', 'v2', 'target', 'release')
    return _os.path.join(d, name + ('.exe' if _os.name == 'nt' else ''))
# ---------------------------------------------------------------------------
C=_cd('rust', 'v2', 'crates') + _os.sep
emb_fn=open(_cd('scripts', 'insert_embedding_fn.txt'),encoding='utf-8').read()
mcp_tools=open(_cd('scripts', 'insert_mcp_tools.txt'),encoding='utf-8').read()

def patch(path, checks):
    s=open(path,encoding='utf-8').read()
    orig=s
    for marker, old, new, n in checks:
        if marker in s:
            print('  skip (already):', marker); continue
        c=s.count(old)
        assert c==n, ('anchor %r count=%d expected %d in %s' % (old[:40], c, n, path))
        s=s.replace(old, new)
        print('  applied:', marker)
    if s!=orig:
        open(path,'w',encoding='utf-8').write(s)

# 1. embedding/Cargo.toml: add fastembed
patch(C+'embedding/Cargo.toml', [
  ('fastembed',
   'thiserror = { workspace = true }\n',
   'thiserror = { workspace = true }\nfastembed = "4"\n', 1),
])
# 2. embedding/src/lib.rs: append semantic_embed fn
s=open(C+'embedding/src/lib.rs',encoding='utf-8').read()
if 'pub fn semantic_embed' not in s:
    open(C+'embedding/src/lib.rs','a',encoding='utf-8').write('\n'+emb_fn)
    print('  applied: embedding semantic_embed appended')
else:
    print('  skip (already): embedding semantic_embed')
# 3. brain/src/lib.rs: swap emb (x2) + drop now-unused use import
s=open(C+'brain/src/lib.rs',encoding='utf-8').read()
if 'embedding::semantic_embed' not in s:
    old_emb='        let emb = embedding::StubEngine::new(384).embed(text).unwrap_or_else(|_| vec![0.0; 384]);'
    assert s.count(old_emb)==2, ('brain emb count', s.count(old_emb))
    s=s.replace(old_emb,'        let emb = embedding::semantic_embed(text);')
    s=s.replace('        use embedding::EmbeddingEngine;\n','')
    open(C+'brain/src/lib.rs','w',encoding='utf-8').write(s)
    print('  applied: brain semantic_embed swap')
else:
    print('  skip (already): brain')
# 4. mcp_server/Cargo.toml: add embedding dep
patch(C+'mcp_server/Cargo.toml', [
  ('embedding  = { path = "../embedding" }',
   'graph      = { path = "../graph" }\n',
   'graph      = { path = "../graph" }\nembedding  = { path = "../embedding" }\n', 1),
])
# 5. mcp_server/src/lib.rs: struct field + ctor + tools
patch(C+'mcp_server/src/lib.rs', [
  ('pub semantic_mem',
   '    pub brain_path: Option<std::path::PathBuf>,\n}',
   '    pub brain_path: Option<std::path::PathBuf>,\n    pub semantic_mem: Vec<(String, Vec<f32>)>,\n}', 1),
  ('semantic_mem: Vec::new()',
   'McpServer { brain, brain_path }',
   'McpServer { brain, brain_path, semantic_mem: Vec::new() }', 1),
  ('lu_semantic_recall',
   '            "lu_status" => {',
   mcp_tools+'            "lu_status" => {', 1),
])
print('keystone patches done')
