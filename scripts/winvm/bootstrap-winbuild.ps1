# SPDX-License-Identifier: GPL-3.0-or-later
# First boot of bp-winbuild, run as SYSTEM through Azure Run Command (the VM has no public IP):
# the data disk as D: (label bpdata), OpenSSH server with the desktop's key for bpadmin, PowerShell 7.
# Tailscale is joined separately (winvm-tailscale.sh) so its key never lands in a script or log.
param([Parameter(Mandatory = $true)][string]$AuthorizedKey)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$tmp = Join-Path $env:TEMP 'bp-bootstrap'; New-Item -ItemType Directory -Force $tmp | Out-Null

# The data disk (the only disk with no partition yet) as D:, the DVD drive moved out of its way to Z:
Get-CimInstance Win32_Volume -Filter "DriveLetter='D:' AND DriveType=5" | Set-CimInstance -Property @{ DriveLetter = 'Z:' }
foreach ($d in Get-Disk | Where-Object { !$_.IsBoot -and !$_.IsSystem -and $_.NumberOfPartitions -le 1 -and !($_ | Get-Partition -ErrorAction SilentlyContinue | Where-Object DriveLetter) }) {
    if ($d.PartitionStyle -eq 'RAW') { Initialize-Disk -Number $d.Number -PartitionStyle GPT }
    New-Partition -DiskNumber $d.Number -UseMaximumSize -DriveLetter D |
        Format-Volume -FileSystem NTFS -NewFileSystemLabel bpdata -AllocationUnitSize 65536 -Confirm:$false | Out-Null
}
Get-Volume | Where-Object DriveLetter | Select-Object DriveLetter, FileSystemLabel, Size, SizeRemaining | Format-Table | Out-String

# Defender scanning every object file costs a build dearly: leave the build drive and the tools out
Add-MpPreference -ExclusionPath 'D:\', 'C:\src' -ExclusionProcess 'cl.exe', 'link.exe', 'ninja.exe', 'ccache.exe', 'cmake.exe', 'vcpkg.exe'

# OpenSSH server (Win32-OpenSSH release; the Windows capability needs Windows Update and is older)
$ssh = 'https://github.com/PowerShell/Win32-OpenSSH/releases/download/10.0.0.0p2-Preview/OpenSSH-Win64-v10.0.0.0.msi'
Invoke-WebRequest $ssh -OutFile "$tmp\openssh.msi" -UseBasicParsing
Start-Process msiexec.exe -Wait -ArgumentList "/i `"$tmp\openssh.msi`" /qn /norestart"
$pwsh = 'https://github.com/PowerShell/PowerShell/releases/download/v7.6.10/PowerShell-7.6.10-win-x64.msi'
Invoke-WebRequest $pwsh -OutFile "$tmp\pwsh.msi" -UseBasicParsing
Start-Process msiexec.exe -Wait -ArgumentList "/i `"$tmp\pwsh.msi`" /qn /norestart ADD_PATH=1 ENABLE_PSREMOTING=0"
$keys = Join-Path $env:ProgramData 'ssh\administrators_authorized_keys'
New-Item -ItemType Directory -Force (Split-Path $keys) | Out-Null
Set-Content -LiteralPath $keys -Value $AuthorizedKey -Encoding ascii
icacls $keys /inheritance:r /grant 'Administrators:F' /grant 'SYSTEM:F' | Out-Null
New-ItemProperty -Path 'HKLM:\SOFTWARE\OpenSSH' -Name DefaultShell -Value 'C:\Program Files\PowerShell\7\pwsh.exe' -PropertyType String -Force | Out-Null
Set-Service sshd -StartupType Automatic; Restart-Service sshd
if (!(Get-NetFirewallRule -Name 'bp-sshd' -ErrorAction SilentlyContinue)) {
    New-NetFirewallRule -Name 'bp-sshd' -DisplayName 'OpenSSH (Backplane)' -Direction Inbound -Protocol TCP -LocalPort 22 -Action Allow | Out-Null
}
# Long paths for KiCad's deep build tree
Set-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem' -Name LongPathsEnabled -Value 1
# Tailscale (joined by winvm-tailscale.sh)
Invoke-WebRequest 'https://pkgs.tailscale.com/stable/tailscale-setup-latest-amd64.msi' -OutFile "$tmp\tailscale.msi" -UseBasicParsing
Start-Process msiexec.exe -Wait -ArgumentList "/i `"$tmp\tailscale.msi`" /qn /norestart TS_UNATTENDEDMODE=always"
'sshd: ' + (Get-Service sshd).Status
& 'C:\Program Files\PowerShell\7\pwsh.exe' -NoProfile -Command '$PSVersionTable.PSVersion.ToString()'
Test-Path 'C:\Program Files\Tailscale\tailscale.exe'
