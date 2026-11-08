# pair-hub.ps1 -Port N [-Name RUN] [-Build DIR]: a hub on a temp home that the tailnet can reach (the machine's tailscale IPv4, --no-tailscale),
# for the pairing rows: prints its URL and token path. Run in the desktop session (pair-hub.sh); stop it with down.ps1 -Name RUN.
param([int]$Port = 3890, [string]$Name = "pair-$Port", [string]$Build = 'C:\bp\build\cur')
$ts = 'C:\Program Files\Tailscale\tailscale.exe'
$ip = (& $ts ip -4 2>$null | Select -First 1).Trim()
if (-not $ip) { 'no tailscale IPv4 on this machine'; exit 1 }
if (-not (Get-NetFirewallRule -DisplayName 'Backplane test hubs' -EA SilentlyContinue)) {
  New-NetFirewallRule -DisplayName 'Backplane test hubs' -Direction Inbound -Protocol TCP -LocalPort 3800-3899 -Action Allow -Profile Any | Out-Null; 'firewall: inbound 3800-3899 allowed' }
& C:\bp\rig\down.ps1 -Name $Name | Out-Null
& C:\bp\rig\up.ps1 -Build $Build -Port $Port -Name $Name -Fresh -NoClient -HubArgs "--host $ip" -At $ip
"URL http://${ip}:$Port"
"TOKEN C:\bp\runs\$Name\home\token"
