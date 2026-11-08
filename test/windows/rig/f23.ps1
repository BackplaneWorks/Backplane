# f23.ps1 (run in the session via sess.sh; hub build in C:\bp\build\cur, repo in C:\bp\repo): F23 on Windows, a hub killed hard (taskkill /F) while a stand-in turn runs leaves no agent behind; prints F23 PASS or FAIL
$ErrorActionPreference = 'Continue'
$env:Path = "C:\Program Files\Git\cmd;C:\Program Files\Git\usr\bin;C:\Python313;$env:USERPROFILE\.bun\bin;$env:Path"
$w = 'C:\bp\runs\f23'; Remove-Item -Recurse -Force $w -EA SilentlyContinue; New-Item -ItemType Directory -Force "$w\home", "$w\bin", "$w\proj" | Out-Null
Copy-Item C:\bp\repo\test\e2e\lib\standin_rows.py "$w\bin\claude"
foreach ($n in 'codex','grok','muse','opencode') { [IO.File]::WriteAllText("$w\bin\$n", "#!/bin/sh`nexit 1`n") }
$now = [int][double]::Parse((Get-Date -UFormat %s))
@("{`"`$`":`"ProjectCreated`",`"id`":`"p1`",`"title`":`"f23`",`"root`":`"C:/bp/runs/f23/proj`",`"at`":$now}", "{`"`$`":`"ThreadCreated`",`"id`":`"t1`",`"project`":`"p1`",`"title`":`"T`",`"env`":`"local`",`"branch`":`"`",`"worktree`":`"`",`"provider`":`"claude`",`"model`":`"`",`"at`":$now}") | Set-Content -Encoding ascii "$w\home\events.jsonl"
$env:BACKPLANE_AGENTS = "$w\bin"; $env:FAKE_LOG = "$w\fake.log"; $env:FAKE_SECS = '600'; $env:BACKPLANE_NO_UPDATE = '1'
$hub = Start-Process C:\bp\build\cur\backplane-hub.exe -ArgumentList '--home', "$w\home", '--port', '3897', '--no-tailscale', '--foreground' -RedirectStandardOutput "$w\hub.log" -RedirectStandardError "$w\hub.err" -WindowStyle Hidden -PassThru
for ($i = 0; $i -lt 60; $i++) { try { Invoke-WebRequest -UseBasicParsing http://127.0.0.1:3897/hello -TimeoutSec 2 | Out-Null; break } catch { Start-Sleep -Milliseconds 500 } }
"turn: $(& bun C:\bp\repo\test\update\send-turn.mjs 3897 "$w\home\token" t1 'a long turn' 2>&1)"
function Tree($root) { $all = Get-CimInstance Win32_Process; $ids = @($root); $grew = $true; while ($grew) { $grew = $false; foreach ($p in $all) { if ($ids -contains $p.ParentProcessId -and -not ($ids -contains $p.ProcessId)) { $ids += $p.ProcessId; $grew = $true } } }; $all | ? { $ids -contains $_.ProcessId -and $_.ProcessId -ne $root } }
$kids = @(); for ($i = 0; $i -lt 40; $i++) { Start-Sleep -Milliseconds 500; $kids = @(Tree $hub.Id | ? { $_.Name -match 'python|sh|bash|bp-sh' }); if ($kids.Count -gt 0 -and (Test-Path "$w\fake.log") -and (Select-String -Path "$w\fake.log" -Pattern 'TURN' -Quiet)) { break } }
"agent processes under the hub before: $(($kids | % { "$($_.ProcessId) $($_.Name)" }) -join ', ')"
Start-Sleep 2
& taskkill.exe /F /PID $hub.Id | Out-Null; "hub $($hub.Id) killed with taskkill /F"
Start-Sleep 5
$left = @($kids | ? { Get-Process -Id $_.ProcessId -EA SilentlyContinue })
"left after 5 s: $(($left | % { "$($_.ProcessId) $($_.Name)" }) -join ', ')"
if ($kids.Count -gt 0 -and $left.Count -eq 0) { 'F23 PASS' } elseif ($kids.Count -eq 0) { 'F23 NO AGENT SEEN' } else { 'F23 FAIL'; $left | % { Stop-Process -Id $_.ProcessId -Force -EA SilentlyContinue } }
