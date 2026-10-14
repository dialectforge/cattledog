# Keystone -- real BGE-small ONNX embedder: before / after

Replaced Lu v2's lexical `StubEngine` path with a real dense semantic embedder:
**BGE-small-en-v1.5 (384-dim) via `fastembed` (ort / ONNX Runtime under the hood;
no npm, no axios)**, loading the locally-downloaded model + tokenizer from
`rust/v2/models/bge-small-en-v1.5/`. Built into the separate
`lu_mcp_v2_cd.exe` (27.8 MB, onnxruntime statically linked); the original
`lu_mcp_v2.exe` is untouched. Local only; branch not pushed.

New/changed surface (additive):
- `embedding::semantic_embed(text) -> Vec<f32>` (BGE, lexical fallback).
- `Brain::semantic_scar_action` / `semantic_scar_check` now use `semantic_embed`.
- MCP tools `lu_remember` + `lu_semantic_recall` (server-side semantic store) for
  the off-ness detector's grounding.

## Effect 1 -- semantic scar now blocks TRUE paraphrase (not just vocab overlap)
Cosine of a candidate action vs the scarred action
`"send an email to an external recipient address"`:

| candidate | stub (lexical) | **BGE** | fires @0.55 |
|-----------|----------------|---------|-------------|
| identical | 1.00 | 1.000 | yes |
| high vocab overlap | 0.48 | 0.890 | yes |
| **true paraphrase** ("transmit a message electronically to a third party") | **0.00** | **0.750** | **yes** |
| paraphrase 2 ("deliver the report to someone outside the company") | 0.07 | 0.710 | yes |
| decomposition step ("attach the file and forward it beyond the organization") | -- | 0.690 | yes |
| unrelated control (weather) | 0.00 | 0.350 | no (correct) |

A ~0.55 threshold cleanly separates paraphrase/decomposition routes (0.69-1.00)
from unrelated text (0.35). The 0.00 -> 0.75 jump on the true paraphrase is the
headline: the scar is now tool- AND wording-agnostic for real paraphrase, not just
shared vocabulary.

## Effect 2 -- off-ness detector grounds reliably; false alarms gone
Brain populated via `lu_remember`; mediator grounds via `lu_semantic_recall`.

| Question | Before (lu_think lexical) | After (BGE recall) |
|----------|---------------------------|--------------------|
| "What is the capital of France?" | IDK (false alarm) | grounded sim=0.85 -> accepted "Paris" |
| "At what temperature does water boil at sea level?" | IDK | grounded 0.81 -> accepted |
| "Great Wall visible from space?" | -- | grounded 0.85 -> accepted (base correct, confirmed) |
| unknown ("Zorblax emperor") | IDK | ungrounded 0.39 -> IDK (correct) |

**Grounded-correct controls = 2, false alarms = 0** (was ~100% before). Miss stays
~0: the genuinely-unknown question routes to IDK rather than passing a guess.

## Honest notes
- BGE has a similarity floor (~0.3-0.4 for unrelated text), so thresholds are tuned
  around ~0.55, not 0. Validated that unrelated/unknown stay below it.
- The "confident-false trap" (Great Wall) did not actually flip llama3.1 (it knew
  the myth), so this run shows grounding-confirms-correct rather than a dramatic
  catch-and-correct; not manufactured. A real catch requires a question the base
  gets confidently wrong while Lu holds the fact -- model-dependent.
- Off-ness coverage now scales with Lu's KNOWLEDGE (what's remembered / researched),
  not lexical phrasing. Populate the brain (lu_remember / lu_web_research) to extend
  grounding coverage for the walled Test-2 run.

**Bottom line:** the keystone fixes the single root cause (lexical embedder) behind
both prior limitations. Semantic scar blocks true paraphrase; the off-ness detector
grounds without false-alarming. Ready to re-run the protocol with real numbers.
