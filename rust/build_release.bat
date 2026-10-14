@echo off
echo ════════════════════════════════════════
echo   Lu Ai — Release Build
echo ════════════════════════════════════════
cd /d <AI_FACTORY_ROOT>
<USER_HOME>\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin\cargo.exe build --release 2>&1
echo EXIT_CODE=%ERRORLEVEL%
