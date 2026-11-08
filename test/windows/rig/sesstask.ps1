$ErrorActionPreference = 'Stop'
$a = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument '-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File C:\bp\sessagent.ps1'
$t = New-ScheduledTaskTrigger -AtLogOn -User $env:USERNAME
$p = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType Interactive -RunLevel Highest
$s = New-ScheduledTaskSettingsSet -ExecutionTimeLimit ([TimeSpan]::Zero) -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -MultipleInstances IgnoreNew
Register-ScheduledTask -Force -TaskName 'bp-sess' -Action $a -Trigger $t -Principal $p -Settings $s | Out-Null
'task bp-sess registered'
