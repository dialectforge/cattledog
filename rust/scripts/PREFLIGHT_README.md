# Lu Preflight Verification Harness

Pre-ingest gate. Runs before any 1+ hour ingest job and refuses to launch if
any contract surface is misaligned. The job that ate 14 hours of compute
during Phase B was a silent vocabulary drop — the extractor was emitting
relation strings the Rust side could not parse. This harness exists so that
class of bug never lands again.

## Quick start

```
python scripts/preflight.py                  # all phases
python scripts/preflight.py --phase A        # vocabulary checks only
python scripts/preflight.py --phase B        # config + deployment
python scripts/preflight.py --phase C        # integration smoke
python scripts/preflight.py --no-remote      # skip Hetzner SSH steps
python scripts/preflight.py --json           # machine-readable output
```

## Verdict semantics

The harness prints one of two banners and sets an exit code CI can read:

| Outcome     | Exit | Meaning                                                |
|-------------|------|--------------------------------------------------------|
| GO          | 0    | All phases pass. Safe to launch ingest.                |
| NO-GO (A)   | 10   | Phase A vocabulary check failed.                       |
| NO-GO (B)   | 20   | Phase B config / deployment check failed.              |
| NO-GO (C)   | 30   | Phase C smoke ingest failed.                           |
| NO-GO (mix) | 40   | More than one phase failed.                            |
| INTERNAL    | 99   | Preflight crashed before producing a verdict.          |

Per-check status glyphs:

| Glyph  | Meaning                                                          |
|--------|------------------------------------------------------------------|
| `[ok]` | PASS — assertion held                                            |
| `[XX]` | FAIL — assertion broke; this blocks GO                           |
| `[??]` | WARN — issue worth knowing about; does **not** block             |
| `[--]` | SKIP — check was bypassed (e.g. `--no-remote`)                   |
| `[..]` | STUB — pending rebuild decision; pass-through with noisy notice  |

## Checks and what each one covers

Cross-references below point to the project log audit components #856 (wider
scope) and #857 (seam-by-seam). The format is `audit-id ↔ check-id`.

### Phase A — Vocabulary contract (load-bearing)

**A.1 — BondType vocabulary** *(covers #857 P0: "producer vocabulary ≠
consumer acceptance")*. Parses every relation string the extractor emits
(literal `"relation": "X"` plus the `_RX_*` pattern tuples) and asserts each
parses on the Rust side. Acceptance set is the union of `BondType` variant
PascalCase identifiers and the strings returned by `BondType::label()`. If
any extractor relation is in neither, the check fails and lists the DOA
strings — exactly the failure mode that ate 14h.

**A.2 — NodeType vocabulary** *(covers #857 mid-tier: "node-type producer
fallthrough")*. Asserts the documented six `NodeType` variants
(`Fact, Concept, Procedure, Emotion, Meta, Media`) exist. v2 uses `KuKind`
instead of `NodeType`; the harness recognises that and degrades to a WARN
listing the 11 `KuKind` variants. The extractor does not emit a `node_type`
field today, so set-difference is deferred until it does internal notes.

**A.3 — KnowledgeContent `kind` presence** **STUB** *(Decision 2)*. Once
the extractor emits a `kind` field on every bond, this check ingests a
50-article sample and asserts no single kind exceeds 90% of the
distribution. Today: pass-through with a `[..]` notice. **TODO: lift stub
when `smart_extractor.py` grows `kind`.**

**A.4 — SourceKind / source attribution** *(covers #857 P1: "every bond
must carry a SourceKind")*. Asserts the producer stamps `source_url` on
every bond and probes whether a Rust `SourceKind` enum exists. Today the
producer ⊆ consumer relationship is implicit (free-form string pass-through)
because `SourceKind` has not been introduced yet. WARN until rebuild.

**A.5 — Cross-language extension fields** *(covers #857 P1: "`layer`,
`frequency` not in TypedBond")*. The extractor stamps `layer`, `source_url`,
and `frequency` on bonds. The Rust `TypedBond` struct does not have these
fields. **Policy of record: extension fields are scrubbed at the ingest
boundary before being passed into the Rust pipeline.** WARN today; the
rebuild will add them as first-class fields and the assertion will flip to
"all extension fields are accepted by Rust struct".

### Phase B — Configuration & deployment

**B.1 — Binary md5 vs deployed** *(covers #856: "deployed binary may not
match local")*. md5sums local `target/release/lu_4_mcp` and asserts equality
with `/usr/local/bin/lu_4_mcp` on Hetzner via SSH alias `lu`. Skips with
WARN if either binary is unreachable (e.g. no `cargo build --release` yet,
no remote access).

**B.2 — Autosave-flag audit** *(covers #856 Perf-2: "autosave was the root
cause of the slowdown")*. Reads `systemctl cat lu.service` over SSH and
asserts no autosave flag (`save-on-every-write`, `save_on_every_write`,
`--autosave`, `AUTOSAVE=1`) appears in the unit. The current unit is
intentionally disabled and contains no autosave flag.

**B.3 — RuntimeConfig setter coverage** *(covers #857 P0: "~14 of ~25 fields
unsettable")*. Parses the runtime config struct (today: `RecallParameters`
in `self_improvement.rs`; post-rebuild: `RuntimeConfig`) and asserts every
public field is reachable through the `lu_configure` MCP tool's `args[]`
keys. Fails with the list of unsettable fields. Once the rebuild introduces
per-port setters (Decision 3) plus `tool_configure_batch`, this check will
read both surfaces.

### Phase C — Integration smoke

**C.1 — 50-article ingest smoke** *(covers #857: "no test exercises the
full pipeline end-to-end")*. Builds a fixed 50-article synthetic sample (or
the real Wikipedia fixture once it lands), runs the extractor over it, and
asserts:

  - bond-type histogram: no single type > 70% of the total
  - at least 8 distinct `BondType` variants populated
  - errors-per-article < 0.1 (gated=0 acceptable)
  - degree_p90 ≥ 5

The synthetic fixture is intentionally diverse-but-thin so the test runs in
seconds. The thresholds are calibrated for the real Wikipedia fixture; with
the synthetic stand-in, a 100% RelatedTo top-share is expected and is itself
a signal that the See-also walker dominates short-bodied articles.

**C.2 — Typed-KU dual-mode** **STUB** *(Decision 4)*. Once the runtime
supports a `recall.typed_ku.enabled` flag, this check runs C.1 twice (flag
true and flag false) and asserts both modes complete inside the smoke
budget. Today: pass-through with a `[..]` notice. **TODO: lift stub when
the feature flag lands.**

### Phase D — Output

**D.1** Single command runs all phases in order, fail-fast within a phase,
clear go/no-go verdict per phase plus a table of tripped assertions.

**D.2** Per-check exit codes (10 / 20 / 30 / 40 / 99) for CI parsing.

**D.3** `--phase A` / `--phase B` / `--phase C` flags select a subset.

**D.4** This file. Revise it when you add a check or lift a stub.

## Where the harness reads from

| Artifact                                               | Read by         |
|--------------------------------------------------------|-----------------|
| `scripts/smart_extractor.py`                           | A.1, A.4, A.5, C.1 |
| `v2/crates/graph/src/bond.rs`                          | A.1, A.5        |
| `v2/crates/graph/src/node.rs`                          | A.2             |
| `v2/crates/graph/src/knowledge_unit.rs`                | A.2 fallback    |
| `v2/crates/brain/src/self_improvement.rs`              | B.3             |
| `v2/crates/mcp_server/src/lib.rs`                      | B.3             |
| `v2/target/release/lu_4_mcp`                           | B.1             |
| `lu:/usr/local/bin/lu_4_mcp` (SSH)                     | B.1             |
| `lu:/etc/systemd/system/lu.service` (SSH)              | B.2             |

The harness is read-only. It does not write to the brain, build the binary,
or modify any source file.

## Lifting stubs

When you implement Decision 2 or Decision 4, find the matching `# TODO`
comment in `scripts/preflight.py` and replace the stub block with the real
check. Keep the check ID stable so audit cross-references stay valid.

## Adding a new check

1. Pick a check ID. Use `A.6`, `B.4`, `C.3` — never re-use a retired ID.
2. Add a `CheckResult` block inside `phase_a` / `phase_b` / `phase_c`.
3. Add a row to this README under the matching phase, with a one-sentence
   description and a `(covers #...)` cross-reference if there's an audit
   item it backs.
4. If the check needs a fixture, put it under `scripts/preflight_fixtures/`
   and keep it under 1MB so the repo stays cheap to clone.
5. Re-run `python scripts/preflight.py --phase <X>` and confirm the check
   passes when expected and fails loudly when broken.
