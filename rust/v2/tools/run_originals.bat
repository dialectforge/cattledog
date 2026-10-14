@echo off
REM Chain original 5 datasets that were cut off at ~50K
REM Resume-safe: skips anything already ingested via CSV log

D:
cd <AI_FACTORY_ROOT>\v2\tools

echo ============================================================
echo   ORIGINAL DATASETS TRAINING - Started %date% %time%
echo ============================================================

echo.
echo [1/5] DailyDialog (13K conversations)...
python ingest_hf_conversations.py --dataset dailydialog --save-every 500 >> overnight_log.txt 2>&1
echo   Done at %time%.

echo.
echo [2/5] ELI5 (272K Q+A pairs)...
python ingest_hf_conversations.py --dataset eli5 --save-every 500 >> overnight_log.txt 2>&1
echo   Done at %time%.

echo.
echo [3/5] Textbooks (1.8K full textbooks)...
python ingest_hf_conversations.py --dataset textbooks --save-every 200 >> overnight_log.txt 2>&1
echo   Done at %time%.

echo.
echo [4/5] Tiny-Textbooks (420K synthetic docs)...
python ingest_hf_conversations.py --dataset tiny-textbooks --save-every 1000 >> overnight_log.txt 2>&1
echo   Done at %time%.

echo.
echo [5/5] UltraChat (207K multi-turn chats)...
python ingest_hf_conversations.py --dataset ultrachat --save-every 1000 >> overnight_log.txt 2>&1
echo   Done at %time%.

echo.
echo ============================================================
echo   ORIGINALS DONE - Finished %date% %time%
echo ============================================================
pause
