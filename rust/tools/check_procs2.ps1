# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

Get-CimInstance Win32_Process -Filter "name='python.exe'" | Select-Object ProcessId,CommandLine,CreationDate | Format-List
