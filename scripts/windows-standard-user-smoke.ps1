param([string]$PackageUrl='', [string]$PackagePath='', [string]$PackageSHA256='', [string]$ExpectedVersion='')
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$out = Join-Path $root 'build\windows-standard-user'
New-Item -ItemType Directory -Force $out | Out-Null
$exe = ''
if ($PackageUrl -or $PackagePath) {
    if (($PackageUrl -and $PackageUrl -notmatch '^https://') -or ($PackageUrl -and $PackagePath) -or $PackageSHA256 -notmatch '^[a-f0-9]{64}$' -or !$ExpectedVersion) { throw 'Supply one HTTPS URL or local ZIP, SHA256 and expected version' }
    $zip = Join-Path $out 'package.zip'
    if ($PackageUrl) { Invoke-WebRequest $PackageUrl -OutFile $zip }
    else { Copy-Item (Resolve-Path $PackagePath).Path $zip }
    if ((Get-FileHash $zip -Algorithm SHA256).Hash.ToLowerInvariant() -ne $PackageSHA256) { throw 'Package SHA256 mismatch' }
    $package = Join-Path $out 'package'
    Expand-Archive $zip -DestinationPath $package
    $found = @(Get-ChildItem $package -Recurse -File -Filter Backplane.exe)
    if ($found.Count -ne 1) { throw 'Expected exactly one packaged Backplane.exe' }
    $exe = $found[0].FullName
    @{url=$PackageUrl;sourcePath=$PackagePath;sha256=$PackageSHA256;version=$ExpectedVersion} | ConvertTo-Json | Set-Content (Join-Path $out 'package.json')
}
# This bootstrap runs on a disposable hosted runner. Only it is elevated;
# every assertion, native filesystem effect and Electron process runs below
# as a newly created user that has never belonged to Administrators.
$key = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\AppModelUnlock'
New-Item $key -Force | Out-Null
New-ItemProperty $key -Name AllowDevelopmentWithoutDevLicense -Value 0 -PropertyType DWord -Force | Out-Null
$name = 'bp_smoke_' + [Guid]::NewGuid().ToString('N').Substring(0,8)
$password = ConvertTo-SecureString ('Bp!9' + [Guid]::NewGuid().ToString('N')) -AsPlainText -Force
$user = New-LocalUser -Name $name -Password $password -AccountNeverExpires
$bootstrap = @{account=$name;sid=$user.SID.Value;timedOut=$false;cleanup=@()}
$bootstrapPath = Join-Path $out 'bootstrap.json'
$users = Get-LocalGroup -SID 'S-1-5-32-545'
Add-LocalGroupMember -Group $users -Member $name
try {
    # Read checkout/dependencies/package; write only the disposable smoke area.
    & icacls $root /grant "${name}:(OI)(CI)RX" /T /Q | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Cannot grant checkout read access' }
    & icacls $out /grant "${name}:(OI)(CI)M" /T /Q | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Cannot grant evidence write access' }
    $node = (Get-Command node.exe).Source
    $test = Join-Path $root 'desktop\test\windows-standard-user.cjs'
    $arguments = @('"'+$test+'"','"'+$out+'"')
    if ($exe) { $arguments += @('"'+$exe+'"','"'+$ExpectedVersion+'"') }
    $credential = [PSCredential]::new("$env:COMPUTERNAME\$name",$password)
    $p = Start-Process -FilePath $node -ArgumentList $arguments -Credential $credential -LoadUserProfile -WorkingDirectory $out -PassThru
    $bootstrap.nodePid = $p.Id
    if (!$p.WaitForExit(240000)) {
        $bootstrap.timedOut = $true
        try { $p.Kill() } catch { $bootstrap.timeoutCleanupError = $_.Exception.Message }
        throw 'Standard-user smoke exceeded four minutes; preserve result.json and bootstrap.json'
    }
    $bootstrap.exitCode = $p.ExitCode
    if ($p.ExitCode -ne 0) { throw "Standard-user smoke failed ($($p.ExitCode)); see result.json" }
    $result = Get-Content (Join-Path $out 'result.json') -Raw | ConvertFrom-Json
    if ($result.status -ne 'PASS') { throw 'Standard-user smoke did not pass' }
    $result | ConvertTo-Json -Depth 10
} finally {
    # Retire only this disposable account's processes, including its resident
    # hub. Never signal a process by name or a PID parsed from hub.lock.
    Get-CimInstance Win32_Process | ForEach-Object {
        $process = $_
        try {
            $owner = Invoke-CimMethod -InputObject $process -MethodName GetOwnerSid -ErrorAction Stop
            if ($owner.Sid -eq $user.SID.Value) {
                $cleanup = @{pid=$process.ProcessId;executable=$process.ExecutablePath;stopped=$false}
                try {
                    $owned = [Diagnostics.Process]::GetProcessById([int]$process.ProcessId)
                    $owned.Kill()
                    $cleanup.stopped = $owned.WaitForExit(5000)
                } catch {
                    $cleanup.error = $_.Exception.Message
                    $cleanup.stopped = !(Get-Process -Id $process.ProcessId -ErrorAction SilentlyContinue)
                }
                $bootstrap.cleanup += $cleanup
            }
        } catch {}
    }
    Remove-LocalUser -Name $name
    $bootstrap.accountRemoved = !(Get-LocalUser -Name $name -ErrorAction SilentlyContinue)
    $bootstrap | ConvertTo-Json -Depth 10 | Set-Content $bootstrapPath
    if (!$bootstrap.accountRemoved -or @($bootstrap.cleanup | Where-Object { !$_.stopped }).Count -ne 0) { throw 'Disposable account cleanup incomplete; see bootstrap.json' }
}
