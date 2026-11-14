# warm.ps1 -Runtime DIR [-Client DIR]: load a newly staged KiCad runtime (and client) once before its first measured run.
# Real-time scanning (Microsoft Defender) checks new files with the cloud on first use, which can hold a cold KiCad start past
# the hub's 30 s window; a first load here takes that wait out of the measured runs. Runs in ssh's session 0: no window shows.
param([Parameter(Mandatory)][string]$Runtime, [string]$Client = '')
$ErrorActionPreference = 'Continue'
$t0 = Get-Date
& (Join-Path $Runtime 'bin\kicad-cli.exe') version | Out-Null
$home_ = Join-Path $env:TEMP ("warm-" + [guid]::NewGuid().ToString('N').Substring(0, 8))
New-Item -ItemType Directory $home_ | Out-Null
$env:KICAD_CONFIG_HOME = $home_
$k = Start-Process -PassThru -WindowStyle Hidden (Join-Path $Runtime 'bin\kicad.exe')
Start-Sleep 20
if ($k -and -not $k.HasExited) { Stop-Process -Id $k.Id -Force -EA SilentlyContinue }
if ($Client) { foreach ($exe in 'backplane-hub.exe', 'backplane-gpui.exe') { $p = Join-Path $Client $exe; if (Test-Path $p) { & $p --version 2>&1 | Out-Null } } }
Remove-Item $home_ -Recurse -Force -EA SilentlyContinue
'warm: {0} loaded in {1:N0} s' -f $Runtime, ((Get-Date) - $t0).TotalSeconds
