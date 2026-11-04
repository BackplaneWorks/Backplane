# Disposable hosted runner only. All provider assertions run as a newly
# created non-administrator; no user machine, model key or model request.
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$out = Join-Path $root 'build\windows-omp'
New-Item -ItemType Directory -Force (Join-Path $out 'bin') | Out-Null
$bun = (Get-Command bun.exe).Source
Copy-Item $bun (Join-Path $out 'bin\bun.exe')
$version = '18.16.0'
$url = "https://github.com/can1357/oh-my-pi/releases/download/v$version"
Invoke-WebRequest "$url/omp-windows-x64.exe" -OutFile (Join-Path $out 'bin\omp.exe')
# Pinned from v18.16.0/SHA256SUMS.txt. Invoke-WebRequest may expose text
# assets as byte[]; do not depend on its Content response-type inference.
$expected = 'c249a9ed0a57c748fb1247d8ee9e2d0fe1b13229266ce74d8810a76a591e21fc'
$actual = (Get-FileHash (Join-Path $out 'bin\omp.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actual -ne $expected) { throw 'OMP executable checksum mismatch' }
function Paging {
    @{ usage=@(Get-CimInstance Win32_PageFileUsage | Select-Object Name,AllocatedBaseSize,CurrentUsage,PeakUsage);
       settings=@(Get-CimInstance Win32_PageFileSetting | Select-Object Name,InitialSize,MaximumSize);
       memory=Get-CimInstance Win32_OperatingSystem | Select-Object TotalVisibleMemorySize,FreePhysicalMemory,TotalVirtualMemorySize,FreeVirtualMemory }
}
$bootstrap = @{sourceCommit=$env:GITHUB_SHA;runId=$env:GITHUB_RUN_ID;providerVersion=$version;providerSHA256=$actual;pagingBefore=Paging;pagefileChanged=$false;cleanup=@()}
$bootstrapPath = Join-Path $out 'bootstrap.json'
$key = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\AppModelUnlock'
New-Item $key -Force | Out-Null
New-ItemProperty $key -Name AllowDevelopmentWithoutDevLicense -Value 0 -PropertyType DWord -Force | Out-Null
$name = 'bp_omp_' + [Guid]::NewGuid().ToString('N').Substring(0,8)
$password = ConvertTo-SecureString ('Bp!9' + [Guid]::NewGuid().ToString('N')) -AsPlainText -Force
$user = New-LocalUser -Name $name -Password $password -AccountNeverExpires
$bootstrap.account=$name
$users = Get-LocalGroup -SID 'S-1-5-32-545'
Add-LocalGroupMember -Group $users -Member $name
try {
    & icacls $root /grant "${name}:(OI)(CI)RX" /T /Q | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Cannot grant checkout read access' }
    & icacls $out /grant "${name}:(OI)(CI)M" /T /Q | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Cannot grant evidence write access' }
    $credential = [PSCredential]::new("$env:COMPUTERNAME\$name",$password)
    $test = Join-Path $root 'test\tools\omp_windows_native_test.mjs'
    $p = Start-Process -FilePath (Join-Path $out 'bin\bun.exe') -ArgumentList @('"'+$test+'"','"'+$out+'"') -Credential $credential -LoadUserProfile -WorkingDirectory $root -PassThru
    $bootstrap.bunPid=$p.Id
    if (!$p.WaitForExit(240000)) { $p.Kill(); throw 'OMP standard-user test exceeded four minutes' }
    $bootstrap.exitCode=$p.ExitCode
    $result=Get-Content (Join-Path $out 'result.json') -Raw | ConvertFrom-Json
    if ($p.ExitCode -ne 0 -or $result.status -ne 'PASS') { $result | ConvertTo-Json -Depth 12; throw 'OMP standard-user test failed; see result.json' }
    $result | ConvertTo-Json -Depth 12
} finally {
    Get-CimInstance Win32_Process | ForEach-Object {
        $process=$_
        try {
            $owner=Invoke-CimMethod -InputObject $process -MethodName GetOwnerSid -ErrorAction Stop
            if ($owner.Sid -eq $user.SID.Value) {
                $owned=[Diagnostics.Process]::GetProcessById([int]$process.ProcessId)
                $owned.Kill()
                $bootstrap.cleanup += @{pid=$process.ProcessId;stopped=$owned.WaitForExit(5000)}
            }
        } catch {}
    }
    Remove-LocalUser -Name $name
    $bootstrap.accountRemoved=!(Get-LocalUser -Name $name -ErrorAction SilentlyContinue)
    $bootstrap.pagingAfter=Paging
    $bootstrap | ConvertTo-Json -Depth 12 | Set-Content $bootstrapPath
    if (!$bootstrap.accountRemoved) { throw 'Disposable account cleanup incomplete' }
}
