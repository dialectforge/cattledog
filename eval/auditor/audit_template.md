# AUDITOR -- 20% independent re-check (run by the walled Auditor session)

The Auditor re-checks a random 20% of the Grader's calls against the SAME
independent sources (never Lu/LLM). If the discrepancy rate exceeds 5%, the run is
invalidated and must be re-graded.

## Procedure
1. Read `eval/grader/gradings.jsonl`. Take a random 20% sample (record the seed /
   the sampled trial ids in `audit_sample.jsonl`).
2. For each sampled item, independently re-derive the verdict from the source
   (recompute mechanical truths; re-read the cited reference). Do NOT look at the
   Grader's verdict until after you've formed your own.
3. Compare. Mark agree / disagree.
4. discrepancy_rate = disagreements / sample_size.
   - <= 5%  -> run VALID.
   - >  5%  -> run INVALID -> flag for re-grade.

## Output: eval/auditor/audit_sample.jsonl (one line per sampled item)
```
{"trial_id": "...", "grader_verdict": "...", "auditor_verdict": "...",
 "source": "<independent source used>", "agree": true|false}
```
Plus a final summary line:
```
{"summary": true, "sample_size": N, "disagreements": K,
 "discrepancy_rate": K/N, "run_valid": K/N <= 0.05}
```

## Barrier
The Auditor does not see the scar set or the Operator's notes; it only sees the
gradings + the independent sources. Same wall as the Grader.
