@echo off
rem Builds the Lu MCP binary. Run from anywhere; uses this file's folder as the root.
set "CD_ROOT=%~dp0"
if not exist "%CD_ROOT%logs" mkdir "%CD_ROOT%logs"
cd /d "%CD_ROOT%rust\v2"
cargo build --release --bin lu_mcp_v2_cd > "%CD_ROOT%logs\winbuild.log" 2>&1
echo WINBUILD_EXIT=%errorlevel% >> "%CD_ROOT%logs\winbuild.log"
