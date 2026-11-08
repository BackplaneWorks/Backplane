# Runs in the autologon user's interactive desktop session (scheduled task, at logon, "only when the user is logged on").
# Every C:\bp\sess\in\<id>.ps1 is run here; its output goes to C:\bp\sess\out\<id>.log and then <id>.done appears (the exit code inside).
$ErrorActionPreference = 'Continue'
$in = 'C:\bp\sess\in'; $out = 'C:\bp\sess\out'
New-Item -ItemType Directory -Force $in, $out | Out-Null
"$(Get-Date -Format s) agent up, session $((Get-Process -Id $PID).SessionId)" | Out-File -Append C:\bp\sess\agent.log
while ($true) {
  foreach ($f in Get-ChildItem $in -Filter *.ps1 | Sort-Object LastWriteTime) {
    $id = $f.BaseName; $job = "$out\$id.ps1"; Move-Item -Force $f.FullName $job
    # every stream of the job, as UTF-8 text (a plain redirect writes PowerShell's UTF-16)
    $cmd = "& '$job' *>&1 | Out-File -Encoding utf8 -Width 400 '$out\$id.log'; exit `$LASTEXITCODE"
    $p = Start-Process powershell -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-Command',$cmd -WindowStyle Hidden -PassThru
    # a job may start long-lived programs and return; the agent waits only for the job script itself
    $p.WaitForExit(); "$($p.ExitCode)" | Out-File -Encoding ascii "$out\$id.done"
  }
  Start-Sleep -Milliseconds 300
}
