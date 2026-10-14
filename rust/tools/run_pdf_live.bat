@echo off
echo Killing any existing MCP server...
taskkill /F /IM ai_factory_mcp.exe 2>nul
timeout /t 2 /nobreak >nul

echo Starting PDF training (pages 15-100)...
python -u <AI_FACTORY_ROOT>\tools\train_pdf.py "<USER_HOME>\OneDrive\Desktop\Workplace_Software_and_Skills_-_WEB_IlfJtcP.pdf" --start-page 15 --end-page 100 --model mistral:7b --batch-size 10 > <AI_FACTORY_ROOT>\tools\pdf_live_output.txt 2>&1
echo DONE >> <AI_FACTORY_ROOT>\tools\pdf_live_output.txt
echo Training finished!
pause
