@echo off
setlocal
title Lu Ai - Training Session

echo.
echo  ============================================
echo   Lu Ai ^| Interactive Training Session
echo  ============================================
echo.
echo   Train multiple topics one after another.
echo   The brain saves after every topic.
echo   Type 'quit' when done, then restart Claude.
echo.
echo   Tip: Prefix with a number for more facts:
echo        20 ocean creatures
echo.

:: ── Kill MCP to avoid conflicts ──────────────
echo  Stopping Lu Ai MCP server...
taskkill /f /im lu_ai_mcp.exe >nul 2>&1
timeout /t 1 /nobreak >nul
echo  Done. Starting training session...
echo.

:: ── Interactive mode ──────────────────────────
cd /d <AI_FACTORY_ROOT>\tools
python train.py --interactive %*

:: ── Done ─────────────────────────────────────
echo.
echo  ============================================
echo   Session ended.
echo.
echo   Restart Claude Desktop to load the
echo   updated brain into the MCP server.
echo  ============================================
echo.
pause
