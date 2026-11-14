#!/bin/bash
# sess.sh HOST JOB.ps1 [TIMEOUT_S]: run JOB.ps1 in the interactive desktop session of a Windows test machine (where windows can open; ssh's own
# session 0 cannot show any) and print what it wrote. The machine runs sessagent.ps1 at logon (setup.md); a job may start programs and return.
set -euo pipefail
host=$1; job=$2; t=${3:-120}; id=j$(date +%s%N)
# Cancelled here (TERM/INT/HUP): the job must not live on in the desktop session, or the next job's pre-run check finds
# its programs and refuses to start. Take it out of the queue if it has not started; else stop what it started from C:\bp
# (the test's Python, its hub and client, KiCad); nothing else is touched.
cancel() {
  trap - TERM INT HUP
  [ -n "${waiter:-}" ] && kill "$waiter" 2>/dev/null
  ssh -o BatchMode=yes "$host" "Remove-Item C:\\bp\\sess\\in\\$id.ps1, C:\\bp\\sess\\stage-$id.ps1 -EA SilentlyContinue; Get-CimInstance Win32_Process | Where-Object { \$_.Name -match '^(python|python3|backplane-hub|backplane-gpui|kicad|kicad-cli)\\.exe\$' -and (\$_.CommandLine -match 'C:\\\\bp\\\\' -or \$_.ExecutablePath -match '^C:\\\\bp\\\\') } | ForEach-Object { Stop-Process -Id \$_.ProcessId -Force -EA SilentlyContinue; 'sess: cancelled, stopped ' + \$_.ProcessId + ' ' + \$_.Name }" 2>&1 | grep -av '^\*\*' || true
  exit 130
}
trap cancel TERM INT HUP
scp -q "$job" "$host:C:/bp/sess/stage-$id.ps1"
# in the background with `wait`: bash runs the cancel trap only between foreground commands, and this one waits up to ${t}s
( set -o pipefail; ssh -o BatchMode=yes "$host" "Move-Item C:\\bp\\sess\\stage-$id.ps1 C:\\bp\\sess\\in\\$id.ps1; \$n = 0; while (-not (Test-Path C:\\bp\\sess\\out\\$id.done) -and \$n -lt $((t * 4))) { Start-Sleep -Milliseconds 250; \$n++ }; if (-not (Test-Path C:\\bp\\sess\\out\\$id.done)) { 'sess: job $id still running after ${t}s' }; Get-Content C:\\bp\\sess\\out\\$id.log, C:\\bp\\sess\\out\\$id.err -EA SilentlyContinue; if (Test-Path C:\\bp\\sess\\out\\$id.done) { exit [int](Get-Content C:\\bp\\sess\\out\\$id.done) } else { exit 124 }" 2>&1 | grep -av '^\*\*' | sed 's/^\xEF\xBB\xBF//' ) &
waiter=$!
wait "$waiter"
