param([Parameter(Mandatory=$true)][string]$ConfigPath)
$ErrorActionPreference = 'Stop'
$cfg = Get-Content -LiteralPath $ConfigPath -Raw | ConvertFrom-Json
$code = 1
try {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    if ($identity.User.Value -ne $cfg.standardUserSid -or $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Wrong standard-user identity' }
    $profile = Get-ItemPropertyValue ('HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList\'+$identity.User.Value) ProfileImagePath
    $env:USERPROFILE = $profile; $env:HOME = $profile
    $env:APPDATA = Join-Path $profile 'AppData\Roaming'
    $env:LOCALAPPDATA = Join-Path $profile 'AppData\Local'
    $env:TEMP = Join-Path $cfg.out 'temp'; $env:TMP = $env:TEMP
    New-Item -ItemType Directory -Force $env:TEMP | Out-Null
    . (Join-Path $PSScriptRoot 'job.ps1')
    # Bootstrap explicitly admits this logon-created process to its capped job.
    # A query-only handle identifies that exact ancestor even if PowerShell
    # creates an inner job. Never infer the cap from a caller-written receipt.
    $admissionPath=Join-Path $cfg.out 'job-admission.json'
    $deadline=[DateTime]::UtcNow.AddSeconds(30)
    while (!(Test-Path -LiteralPath $admissionPath)) {
        if ([DateTime]::UtcNow -gt $deadline) { throw 'Missing capped-job handle admission' }
        Start-Sleep -Milliseconds 100
    }
    $admission=Get-Content -LiteralPath $admissionPath -Raw | ConvertFrom-Json
    if ($admission.pid -ne $PID -or $admission.sid -ne $identity.User.Value) { throw 'Job query handle belongs to another process' }
    [BP927Job]::UseQueryHandle([long]$admission.queryHandle)
    # Reread actual kernel limits and membership through that handle.
    $resources = Get-BP927Resources
    $resources | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 (Join-Path $cfg.out 'resources-start.json')
    Assert-BP927Resources $resources $cfg.resourcePolicy.allowExistingPagefile
    $env:PYTHONDONTWRITEBYTECODE = '1'
    & $cfg.python (Join-Path $PSScriptRoot 'canonical-946.py') $ConfigPath *> (Join-Path $cfg.out 'canonical-946.log')
    $abiCode = $LASTEXITCODE
    & $cfg.node (Join-Path $PSScriptRoot 'accept.cjs') $ConfigPath *> (Join-Path $cfg.out 'consumer.log')
    $code = $LASTEXITCODE
    if ($abiCode -ne 0 -and $code -eq 0) { $code = $abiCode }
} catch {
    $_ | Out-String | Set-Content -Encoding utf8 (Join-Path $cfg.out 'user-error.txt')
} finally {
    try {
        $resources = Get-BP927Resources
        $resources | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 (Join-Path $cfg.out 'resources-end.json')
        Assert-BP927Resources $resources $cfg.resourcePolicy.allowExistingPagefile
    } catch { $code = 1; $_ | Out-String | Add-Content (Join-Path $cfg.out 'user-error.txt') }
}
exit $code
