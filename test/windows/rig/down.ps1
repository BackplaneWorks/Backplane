# down.ps1 -Name RUN: stop the hub and client up.ps1 started (by their saved pids only), and the hub's process tree.
param([string]$Name = 'r1')
$w = "C:\bp\runs\$Name"
foreach ($l in Get-Content "$w\pids" -EA SilentlyContinue) { $p = ($l -split '=')[1]; if ($p) { & taskkill.exe /T /F /PID $p 2>&1 | Out-Null; "stopped $l" } }
Remove-Item "$w\pids" -EA SilentlyContinue
