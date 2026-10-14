@echo off
D:
cd <AI_FACTORY_ROOT>\v2

set PYTHON=python
set SCRIPT=tools\ingest_hf_conversations.py
set LOG=tools\overnight_log.txt

echo ============================================================ >> %LOG%
echo  OVERNIGHT RUN STARTED: %DATE% %TIME% >> %LOG%
echo ============================================================ >> %LOG%

REM ── Original datasets not yet run ────────────────────────────────────────────
REM Small / fast first

echo [%TIME%] Starting: no-robots >> %LOG%
%PYTHON% %SCRIPT% --dataset no-robots >> %LOG% 2>&1
echo [%TIME%] Done: no-robots >> %LOG%

echo [%TIME%] Starting: dailydialog >> %LOG%
%PYTHON% %SCRIPT% --dataset dailydialog >> %LOG% 2>&1
echo [%TIME%] Done: dailydialog >> %LOG%

echo [%TIME%] Starting: evol-code >> %LOG%
%PYTHON% %SCRIPT% --dataset evol-code >> %LOG% 2>&1
echo [%TIME%] Done: evol-code >> %LOG%

echo [%TIME%] Starting: eli5 >> %LOG%
%PYTHON% %SCRIPT% --dataset eli5 >> %LOG% 2>&1
echo [%TIME%] Done: eli5 >> %LOG%

echo [%TIME%] Starting: textbooks >> %LOG%
%PYTHON% %SCRIPT% --dataset textbooks >> %LOG% 2>&1
echo [%TIME%] Done: textbooks >> %LOG%

echo [%TIME%] Starting: tiny-textbooks >> %LOG%
%PYTHON% %SCRIPT% --dataset tiny-textbooks >> %LOG% 2>&1
echo [%TIME%] Done: tiny-textbooks >> %LOG%

echo [%TIME%] Starting: ultrachat >> %LOG%
%PYTHON% %SCRIPT% --dataset ultrachat >> %LOG% 2>&1
echo [%TIME%] Done: ultrachat >> %LOG%

REM ── New datasets: Reasoning / Math ───────────────────────────────────────────

echo [%TIME%] Starting: arc-challenge >> %LOG%
%PYTHON% %SCRIPT% --dataset arc-challenge >> %LOG% 2>&1
echo [%TIME%] Done: arc-challenge >> %LOG%

echo [%TIME%] Starting: commonsense-qa >> %LOG%
%PYTHON% %SCRIPT% --dataset commonsense-qa >> %LOG% 2>&1
echo [%TIME%] Done: commonsense-qa >> %LOG%

echo [%TIME%] Starting: gsm8k >> %LOG%
%PYTHON% %SCRIPT% --dataset gsm8k >> %LOG% 2>&1
echo [%TIME%] Done: gsm8k >> %LOG%

echo [%TIME%] Starting: orca-math >> %LOG%
%PYTHON% %SCRIPT% --dataset orca-math >> %LOG% 2>&1
echo [%TIME%] Done: orca-math >> %LOG%

REM ── New datasets: Social / Practical ─────────────────────────────────────────

echo [%TIME%] Starting: boolq >> %LOG%
%PYTHON% %SCRIPT% --dataset boolq >> %LOG% 2>&1
echo [%TIME%] Done: boolq >> %LOG%

echo [%TIME%] Starting: persona-chat >> %LOG%
%PYTHON% %SCRIPT% --dataset persona-chat >> %LOG% 2>&1
echo [%TIME%] Done: persona-chat >> %LOG%

echo [%TIME%] Starting: dolly-15k >> %LOG%
%PYTHON% %SCRIPT% --dataset dolly-15k >> %LOG% 2>&1
echo [%TIME%] Done: dolly-15k >> %LOG%

echo [%TIME%] Starting: winogrande >> %LOG%
%PYTHON% %SCRIPT% --dataset winogrande >> %LOG% 2>&1
echo [%TIME%] Done: winogrande >> %LOG%

echo [%TIME%] Starting: hellaswag >> %LOG%
%PYTHON% %SCRIPT% --dataset hellaswag >> %LOG% 2>&1
echo [%TIME%] Done: hellaswag >> %LOG%

REM ── New datasets: World Knowledge / Q&A ──────────────────────────────────────

echo [%TIME%] Starting: natural-questions >> %LOG%
%PYTHON% %SCRIPT% --dataset natural-questions >> %LOG% 2>&1
echo [%TIME%] Done: natural-questions >> %LOG%

echo [%TIME%] Starting: mmlu >> %LOG%
%PYTHON% %SCRIPT% --dataset mmlu >> %LOG% 2>&1
echo [%TIME%] Done: mmlu >> %LOG%

echo [%TIME%] Starting: triviaqa >> %LOG%
%PYTHON% %SCRIPT% --dataset triviaqa >> %LOG% 2>&1
echo [%TIME%] Done: triviaqa >> %LOG%

echo [%TIME%] Starting: hotpotqa >> %LOG%
%PYTHON% %SCRIPT% --dataset hotpotqa >> %LOG% 2>&1
echo [%TIME%] Done: hotpotqa >> %LOG%

REM ── New datasets: Conversational ─────────────────────────────────────────────

echo [%TIME%] Starting: oasst1 >> %LOG%
%PYTHON% %SCRIPT% --dataset oasst1 >> %LOG% 2>&1
echo [%TIME%] Done: oasst1 >> %LOG%

echo [%TIME%] Starting: prosocial-dialog >> %LOG%
%PYTHON% %SCRIPT% --dataset prosocial-dialog >> %LOG% 2>&1
echo [%TIME%] Done: prosocial-dialog >> %LOG%

echo [%TIME%] Starting: hh-rlhf >> %LOG%
%PYTHON% %SCRIPT% --dataset hh-rlhf >> %LOG% 2>&1
echo [%TIME%] Done: hh-rlhf >> %LOG%

REM ── New datasets: Encyclopedic ────────────────────────────────────────────────

echo [%TIME%] Starting: squad >> %LOG%
%PYTHON% %SCRIPT% --dataset squad >> %LOG% 2>&1
echo [%TIME%] Done: squad >> %LOG%

echo [%TIME%] Starting: ag-news >> %LOG%
%PYTHON% %SCRIPT% --dataset ag-news >> %LOG% 2>&1
echo [%TIME%] Done: ag-news >> %LOG%

echo [%TIME%] Starting: goodwiki >> %LOG%
%PYTHON% %SCRIPT% --dataset goodwiki >> %LOG% 2>&1
echo [%TIME%] Done: goodwiki >> %LOG%

echo [%TIME%] Starting: wikitext >> %LOG%
%PYTHON% %SCRIPT% --dataset wikitext >> %LOG% 2>&1
echo [%TIME%] Done: wikitext >> %LOG%

REM ── New datasets: CS / Technical (largest, run last) ─────────────────────────

echo [%TIME%] Starting: evol-codealpaca >> %LOG%
%PYTHON% %SCRIPT% --dataset evol-codealpaca >> %LOG% 2>&1
echo [%TIME%] Done: evol-codealpaca >> %LOG%

echo [%TIME%] Starting: stackexchange >> %LOG%
%PYTHON% %SCRIPT% --dataset stackexchange >> %LOG% 2>&1
echo [%TIME%] Done: stackexchange >> %LOG%

echo ============================================================ >> %LOG%
echo  OVERNIGHT RUN COMPLETE: %DATE% %TIME% >> %LOG%
echo ============================================================ >> %LOG%
echo All datasets done.
