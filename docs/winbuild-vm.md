# Windows VMs: bp-winbuild (KiCad builds) and bp-win (UI tests)

Both are in Azure, resource group `BP-WIN-RG`, region westus2, on the same VNet and tailnet.

| VM | Size | Use | Compute (only while running) | Disks (always billed) |
|---|---|---|---|---|
| bp-winbuild | Standard_F32als_v7, 32 vCPU AMD, 64 GB | Windows KiCad builds | $3.41/h (Windows licence included) | OS 128 GB P10 $17.92/mo + data 256 GB P15 $34.56/mo = $52.48/mo |
| bp-win | Standard_D4s_v5, 4 vCPU, 16 GB | Windows UI tests (auto-logon console, session agent) | $0.376/h | OS P10 |

Westus2 list prices, November 2026. F32s_v2 is a legacy family that Azure gives no new quota for;
F32als_v7 is its current equivalent (2 GB per vCPU, no temp disk, NVMe). Quota: Falsv7 32, region 40.

## Windows KiCad builds run on bp-winbuild

```
scripts/winbuild-kicad.sh <fork commit> [--scripts-from <ref>] [--clean-build] [--libraries]
```

It takes the `winbuild-vm` lock, starts the VM (`scripts/winbuild-up.sh`), installs or checks the toolchain
(`scripts/backplane-windows-toolchain.ps1` in the fork), checks the commit out in `C:\src\kicad`, builds with
the fork's `scripts/backplane-windows-local.ps1 -Root D:\bpk -Jobs 32 -Ccache` (vcpkg's binary cache in
`D:\bpk\vcpkg-cache`, ccache in `D:\bpk\ccache`, both on the persistent data disk), copies the package to
`~/.cache/bp-winbuild/out/<commit>/` and deallocates the VM (`scripts/winbuild-down.sh`). Anything else that
uses the VM holds the `winbuild-vm` lock for as long as it does. ssh: `bp-winbuild` (Tailscale), or
`bp-winbuild-vnet` (through bp-win) if Tailscale is down. land.sh's Windows gate reaches it at its Azure public address
(`bp-winbuild-gate-ip`, made with `scripts/winvm/winvm-gate-net.sh`); its NSG `bp-winbuild-gate-nsg` lets nothing in from the internet
but the gate's per-run rule `land-gate-ssh` (TCP 22 from the desktop's address alone, deleted at the end).

bp-win no longer builds KiCad; it stays the UI test box. Lanes start it with `az vm start -g BP-WIN-RG -n bp-win`
when they need it and hold `bp-win-ui` while they use it.

## Build times (bp.13.1, 7038d2231b, 2026-11-17)

| | GitHub windows-2022 (4 cores) | bp-winbuild cold (empty caches) | bp-winbuild warm (fresh build folder) |
|---|---|---|---|
| vcpkg dependencies + configure | (vcpkg cache restored) | 43 min 25 s | 1 min 52 s |
| compile | | 8 min 36 s (32 jobs) | 1 min 49 s (ccache 2507/2508 hits) |
| whole build script | 2 h 02 min – 2 h 03 min | 53 min 30 s | 5 min 53 s |
| whole run | 2 h 05 min – 2 h 06 min | 1 h 0 min (incl. first toolchain install, 5.5 min) | 6 min 2 s, plus about 2 min to start the VM |

Caches after one build: ccache 2.3 GB (of 40 GB), vcpkg binary cache 1.5 GB. A warm Windows build costs about
$0.50 of compute. bp-win (4 cores, as GitHub's runner) was not timed for a full compile here.

## Making sure they shut off

A stopped VM that is not *deallocated* still bills for compute, so every layer ends in a deallocate.

1. **On the VM: idle watchdog** (`scripts/winvm/bp-idle-watchdog.ps1`, scheduled task `bp-idle-watchdog`,
   SYSTEM, every minute; installed with `scripts/winvm/winvm-push.sh`). Busy: a build or job process (cl,
   link, ninja, cmake, vcpkg, ccache, git, the GitHub runner's `Runner.Worker`, ...), an ssh connection a minute
   old or more, an ssh login since (lanes' short polls, from OpenSSH's event log), an active RDP session (the
   console session doesn't count), and on bp-win also: a session-agent job waiting or running (`C:\bp\sess`); the GUI tests' processes alive in any
   session, the console's included (backplane-hub, backplane-gpui, backplane, kicad, kicad-cli, pcbnew, eeschema, python,
   pythonw: a harness waiting on a KiCad window with no CPU still counts); a copy (scp, sftp-server, robocopy); anything
   written (including overwrites), created, renamed or deleted under `C:\bp` in the last 30 minutes; and a hold (`winvm-hold.sh`), which always wins. After
   20 idle minutes (bp-winbuild) or 30 (bp-win) it deallocates its own VM through Azure Resource Manager with the
   VM's system-assigned identity, whose only role is the custom **Backplane VM self-deallocate** (read and
   deallocate), scoped to that one VM. If Azure can't be reached it shuts Windows down, and layer 3 deallocates.
   Log: `C:\ProgramData\bp-watchdog\watchdog.log`; state each minute: `status.json`.
2. **Azure auto-shutdown** (DevTestLab schedule, as bp-win has had): bp-winbuild daily at 05:00 Eastern;
   bp-win daily at 07:00 UTC (unchanged; its backstop). Azure's shutdown deallocates.
3. **The desktop's watcher** (`scripts/winvm/winvm-watch.sh`, cron every 10 minutes, installed by
   `scripts/winvm/winvm-cron-install.sh` as `~/.local/bin/bp-winvm-watch.sh`; log `~/.cache/bp-winvm/watch.log`).
   A VM that is stopped but not deallocated is deallocated at once. A running VM whose locks have been free for 30
   minutes is deallocated: bp-winbuild's locks are `winbuild-vm` and `bp-win-gate`; bp-win's are `bp-win-ui`, `winbuild` and `bp-win-gate` (`scripts/land.sh`'s Windows gate), and its
   watchdog is asked first (through Azure Run Command, not ssh) whether it saw a session or job in those 30 minutes;
   it is left running if so, without a time cap on active work. A stale, unreadable or dry-run watchdog also leaves bp-win running; its 07:00 UTC shutdown remains the backstop. While `bp-win-ui` or `winbuild` is held, the watcher keeps bp-win's watchdog hold at least 60 minutes ahead
   (Run Command; it never shortens a longer hold), since the VM can't see the desktop's locks. A lock is held when its
   `/tmp/bp-<name>.lock` has a flock (the hub's named locks and `scripts/bp-lock.sh` both take it).

To keep a VM up for hand work, hold its lock (and stay connected); the watchdog still counts your session.

Install bp-win's watchdog with all UI-test signals (add `-WhatIf` for a dry run):

```sh
scripts/winvm/winvm-push.sh bp-win 30 -JobDir 'C:\bp\sess' -RecentWriteDir 'C:\bp' -ExtraBusyProcesses 'backplane-hub,backplane-gpui,backplane,kicad,kicad-cli,pcbnew,eeschema,python,pythonw'
```

The regression checks are `bash test/winvm-watch.sh` (Azure and lock stand-ins) and, on Windows,
`powershell -NoProfile -File test/winvm-watchdog.ps1 -Python C:\Python313\python.exe`.
The Windows check uses isolated state and blocked shutdown calls. Process-name cases use synthetic
zero-CPU console processes; the optional Python check launches a real sleeping harness and child,
checks each PID independently, and stops only those captured PIDs. It also checks existing-file
writes, directory changes, transfers, session jobs, holds, and idle negative controls.
