# SPDX-License-Identifier: GPL-3.0-or-later
# Idle watchdog for Backplane's Azure Windows VMs (bp-winbuild, bp-win). It runs on the VM itself, as
# SYSTEM, once a minute (install-idle-watchdog.ps1 registers it). When the VM has gone -IdleMinutes with
# no build or job process and no ssh or RDP session, it deallocates its own VM through Azure Resource
# Manager with the VM's managed identity (role "Backplane VM self-deallocate", scoped to that VM only),
# so the compute meter stops. If Azure can't be reached, it shuts Windows down: a VM that is stopped
# but not deallocated still bills, so the desktop's watcher (scripts/winvm/winvm-watch.sh) deallocates
# any VM it finds stopped.
#
# Busy means any of:
#  - a process in -BusyProcesses (build tools; the GitHub runner's job worker; scp, sftp-server, robocopy) or
#    -ExtraBusyProcesses (bp-win: Backplane, KiCad, python) is alive, in any session;
#  - with -RecentWriteDir, a file or folder in that tree was written recently;
#  - an ssh session is open (an established connection to port 22, a minute old or more);
#  - an ssh login happened since (a lane's short polls: OpenSSH's event log);
#  - an RDP session is active (an rdp-tcp# session; the auto-logon console session never counts);
#  - with -JobDir, a session-agent job is waiting or running;
#  - a hold (C:\ProgramData\bp-watchdog\hold-until.txt, set by winvm-hold.sh) has not run out.
# Each tick writes C:\ProgramData\bp-watchdog\status.json ({busy, why, idleSince, lastBusy, ...}),
# which the desktop's watcher reads, and appends a line to watchdog.log when something changes.
param(
    [int]$IdleMinutes = 20,
    [string[]]$BusyProcesses = @('cl', 'link', 'lib', 'ninja', 'cmake', 'ctest', 'vcpkg', 'msbuild',
        'ccache', 'sccache', 'rc', 'mt', 'git', 'git-lfs', 'Runner.Worker',
        # a copy in progress
        'scp', 'sftp-server', 'robocopy'),
    # More process names that are busy while alive, in any session, the console's included (bp-win: the GUI
    # tests' Backplane, KiCad and the python harness, which may wait on a window for minutes using no CPU)
    [string[]]$ExtraBusyProcesses = @(),
    # A folder (bp-win: C:\bp): recent file writes and folder changes both count, including overwrites.
    [string]$RecentWriteDir = '',
    [int]$RecentWriteMinutes = 30,
    # A session agent's job folder (bp-win: C:\bp\sess): a job waiting in in\ or running (out\<id>.ps1
    # with no out\<id>.done, written in the last 6 hours) is busy
    [string]$JobDir = '',
    # Log what it would do, never deallocate or shut down (a dry run)
    [switch]$WhatIf
)
$ErrorActionPreference = 'Stop'
$dir = Join-Path $env:ProgramData 'bp-watchdog'
New-Item -ItemType Directory -Force $dir | Out-Null
$statePath = Join-Path $dir 'state.json'
$statusPath = Join-Path $dir 'status.json'
$logPath = Join-Path $dir 'watchdog.log'
function Log([string]$m) {
    $line = '{0:yyyy-MM-ddTHH:mm:ssK} {1}' -f (Get-Date), $m
    Add-Content -LiteralPath $logPath $line
    if ((Get-Item $logPath).Length -gt 1MB) { Move-Item -Force $logPath "$logPath.1" }
}
# Windows PowerShell 5.1 (bp-win has no pwsh) serialises dates as \/Date()\/: keep ISO strings
$now = Get-Date
$nowS = $now.ToString('o')
$bootS = (Get-CimInstance Win32_OperatingSystem).LastBootUpTime.ToString('o')
$state = $null
if (Test-Path $statePath) { try { $state = Get-Content $statePath -Raw | ConvertFrom-Json } catch { $state = $null } }
# A new boot starts a new idle period: the time before the VM was started is not idle time
if (!$state -or $state.boot -ne $bootS) {
    $state = [pscustomobject]@{ boot = $bootS; idleSince = $nowS; lastBusy = $null; lastAttempt = $null; lastWhy = 'boot' }
    Log "watchdog: boot $bootS, idle limit $IdleMinutes min$(if ($WhatIf) { ' (dry run)' })"
}

$why = @()
$procs = Get-Process -ErrorAction SilentlyContinue
# powershell -File passes 'a,b,c' as one string: split it
$names = (@($BusyProcesses) + @($ExtraBusyProcesses)) -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ } |
    ForEach-Object { $_.ToLowerInvariant() }
$busyProcs = $procs | Where-Object { $names -contains $_.ProcessName.ToLowerInvariant() } |
    Select-Object -ExpandProperty ProcessName -Unique
if ($busyProcs) { $why += 'process ' + ($busyProcs -join ',') }
# ssh: established connections to port 22 older than a minute. Counting sshd processes is wrong: a
# half-open connection leaves an sshd -R behind for hours (seen on bp-win), and the desktop watcher's
# status probe lasts seconds
$ssh = @(Get-NetTCPConnection -State Established -LocalPort 22 -ErrorAction SilentlyContinue |
    Where-Object { $_.CreationTime -lt $now.AddMinutes(-1) })
if ($ssh.Count) { $why += "ssh x$($ssh.Count)" }
# RDP: an active rdp-tcp#N session (the console session, auto-logon on bp-win, is not a person)
$rdp = @(qwinsta 2>$null | Select-String -Pattern '^\s*>?rdp-tcp#\d+\s+\S+\s+\d+\s+Active' )
if ($rdp.Count) { $why += "rdp x$($rdp.Count)" }

if ($RecentWriteDir) {
    $cut = $now.AddMinutes(-$RecentWriteMinutes)
    $recent = $null
    try {
        $pending = New-Object 'System.Collections.Generic.Stack[System.IO.DirectoryInfo]'
        $pending.Push((New-Object IO.DirectoryInfo $RecentWriteDir))
        while ($pending.Count -and !$recent) {
            $folder = $pending.Pop()
            if (!$folder.Exists) { throw "Missing activity folder $($folder.FullName)" }
            if ($folder.LastWriteTime -gt $cut) { $recent = $folder.FullName; break }
            $entries = $folder.EnumerateFileSystemInfos().GetEnumerator()
            try {
                while ($entries.MoveNext()) {
                    $entry = $entries.Current
                    if ($entry.LastWriteTime -gt $cut) { $recent = $entry.FullName; break }
                    if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) {
                        # Do not traverse junctions (cycles/outside the tree), or infer idle from an incomplete scan.
                        $recent = "$($entry.FullName) (junction: counted busy)"; break
                    }
                    if ($entry -is [IO.DirectoryInfo]) { $pending.Push($entry) }
                }
            } finally { $entries.Dispose() }
        }
    } catch { $recent = "$RecentWriteDir (unreadable: counted busy)" }
    if ($recent) { $why += "written $recent" }
}
if ($JobDir -and (Test-Path $JobDir)) {
    $waiting = @(Get-ChildItem (Join-Path $JobDir 'in') -Filter *.ps1 -ErrorAction SilentlyContinue)
    $running = @(Get-ChildItem (Join-Path $JobDir 'out') -Filter *.ps1 -ErrorAction SilentlyContinue | Where-Object {
        $_.LastWriteTime -gt $now.AddHours(-6) -and !(Test-Path (Join-Path $_.DirectoryName "$($_.BaseName).done")) })
    if ($waiting.Count + $running.Count) { $why += "session job x$($waiting.Count + $running.Count)" }
}

# An unexpired hold always wins. Never delete the file here: another lane may be refreshing it.
# winvm-hold.sh limits new holds to 12 hours; a malformed hold is treated as busy until repaired.
$holdPath = Join-Path $dir 'hold-until.txt'
if (Test-Path $holdPath) {
    try {
        $until = [datetime](Get-Content $holdPath -Raw).Trim()
        if ($until -gt $now) { $why += "hold until $($until.ToString('o'))" }
    } catch { $why += 'hold unreadable: counted busy' }
}

$busy = $why.Count -gt 0
# A lane polls with short ssh commands the minute's sample misses: its last login counts as activity
$lastLogin = $null
try {
    $lastLogin = Get-WinEvent -FilterHashtable @{ LogName = 'OpenSSH/Operational'; StartTime = $now.AddMinutes(-2 * $IdleMinutes) } -ErrorAction Stop |
        Where-Object { $_.Message -like '*Accepted publickey*' -or $_.Message -like '*Accepted password*' } |
        Select-Object -First 1 -ExpandProperty TimeCreated
} catch { }
if (!$busy -and $lastLogin -and $lastLogin -gt [datetime]$state.idleSince) {
    $state.idleSince = $lastLogin.ToString('o'); $state.lastBusy = $state.idleSince
}
if ($busy) {
    if ($state.lastWhy -eq 'idle') { Log ('busy: ' + ($why -join '; ')) }
    $state.idleSince = $nowS; $state.lastBusy = $nowS; $state.lastWhy = ($why -join '; ')
} elseif ($state.lastWhy -ne 'idle') {
    Log "idle (was: $($state.lastWhy))"; $state.lastWhy = 'idle'
}
$idleFor = ($now - [datetime]$state.idleSince).TotalMinutes
[pscustomobject]@{
    at = $nowS; boot = $bootS; busy = $busy; why = ($why -join '; ')
    idleSince = $state.idleSince; idleMinutes = [math]::Round($idleFor, 1)
    limitMinutes = $IdleMinutes; lastBusy = $state.lastBusy; dryRun = [bool]$WhatIf
} | ConvertTo-Json | Set-Content -LiteralPath $statusPath -Encoding utf8

if (!$busy -and $idleFor -ge $IdleMinutes) {
    # One attempt every 5 minutes: a deallocate takes a minute or two to reach the VM
    $last = if ($state.lastAttempt) { [datetime]$state.lastAttempt } else { [datetime]::MinValue }
    if (($now - $last).TotalMinutes -ge 5) {
        $state.lastAttempt = $nowS
        $state | ConvertTo-Json | Set-Content -LiteralPath $statePath -Encoding utf8
        Log ("idle {0:N0} min (limit {1}): deallocating{2}" -f $idleFor, $IdleMinutes, $(if ($WhatIf) { ' (dry run: not)' }))
        if ($WhatIf) { return }
        try {
            $imds = @{ Metadata = 'true' }
            $compute = Invoke-RestMethod -Headers $imds -TimeoutSec 10 -Uri 'http://169.254.169.254/metadata/instance/compute?api-version=2021-02-01'
            $token = (Invoke-RestMethod -Headers $imds -TimeoutSec 10 -Uri 'http://169.254.169.254/metadata/identity/oauth2/token?api-version=2018-02-01&resource=https%3A%2F%2Fmanagement.azure.com%2F').access_token
            $uri = "https://management.azure.com/subscriptions/$($compute.subscriptionId)/resourceGroups/$($compute.resourceGroupName)/providers/Microsoft.Compute/virtualMachines/$($compute.name)/deallocate?api-version=2024-07-01"
            $r = Invoke-WebRequest -Method Post -Uri $uri -Headers @{ Authorization = "Bearer $token" } -TimeoutSec 30 -UseBasicParsing
            Log "deallocate accepted by Azure (HTTP $($r.StatusCode)) for $($compute.name)"
        } catch {
            Log "deallocate through Azure failed ($($_.Exception.Message)): shutting Windows down; the desktop watcher deallocates a stopped VM"
            shutdown.exe /s /f /t 30 /d p:0:0 /c 'Backplane idle watchdog: no build or session'
        }
        return
    }
}
$state | ConvertTo-Json | Set-Content -LiteralPath $statePath -Encoding utf8
