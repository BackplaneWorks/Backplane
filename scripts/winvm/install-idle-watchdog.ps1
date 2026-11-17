# SPDX-License-Identifier: GPL-3.0-or-later
# Install (or update) the idle watchdog on this Azure VM: copies bp-idle-watchdog.ps1 to
# C:\ProgramData\bp-watchdog and registers the scheduled task "bp-idle-watchdog", run as SYSTEM every
# minute. Run it elevated, from the folder holding both scripts (winvm-push.sh copies them over ssh):
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File install-idle-watchdog.ps1 -IdleMinutes 20
#
# bp-winbuild: -IdleMinutes 20.  bp-win: -IdleMinutes 30 -JobDir C:\bp\sess (its session agent's jobs).
param([int]$IdleMinutes = 20, [string]$JobDir = '', [switch]$WhatIf)
$ErrorActionPreference = 'Stop'
$dir = Join-Path $env:ProgramData 'bp-watchdog'
New-Item -ItemType Directory -Force $dir | Out-Null
Copy-Item -Force (Join-Path $PSScriptRoot 'bp-idle-watchdog.ps1') $dir
# Only SYSTEM and Administrators may change what SYSTEM runs
icacls $dir /inheritance:r /grant:r 'SYSTEM:(OI)(CI)F' 'Administrators:(OI)(CI)F' | Out-Null
$arguments = "-NoProfile -NonInteractive -ExecutionPolicy Bypass -File `"$dir\bp-idle-watchdog.ps1`" -IdleMinutes $IdleMinutes"
if ($JobDir) { $arguments += " -JobDir `"$JobDir`"" }
if ($WhatIf) { $arguments += ' -WhatIf' }
$action = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument $arguments
$every = New-ScheduledTaskTrigger -Once -At (Get-Date).AddMinutes(1) -RepetitionInterval (New-TimeSpan -Minutes 1)
$boot = New-ScheduledTaskTrigger -AtStartup
$settings = New-ScheduledTaskSettingsSet -MultipleInstances IgnoreNew -StartWhenAvailable `
    -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Minutes 4)
$principal = New-ScheduledTaskPrincipal -UserId 'SYSTEM' -LogonType ServiceAccount -RunLevel Highest
Register-ScheduledTask -TaskName 'bp-idle-watchdog' -Action $action -Trigger $every, $boot -Settings $settings `
    -Principal $principal -Description "Backplane: deallocate this VM after $IdleMinutes min with no build or session" -Force | Out-Null
Start-ScheduledTask -TaskName 'bp-idle-watchdog'
Start-Sleep 8
Get-ScheduledTask 'bp-idle-watchdog' | Get-ScheduledTaskInfo | Select-Object LastRunTime, LastTaskResult, NextRunTime | Format-List
Get-Content (Join-Path $dir 'status.json') -ErrorAction SilentlyContinue
