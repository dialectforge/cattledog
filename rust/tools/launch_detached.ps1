# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC
# Kill any leftover MCP servers first
Stop-Process -Name "ai_factory_mcp" -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2

# Create scheduled task that runs independently
$action = New-ScheduledTaskAction -Execute "python.exe" -Argument "-u <AI_FACTORY_ROOT>\tools\train_all_books.py" -WorkingDirectory "<AI_FACTORY_ROOT>\tools"
$trigger = New-ScheduledTaskTrigger -Once -At (Get-Date).AddSeconds(5)
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Hours 24)
Unregister-ScheduledTask -TaskName "LuPDFTraining" -Confirm:$false -ErrorAction SilentlyContinue
Register-ScheduledTask -TaskName "LuPDFTraining" -Action $action -Trigger $trigger -Settings $settings -Force
Write-Host "Task registered. Starting now..."
Start-ScheduledTask -TaskName "LuPDFTraining"
Start-Sleep -Seconds 3
$task = Get-ScheduledTask -TaskName "LuPDFTraining"
Write-Host "Task state: $($task.State)"
