# AGENTS.md: notes for coding AIs (and the humans directing them)

Cattledog was built by John Martin directing AI coding agents. Vibe coders are welcome: if you work by pointing an AI at a repo, this file is for your AI.

## What this is
Cattledog wraps any LLM with two ideas from Lu: **honesty** (say "I don't know" when an answer isn't grounded) and **scars** (memories of forbidden or past-mistake actions that block matching tool calls). Read `README.md`, then `docs/DEMOS.md`.

## Layout
- `orchestrator/`: the Python gate.
  - `lu_client.py` (talks to the Lu MCP binary), `lu_gate.py` and `hybrid_gate.py` (tool-call gate), `killswitch.py` (blocks revoked items from coming back in), `answer_mediator.py` (grounds answers, IDK fallback), `normalize.py` (decodes obfuscated payloads), `ollama_client.py`
- `demo/`: runnable demos. `eval/`: red-team prompts, harness, results. `docs/`: findings from each experiment.
- `rust/`: Lu brain crates. `rust/v2` builds `lu_mcp_v2_cd`, the MCP binary the orchestrator uses, plus a Tauri desktop app (`lu-desktop`).

## Build and test
```bash
cd rust/v2 && cargo build --release && cargo test --workspace
cd ..      && cargo build && cargo test --workspace
python demo/demo_capability_gate.py   # needs Ollama and a pulled model
```
Paths resolve from `CD_ROOT` (by default the repo root). Binaries are looked up in `rust/v2/target/release` (override with `LU_BIN_DIR`).

## Rules
- **Never commit secrets** or personal data. Brain files (`*.lbr`) and `logs/` are ignored.
- **Make honest claims.** The findings in `docs/` include the failures and bypasses. Keep doing that. Don't call the gate "unbreakable".
- New source files get `SPDX-License-Identifier: Apache-2.0`.

## License
Apache-2.0. Free for anyone, including companies. See `CONTRIBUTING.md`.

## Where to start
- Run the demos on macOS or Linux and fix what breaks.
- Add an adapter for a hosted LLM API alongside Ollama.
- Improve semantic scar generalization (see `docs/hybrid_gate_findings.md`).
- Pin Python dependencies and add a one-command setup.
