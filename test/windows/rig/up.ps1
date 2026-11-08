# up.ps1 -Build DIR -Port N -Name RUN [-Fresh] [-Agents DIR] [-NoClient]: a hub on its own home C:\bp\runs\RUN\home (--no-tailscale), and the client
# connected to it with its own settings folder, both started detached in this session; their pids in C:\bp\runs\RUN\pids. Run it through sess.sh.
param([string]$Build = 'C:\bp\build\cur', [int]$Port = 3799, [string]$Name = 'r1', [switch]$Fresh, [string]$Agents = '', [switch]$NoClient, [string]$HubArgs = '', [string]$At = '127.0.0.1')
$ErrorActionPreference = 'Stop'
$w = "C:\bp\runs\$Name"; if ($Fresh) { Remove-Item -Recurse -Force $w -EA SilentlyContinue }
New-Item -ItemType Directory -Force "$w\home", "$w\config" | Out-Null
if ($Agents) { $env:BACKPLANE_AGENTS = $Agents }
$ha = @('--home', "$w\home", '--port', "$Port", '--no-tailscale', '--foreground') + ($HubArgs -split ' ' | ? { $_ })
$hub = Start-Process "$Build\backplane-hub.exe" -ArgumentList $ha -RedirectStandardOutput "$w\hub.log" -RedirectStandardError "$w\hub.err" -WindowStyle Hidden -PassThru
$ok = $false
for ($i = 0; $i -lt 120; $i++) { try { Invoke-WebRequest -UseBasicParsing "http://${At}:$Port/hello" -TimeoutSec 2 | Out-Null; $ok = $true; break } catch { if ($hub.HasExited) { break }; Start-Sleep -Milliseconds 500 } }
if (-not $ok) { "hub not answering (exited: $($hub.HasExited))"; Get-Content "$w\hub.log", "$w\hub.err" -Tail 30 -EA SilentlyContinue; exit 1 }
$hello = Invoke-WebRequest -UseBasicParsing "http://${At}:$Port/hello"; $b = $hello.RawContentStream.ToArray()
"hub $($hub.Id) on ${Port}: /hello $($hello.StatusCode), $($b.Length) bytes"
$pids = @("hub=$($hub.Id)")
if (-not $NoClient) {
  $env:XDG_CONFIG_HOME = "$w\config"
  $c = Start-Process "$Build\backplane-gpui.exe" -ArgumentList '--url', "ws://127.0.0.1:$Port", '--token-file', "$w\home\token" -RedirectStandardOutput "$w\client.log" -RedirectStandardError "$w\client.err" -PassThru
  "client $($c.Id)"; $pids += "client=$($c.Id)"
}
$pids | Set-Content "$w\pids"
