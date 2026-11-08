#!/bin/bash
# sess.sh HOST JOB.ps1 [TIMEOUT_S]: run JOB.ps1 in the interactive desktop session of a Windows test machine (where windows can open; ssh's own
# session 0 cannot show any) and print what it wrote. The machine runs sessagent.ps1 at logon (setup.md); a job may start programs and return.
set -euo pipefail
host=$1; job=$2; t=${3:-120}; id=j$(date +%s%N)
scp -q "$job" "$host:C:/bp/sess/stage-$id.ps1"
ssh -o BatchMode=yes "$host" "Move-Item C:\\bp\\sess\\stage-$id.ps1 C:\\bp\\sess\\in\\$id.ps1; \$n = 0; while (-not (Test-Path C:\\bp\\sess\\out\\$id.done) -and \$n -lt $((t * 4))) { Start-Sleep -Milliseconds 250; \$n++ }; if (-not (Test-Path C:\\bp\\sess\\out\\$id.done)) { 'sess: job $id still running after ${t}s' }; Get-Content C:\\bp\\sess\\out\\$id.log, C:\\bp\\sess\\out\\$id.err -EA SilentlyContinue; if (Test-Path C:\\bp\\sess\\out\\$id.done) { exit [int](Get-Content C:\\bp\\sess\\out\\$id.done) } else { exit 124 }" 2>&1 | grep -v '^\*\*'
