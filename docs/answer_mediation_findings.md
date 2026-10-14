# A) Answer-mediation layer -- build + honest empirical finding

## What was built (all real, all from Lu's primitives)
`orchestrator/answer_mediator.py`:
- **Off-ness / grounding signal:** `lu_think` (it has an idk gate -- returns *"I don't
  have sufficient information..."* when ungrounded) plus `lu_recall_quality`
  (per-query relevance). `lu_conversation` was rejected as a signal -- it returned
  the "Paris" node for an unrelated "Zorblax" question (nearest-node bleed).
- **Kick-back-to-LLM re-query (built explicitly):** shows the model its own prior
  answer + Lu's specific objection/evidence and asks it to self-correct or admit it
  is unsure. (John: "LLMs are good at correcting if you show them their output.")
- **Independent path (built):** `lu_web_research` -- external fetch+ingest, a
  non-base-model corroboration source.
- **Hard loop cap -> IDK (built explicitly):** on exhaustion it routes to an IDK
  output, never accept-last. `lu_think` alone does one retry then accepts; that gap
  is closed here.
- **Test-3 logging:** initial output, detector verdict, #loops, final output,
  termination reason -- all captured (`logs/mediation_run.log`).

## The honest finding (partial / negative -- valid per the protocol)
On the sparse sandbox brain the **grounding signal is too weak to be useful**:

| Question | Base answer (correct?) | Lu signal | Mediator final |
|----------|------------------------|-----------|----------------|
| "What is the capital of France?" (Paris taught to Lu) | Paris (correct) | `lu_think` returned **idk** | **[IDK]** -- false alarm |
| obscure fictional entity | "that's fictional / doesn't exist" (correct) | idk / conflict | [IDK] -- correct answer degraded |

Even with `use_web=True`, `lu_think` still idk'd "capital of France." Root cause:
Lu v2's text recall is **lexical** (StubEngine / keyword) and brittle to phrasing
-- `lu_think("capital of France")` grounds, but `lu_think("What is the capital of
France?")` does not. The real semantic embedder (`BgeSmallEngine`, BGE ONNX) is a
stub, so paraphrase/question-form queries don't match taught facts.

**Consequence:** the detector flags ~everything and routes to IDK. In Test-2 terms
that is a degenerate corner -- miss rate ~0 (it never passes a confident guess) but
false-alarm rate ~100% (it also refuses correct answers). "No hallucinations" is
trivially reachable by always saying IDK; it is useless without a real grounding
signal.

## What would make it real (flagged, out of scope under client-only build)
1. Implement Lu's real semantic embedder (`BgeSmallEngine` ONNX) so recall grounds
   paraphrased/question-form queries -- this is the load-bearing fix.
2. Tune the idk-handling policy: treat `lu_idk` as "pass-through with low-confidence
   note" instead of "flag," and only hard-flag on an actual grounded `lu_view`
   conflict. That trades false alarms for misses; on a sparse brain it shifts the
   failure, it does not remove it. The knob is exposed; the bound is Lu's recall.

**Bottom line:** the answer-mediation *machinery* is complete and correct; its
*usefulness* is currently bounded by Lu v2's weak lexical recall. Test 2/3 can be
run with this layer, but the honest expected result today is a high false-alarm /
IDK-heavy profile, not "no hallucinations."
