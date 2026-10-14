# SPDX-License-Identifier: Apache-2.0
# Copyright (C) 2026 DialectForge LLC

$logOut = '<AI_FACTORY_ROOT>\v2\tools\hf_ingest_run.log'
$logErr = '<AI_FACTORY_ROOT>\v2\tools\hf_ingest_err.log'
$script = '<AI_FACTORY_ROOT>\v2\tools\ingest_huggingface.py'
$proc = Start-Process -FilePath python -ArgumentList $script, '--limit', '50000' -WorkingDirectory '<AI_FACTORY_ROOT>\v2\tools' -RedirectStandardOutput $logOut -RedirectStandardError $logErr -NoNewWindow -PassThru
Write-Host "Started PID: $($proc.Id)"
