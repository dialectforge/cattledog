@echo off
setlocal
title Lu Ai - Ollama Training

echo.
echo  ============================================
echo   Lu Ai ^| Ollama Training
echo  ============================================
echo.

:: ── Usage check ──────────────────────────────
if "%~1"=="" (
    echo  Usage:   train_ollama.bat "topic"
    echo  Usage:   train_ollama.bat "topic" --count 12
    echo  Usage:   train_ollama.bat "topic" --model mistral
    echo.
    echo  Examples:
    echo    train_ollama.bat "volcanoes and lava"
    echo    train_ollama.bat "cooking techniques" --count 12
    echo    train_ollama.bat "quantum physics" --model mistral
    echo.
    echo  For multiple topics in one session, use:
    echo    train_session.bat
    echo.
    pause
    exit /b 1
)

:: ── Kill Claude Desktop MCP to avoid brain file conflicts ────────────────
echo  [1/3] Stopping Lu Ai MCP server...
taskkill /f /im lu_ai_mcp.exe >nul 2>&1
timeout /t 1 /nobreak >nul
echo        Done.
echo.

:: ── Run training ─────────────────────────────
echo  [2/3] Training: %*
echo.
cd /d <AI_FACTORY_ROOT>\tools
python train.py %*

:: ── Done ─────────────────────────────────────
echo.
echo  ============================================
echo   [3/3] Training complete!
echo.
echo   Next step: Restart Claude Desktop to load
echo   the updated brain into the MCP server.
echo  ============================================
echo.
pause
