@echo off
python <AI_FACTORY_ROOT>\tools\test_ollama.py mistral:7b > <AI_FACTORY_ROOT>\tools\ollama_result.txt 2>&1
echo EXIT_CODE=%ERRORLEVEL% >> <AI_FACTORY_ROOT>\tools\ollama_result.txt
