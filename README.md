# Cattledog

Cattledog is a safety layer you put around an LLM. It is a spinoff of Lu, its author's larger AI project, and reuses two of Lu's ideas:

- **Honesty**: if an answer isn't backed by anything, say "I don't know".
- **Scars**: memories of past mistakes and forbidden actions. Before a tool call runs, Cattledog checks it against the scars and blocks matches.

It is meant to work with any LLM. This proof of concept uses a local Ollama model:

`LLM (Ollama) -> thin Python orchestrator -> Lu sandbox brain as the gate`

## Layout
- `orchestrator/`, `demo/`, `eval/`, `sandbox/`, `scripts/`: the Python side.
- `rust/`: the Lu brain crates, semantic scars, `lu_remember` / `lu_semantic_recall`, a BGE-small ONNX embedder, and the `lu_mcp_v2_cd` MCP binary (in `rust/v2`). See `rust/README.md`.
- `docs/`: findings from each experiment. `docs/DEMOS.md` describes the demos in detail.

## What works
- **Capability gate demo** (`demo/demo_capability_gate.py`): 8 tools are registered and 4 of them are scarred. In the author's run, two jailbreak prompts made the model call forbidden tools. Without the gate those calls ran; with the gate they were blocked.
- Answer mediation, a hybrid gate, a kill switch and red-team evals are in `demo/`, `orchestrator/` and `eval/`, with write-ups in `docs/`.

## What is unproven
- The results come from a small number of the author's own runs and prompts. Nobody has replicated them independently.
- The evasion and red-team findings in `docs/` show bypasses as well as blocks. Read them before you trust the gate.
- It has only been tested with local Ollama models on Windows. Path handling is now cross-platform, but the demos haven't been run end to end on macOS or Linux.
- It is not a security product. Don't rely on it to stop a determined attacker.

## How to run it
1. Install Python 3, Rust and [Ollama](https://ollama.com), then pull a model.
2. Build the Lu MCP binary: `cd rust/v2 && cargo build --release` (it produces `lu_mcp_v2_cd`).
3. Run a demo from any folder: `python demo/demo_capability_gate.py`

Paths are resolved from the repo root. By default that's worked out from each script's location; set `CD_ROOT` to point somewhere else. Lu binaries are looked up in `rust/v2/target/release/` (with `.exe` on Windows); set `LU_BIN_DIR` to override. Some scripts write to `logs/`, so create it first (`mkdir logs`).

Versions aren't pinned yet, and these steps haven't been tested on a clean machine.

## License
Copyright 2026 DialectForge LLC. Created by John Dean Martin.

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
