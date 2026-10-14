@echo off
REM Chain all factual + programming datasets for overnight ingestion
REM Resume-safe: CSV log tracks what's already done, so re-running is fine
REM Wikipedia limited to 50K articles (would take days otherwise)

D:
cd <AI_FACTORY_ROOT>\v2\tools

echo ============================================================
echo   OVERNIGHT TRAINING - Started %date% %time%
echo ============================================================

echo.
echo [1/10] Science Facts (10K short facts)...
python ingest_hf_conversations.py --dataset science-facts --save-every 500 >> overnight_log.txt 2>&1
echo   Done at %time%.

echo.
echo [2/10] SciQ (13.7K science Q+A with support paragraphs)...
python ingest_hf_conversations.py --dataset sciq --save-every 500 >> overnight_log.txt 2>&1
echo   Done at %time%.

echo.
echo [3/10] OpenBookQA (5.9K elementary science facts)...
python ingest_hf_conversations.py --dataset openbookqa --save-every 500 --min-len 50 >> overnight_log.txt 2>&1
echo   Done at %time%.

echo.
echo [4/10] ScienceQA (21K science Q+A with lectures)...
python ingest_hf_conversations.py --dataset scienceqa --save-every 500 >> overnight_log.txt 2>&1
echo   Done at %time%.

echo.
echo [5/10] Structured Wikipedia (limit 50K articles)...
python ingest_hf_conversations.py --dataset structured-wikipedia --save-every 1000 --limit 50000 >> overnight_log.txt 2>&1
echo   Done at %time%.

echo.
echo [6/10] CodeAlpaca (20K code instructions)...
python ingest_hf_conversations.py --dataset code-alpaca --save-every 500 >> overnight_log.txt 2>&1
echo   Done at %time%.

echo.
echo [7/10] Python Code (18K Python instructions)...
python ingest_hf_conversations.py --dataset python-code --save-every 500 >> overnight_log.txt 2>&1
echo   Done at %time%.

echo.
echo [8/10] Code-Feedback (multi-turn code Q+A)...
python ingest_hf_conversations.py --dataset code-feedback --save-every 500 --limit 30000 >> overnight_log.txt 2>&1
echo   Done at %time%.

echo.
echo [9/10] No Robots (10K human-written instructions)...
python ingest_hf_conversations.py --dataset no-robots --save-every 500 >> overnight_log.txt 2>&1
echo   Done at %time%.

echo.
echo [10/10] Evol-Instruct Code (80K evolved code, limit 40K)...
python ingest_hf_conversations.py --dataset evol-code --save-every 500 --limit 40000 >> overnight_log.txt 2>&1
echo   Done at %time%.

echo.
echo ============================================================
echo   ALL DONE - Finished %date% %time%
echo ============================================================
echo   Check overnight_log.txt for full details
echo ============================================================
pause
