# SPDX-License-Identifier: GPL-3.0-or-later
# Run on Windows, with an isolated ProgramData and all shutdown/network actions blocked.
# Optional -Python proves a real sleeping Python harness and its child independently.
param([string]$Watchdog = "$PSScriptRoot\..\scripts\winvm\bp-idle-watchdog.ps1", [string]$Python = '')
$ErrorActionPreference = 'Stop'
$oldProgramData = $env:ProgramData
$root = Join-Path ([IO.Path]::GetTempPath()) ('bp-watchdog-proof-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $root | Out-Null
$env:ProgramData = $root
$global:WatchdogProofNames = @()
$global:WatchdogProofIds = @()
$parent = $null; $childId = $null
function Get-Process {
    if ($global:WatchdogProofIds.Count) { Microsoft.PowerShell.Management\Get-Process -Id $global:WatchdogProofIds }
    else { foreach ($name in $global:WatchdogProofNames) { [pscustomobject]@{ ProcessName = $name; CPU = 0; SessionId = 1 } } }
}
function Get-NetTCPConnection { }
function qwinsta { }
function Get-WinEvent { }
function Invoke-RestMethod { throw 'Network shutdown action attempted in proof' }
function Invoke-WebRequest { throw 'Network shutdown action attempted in proof' }
function shutdown.exe { throw 'OS shutdown attempted in proof' }
function Check([string]$name, [bool]$busy, [string]$reason, [hashtable]$options = @{}) {
    & $Watchdog -WhatIf -IdleMinutes 0 @options
    $s = Get-Content (Join-Path $root 'bp-watchdog\status.json') -Raw | ConvertFrom-Json
    if (!$s.dryRun -or $s.busy -ne $busy -or $s.why -notlike "*$reason*") { throw "$name failed: $($s | ConvertTo-Json -Compress)" }
    Write-Output "PASS $name : busy=$($s.busy), $($s.why)"
}
try {
    Check 'idle negative control reaches dry-run shutdown branch' $false ''
    $gui = 'backplane-hub,backplane-gpui,backplane,kicad,kicad-cli,pcbnew,eeschema,python,pythonw'
    foreach ($name in $gui.Split(',')) {
        $global:WatchdogProofNames = @($name)
        Check "zero-CPU console process $name" $true "process $name" @{ ExtraBusyProcesses = $gui }
    }
    foreach ($name in @('scp', 'sftp-server', 'robocopy')) {
        $global:WatchdogProofNames = @($name)
        Check "transfer $name" $true "process $name"
    }
    $global:WatchdogProofNames = @()
    $tree = Join-Path $root 'activity'; $nested = Join-Path $tree 'nested'
    New-Item -ItemType Directory $nested -Force | Out-Null
    $file = Join-Path $nested 'existing.txt'
    Set-Content $file 'old'
    $old = (Get-Date).AddHours(-2)
    (Get-Item $file).LastWriteTime = $old
    (Get-Item $nested).LastWriteTime = $old
    (Get-Item $tree).LastWriteTime = $old
    Check 'old files are idle' $false '' @{ RecentWriteDir = $tree }
    Add-Content $file 'overwrite existing file'
    # Make the directory timestamps explicitly old, so only the file timestamp can protect the VM.
    (Get-Item $nested).LastWriteTime = $old
    (Get-Item $tree).LastWriteTime = $old
    Check 'existing file overwrite' $true 'existing.txt' @{ RecentWriteDir = $tree }
    (Get-Item $file).LastWriteTime = $old
    (Get-Item $nested).LastWriteTime = Get-Date
    Check 'nested folder change' $true 'nested' @{ RecentWriteDir = $tree }
    # A junction (bp-win has C:\bp\kicad-b12). Setting LastWriteTime on one follows it to its target, so the
    # junction's own time can't be made old here: the target gets a file dated an hour ahead instead, and a
    # 0-minute window makes everything already written idle; walking into the junction would find that file.
    (Get-Item $nested).LastWriteTime = $old
    $target = Join-Path $root 'junction-target'
    New-Item -ItemType Directory $target -Force | Out-Null
    $ahead = Join-Path $target 'ahead.txt'
    Set-Content $ahead 'outside the tree'
    (Get-Item $ahead).LastWriteTime = (Get-Date).AddHours(1)
    $link = Join-Path $tree 'link'
    cmd /c mklink /J $link $target | Out-Null
    (Get-Item $tree).LastWriteTime = $old
    Start-Sleep -Seconds 2
    Check 'junction target is not walked' $false '' @{ RecentWriteDir = $tree; RecentWriteMinutes = 0 }
    Check 'a new junction counts by its own write time' $true 'link' @{ RecentWriteDir = $tree }
    cmd /c rmdir $link | Out-Null
    (Get-Item $tree).LastWriteTime = $old
    Check 'unreadable activity tree is safe' $true 'unreadable' @{ RecentWriteDir = (Join-Path $root 'missing') }
    $jobs = Join-Path $root 'jobs'
    New-Item -ItemType Directory "$jobs\in", "$jobs\out" -Force | Out-Null
    Set-Content "$jobs\in\queued.ps1" '# waiting'
    Check 'queued session job' $true 'session job x1' @{ JobDir = $jobs }
    Move-Item "$jobs\in\queued.ps1" "$jobs\out\queued.ps1"
    Check 'running session job' $true 'session job x1' @{ JobDir = $jobs }
    Set-Content "$jobs\out\queued.done" 'done'
    Check 'finished session job is idle' $false '' @{ JobDir = $jobs }
    $hold = Join-Path $root 'bp-watchdog\hold-until.txt'
    foreach ($hours in @(6, 13)) {
        $value = (Get-Date).AddHours($hours).ToString('o')
        Set-Content $hold $value
        Check "hold wins ($hours hours)" $true 'hold until'
        if ((Get-Content $hold -Raw).Trim() -ne $value) { throw 'Hold was changed' }
    }
    Set-Content $hold 'incomplete write'
    Check 'malformed hold is safe' $true 'hold unreadable'
    Set-Content $hold (Get-Date).AddMinutes(-1).ToString('o')
    Check 'expired hold is idle' $false ''
    if (!(Test-Path $hold)) { throw 'Watchdog deleted a hold it might race with' }
    Remove-Item $hold
    if ($Python) {
        $pidFile = Join-Path $root 'child-pid.txt'
        $harness = Join-Path $root 'harness.py'
        @'
import pathlib, subprocess, sys, threading
child = subprocess.Popen([sys.executable, '-c', 'import threading; threading.Event().wait(300)'])
pathlib.Path(sys.argv[1]).write_text(str(child.pid))
threading.Event().wait(300)
'@ | Set-Content $harness
        # Do not pass the isolated ProgramData to Windows shell launchers or the harness.
        $env:ProgramData = $oldProgramData
        try { $parent = Start-Process -FilePath $Python -ArgumentList @($harness, $pidFile) -PassThru -WindowStyle Hidden }
        finally { $env:ProgramData = $root }
        for ($n=0; $n -lt 30 -and !(Test-Path $pidFile); $n++) { Start-Sleep -Milliseconds 100 }
        $childId = [int](Get-Content $pidFile -Raw)
        foreach ($id in @($parent.Id, $childId)) {
            $global:WatchdogProofIds = @($id)
            Check "real sleeping Python PID $id" $true 'process python' @{ ExtraBusyProcesses = $gui }
        }
        $global:WatchdogProofIds = @()
    }
    Write-Output 'PASS watchdog proofs; no real hold, task, build or shutdown action touched'
} finally {
    if ($childId) {
        $child = Microsoft.PowerShell.Management\Get-Process -Id $childId -ErrorAction SilentlyContinue
        if ($child) { Stop-Process -Id $childId; $null = $child.WaitForExit(5000) }
    }
    if ($parent -and !$parent.HasExited) { Stop-Process -Id $parent.Id; $null = $parent.WaitForExit(5000) }
    Remove-Variable WatchdogProofNames, WatchdogProofIds -Scope Global
    $env:ProgramData = $oldProgramData
    Remove-Item -Recurse -Force $root
}
