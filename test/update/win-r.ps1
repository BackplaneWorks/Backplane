# win-r.ps1 STEP: the R rows on a Windows machine, on real release zips (C:\bp\feed\<tag>\: the zip and its manifest.json, .sig, .keyid),
# run in the desktop session (test/windows/rig/sess.sh). The feed is a GitHub-style releases.json on loopback; the guard and hub read it
# through BACKPLANE_UPDATE_FEED, which the client passes on.
#   install TAG   remove any install, start the feed (empty), install TAG from its zip as a person does (SmartScreen, Install, Use this folder)
#   list TAGS...  the feed lists TAGS; the hub checks and updates now; the guard's state after
#   state         the guard's state, BAD and /hello
param([string]$Step = 'state', [string[]]$Tags = @())
$ErrorActionPreference = 'Continue'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$P = "$env:LOCALAPPDATA\Backplane"; $H = "$env:APPDATA\Backplane\hub"; $F = 'C:\bp\feed'; $FP = 8899; $o = 'C:\bp\runs\r'; New-Item -ItemType Directory -Force $o, $F | Out-Null
$env:Path = "C:\Program Files\Git\cmd;C:\Python313;$env:USERPROFILE\.bun\bin;$env:Path"
$env:BACKPLANE_UPDATE_FEED = "http://127.0.0.1:$FP/releases.json"
function Shot($n) { & C:\bp\rig\shot.ps1 "$o\$n.png" | Out-Null; "shot $n" }
function Click($name) { $root = [Windows.Automation.AutomationElement]::RootElement; $c = New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::NameProperty, $name); $b = $root.FindFirst([Windows.Automation.TreeScope]::Descendants, $c); if ($b) { try { ($b.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern)).Invoke(); "clicked $name"; return $true } catch { "invoke ${name}: $_" } } else { "no '$name'" }; return $false }
function Publish([string[]]$ts) { & C:\Python313\python.exe -I -c @"
import json, os, sys
root, port, tags = sys.argv[1], sys.argv[2], sys.argv[3:]
print(json.dumps([{'tag_name': t, 'prerelease': False, 'draft': False, 'assets': [{'name': n, 'browser_download_url': f'http://127.0.0.1:{port}/{t}/{n}'} for n in sorted(os.listdir(os.path.join(root, t)))]} for t in tags]))
"@ $F $FP @ts | Set-Content -Encoding ascii "$F\releases.json"; "feed lists: $($ts -join ', ')" }
function Port { $s = Get-Content "$H\guard\status.json" -Raw -EA SilentlyContinue | ConvertFrom-Json; if ($s.port) { $s.port } else { 3800 } }
function Call($m) { & bun C:\bp\repo\test\update\win-r.mjs (Port) "$H\token" $m 2>&1 }
function State {
  $s = Get-Content "$H\guard\status.json" -Raw -EA SilentlyContinue
  "status.json: $s"
  "CURRENT: $(Get-Content $P\CURRENT -EA SilentlyContinue)"; "BAD: $((Get-Content $P\BAD -EA SilentlyContinue) -join ' | ')"; "builds: $((Get-ChildItem $P\builds -EA SilentlyContinue | % Name) -join ',')"
  "update.status: $(Call update.status)"
}
switch ($Step) {
  'install' {
    $tag = $Tags[0]
    if (Test-Path "$P\backplane.exe") { & "$P\backplane.exe" hub stop --home $H 2>&1 | Select -Last 1 }
    # only the clients this install started (their program is under $P), never one by its name alone
    Get-CimInstance Win32_Process -Filter "Name='backplane-gpui.exe'" | ? { $_.ExecutablePath -and $_.ExecutablePath.StartsWith("$P\", 'OrdinalIgnoreCase') } | % { "stop client $($_.ProcessId) ($($_.ExecutablePath))"; Stop-Process -Id $_.ProcessId -Force }
    Start-Sleep 3
    # the install only: Backplane KiCad (kicad\) shares the prefix and stays
    Remove-Item -Recurse -Force "$P\builds", "$P\CURRENT", "$P\BAD", "$P\backplane.exe", "$P\backplane.exe.old", "$env:APPDATA\Backplane", "$env:APPDATA\backplane-gpui", "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\Backplane.lnk" -EA SilentlyContinue
    if (-not (Get-NetTCPConnection -LocalPort $FP -State Listen -EA SilentlyContinue)) { Start-Process C:\Python313\python.exe -ArgumentList '-I', '-m', 'http.server', "$FP", '--bind', '127.0.0.1', '--directory', $F -WindowStyle Hidden -RedirectStandardError C:\bp\runs\feed.log; "feed server up" }
    Publish @()
    $zip = Get-ChildItem "$F\$tag\*windows-x64-native.zip" | Select -First 1
    $dl = "$env:USERPROFILE\Downloads"; $z = "$dl\$($zip.Name)"; Copy-Item $zip.FullName $z -Force
    Set-Content -Path $z -Stream Zone.Identifier -Value "[ZoneTransfer]`r`nZoneId=3`r`nHostUrl=https://github.com/"
    $x = "$dl\backplane-$tag"; Remove-Item -Recurse -Force $x -EA SilentlyContinue; New-Item -ItemType Directory $x | Out-Null
    $sh = New-Object -ComObject Shell.Application; $sh.NameSpace($x).CopyHere($sh.NameSpace($z).Items(), 0x14)
    for ($i = 0; $i -lt 60 -and -not (Test-Path "$x\backplane-gpui.exe"); $i++) { Start-Sleep 1 }; Start-Sleep 3
    "unpacked: $((Get-ChildItem $x | % Name) -join ' ')"
    # a double-click: ShellExecute, in a process of its own (SmartScreen holds the caller until it is answered)
    Start-Process powershell -ArgumentList '-NoProfile', '-Command', "(New-Object -ComObject Shell.Application).ShellExecute('$x\backplane-gpui.exe')" -WindowStyle Hidden
    Start-Sleep 8; Shot 'i1-smartscreen'
    if (Click 'More info') { Start-Sleep 1; Shot 'i2-moreinfo'; Click 'Run anyway' | Out-Null }
    Start-Sleep 8; Shot 'i3-ask'; Click 'Install'; Start-Sleep 15; Shot 'i4-first'
    & C:\Python313\python.exe C:\bp\rig\input.py click 707 632; Start-Sleep 25; Shot 'i5-home'
    State
  }
  'list' { Publish $Tags; "check: $(Call update.check)"; "update: $(Call app.update)"
    $want = $Tags[-1]
    for ($i = 0; $i -lt 60; $i++) { Start-Sleep 5; $b = Get-Content $P\BAD -EA SilentlyContinue; $s = Get-Content "$H\guard\status.json" -Raw -EA SilentlyContinue
      if (($b -match [regex]::Escape($want)) -or ($s -match "`"build`":`"$([regex]::Escape($want))`"" -and $s -match '"stable":true')) { break } }
    Start-Sleep 10; State; Shot "l-$want"
  }
  'state' { State; Shot 'state' }
}
