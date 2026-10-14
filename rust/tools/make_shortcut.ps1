# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

$WshShell = New-Object -comObject WScript.Shell
$Shortcut = $WshShell.CreateShortcut("<USER_HOME>\OneDrive\Desktop\Lu Ai.lnk")
$Shortcut.TargetPath = "<AI_FACTORY_ROOT>\target\release\lu_ai.exe"
$Shortcut.WorkingDirectory = "<AI_FACTORY_ROOT>"
$Shortcut.Description = "Lu Ai - Associative Memory Terminal"
$Shortcut.IconLocation = "<AI_FACTORY_ROOT>\assets\lu_ai.ico"
$Shortcut.Save()
Write-Host "Shortcut created with Lu Ai icon"
