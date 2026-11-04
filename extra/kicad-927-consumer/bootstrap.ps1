param([Parameter(Mandatory=$true)][string]$ArtifactDirectory,[switch]$InstallOnly)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_OS -ne 'Windows') { throw 'GitHub Windows runner only' }
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$out = Join-Path $root 'build\kicad-927-native'
New-Item -ItemType Directory -Force $out | Out-Null
$report = @{status='INCOMPLETE';phase='resource-gate';cleanup=@();published=$false;releaseBuildApproved=$false}
# Explicit person authorization: retain the existing hosted Windows pagefile
# for this #927/#946 test, keep the same cap, and measure paging throughout.
$resourcePolicy = @{maxJobMemoryBytes=8589934592;allowExistingPagefile=$true;pagefileModificationAllowed=$false;authorization='User option 1: allow existing GitHub Windows pagefile for #927 test; retain cap and record paging';scope='This #927 consumer and separate #946 native measurement only'}
$report.resourcePolicy=$resourcePolicy
$account = $null
function Save-Report { $report | ConvertTo-Json -Depth 12 | Set-Content -Encoding utf8 (Join-Path $out 'bootstrap.json') }
function SHA($file) { (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant() }
function Require-Equal($actual,$expected,$label) { if ($actual -cne $expected) { throw "Mismatch: $label" } }
try {
    . (Join-Path $PSScriptRoot 'job.ps1')
    [BP927Job]::CapSelf()
    $report.resourcesStart = Get-BP927Resources; Save-Report
    Assert-BP927Resources $report.resourcesStart $resourcePolicy.allowExistingPagefile
    $report.phase = 'authenticated-artifact-verification'; Save-Report
    $zip = Join-Path $ArtifactDirectory 'dist\backplane-v0.14.2-windows-x64-TEST-ONLY.zip'
    $receipt = Join-Path $ArtifactDirectory 'dist\TEST-PACKAGE.json'
    Require-Equal (SHA $zip) 'de613310949b72b0a70bebf9a07aff021cdea98f894856fabdc42134d678cd22' 'Windows ZIP SHA256'
    Require-Equal (SHA $receipt) '01ef6ce34346fc581219ad7a96dd724c089b9263e07b42bc6ddce9266966db97' 'package receipt SHA256'
    $report.artifact = @{runId=41886983230;artifactId=13352107148;name='TEST-ONLY-package-windows-x64';zipSHA256=(SHA $zip);receiptSHA256=(SHA $receipt)}
    Copy-Item -LiteralPath $receipt -Destination (Join-Path $out 'TEST-PACKAGE.json')
    $package = Join-Path $out 'package'
    Expand-Archive -LiteralPath $zip -DestinationPath $package
    $exe = Join-Path $package 'backplane-windows-x64\Backplane.exe'
    $build = Get-Content -LiteralPath (Join-Path (Split-Path $exe) 'resources\DESKTOP-BUILD.json') -Raw | ConvertFrom-Json
    Require-Equal $build.sourceCommit 'afa9679273206b3e9b1da4c966c9b63fb6f57ab4' 'source commit'
    Require-Equal $build.sourceTree 'ec6c85b4a8b62a0227f09151a9ec52a362b20ae2' 'source tree'
    Require-Equal $build.serviceVersion '0.14.2' 'version'
    Require-Equal $build.buildProvenance.compilerMode 'composite-settle-41262875' 'compiler mode'
    Require-Equal $build.buildProvenance.producerWorkId 'w-t1792701529-415584-1614985' 'raw producer'
    if (!$build.testOnly -or $build.releaseQualified -or $build.buildProvenance.releaseBuildApproved) { throw 'Test-only provenance changed' }
    Copy-Item -LiteralPath (Join-Path (Split-Path $exe) 'resources\DESKTOP-BUILD.json') -Destination (Join-Path $out 'DESKTOP-BUILD.json')
    $report.phase = 'vendor-runtime-fixtures'; Save-Report
    $pin = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'runtime-pin.json') -Raw | ConvertFrom-Json
    $vendorZip = Join-Path $out $pin.asset
    # The application ZIP uses Actions transport only. This separate public
    # runtime URL is the actual frozen in-app catalog, also exercised by the app.
    Invoke-WebRequest -Uri $pin.url -OutFile $vendorZip
    Require-Equal (SHA $vendorZip) $pin.zipSHA256 'vendor runtime ZIP'
    $vendor = Join-Path $out 'vendor-runtime'
    Expand-Archive -LiteralPath $vendorZip -DestinationPath $vendor
    $runtime = Join-Path $vendor $pin.archiveRoot
    $manifest = Get-Content -LiteralPath (Join-Path $runtime 'manifest.json') -Raw | ConvertFrom-Json
    Require-Equal $manifest.version $pin.version 'vendor manifest version'
    Require-Equal $manifest.platform $pin.platform 'vendor manifest platform'
    $cliSHA = SHA (Join-Path $runtime $pin.cli)
    $report.runtime = @{zipSHA256=(SHA $vendorZip);runtimeCliSHA256=$cliSHA;manifest=$manifest}
    $programFilesRoot = Join-Path $env:ProgramFiles ('KiCad\'+$pin.version)
    $pathRoot = Join-Path $out 'PATH runtime'
    $manualRoots = @('C:\backplane_kicad\','C:\bp927\new tools\test\')
    $destinations = @($programFilesRoot,$pathRoot)+$manualRoots
    if ($InstallOnly) {
        if (Test-Path -LiteralPath (Join-Path $env:ProgramFiles 'KiCad')) { throw 'Install-only runner must have no Program Files KiCad' }
        $destinations = @(); $manualRoots = @()
    }
    foreach ($destination in $destinations) {
        if (Test-Path -LiteralPath $destination) { throw "Fixture destination already exists: $destination" }
        New-Item -ItemType Directory -Force $destination | Out-Null
        Copy-Item -Path (Join-Path $runtime '*') -Destination $destination -Recurse
    }
    $report.phase = 'consumer-dependencies'; Save-Report
    & npm.cmd --prefix (Join-Path $root 'desktop') ci --ignore-scripts --no-audit --no-fund *> (Join-Path $out 'npm.log')
    if ($LASTEXITCODE -ne 0) { throw 'Pinned Playwright dependency setup failed' }
    # The probe runtime is a tool, not an app overlay. Copy its exact bytes
    # out of the administrator's private profile for the standard account.
    $probeDir=Join-Path $out 'diagnostic-tools'
    New-Item -ItemType Directory -Force $probeDir | Out-Null
    $probeBun=Join-Path $probeDir 'bun.exe'
    Copy-Item -LiteralPath (Get-Command bun.exe).Source -Destination $probeBun
    $report.probeBunSHA256=SHA $probeBun
    $key = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\AppModelUnlock'
    New-Item $key -Force | Out-Null
    New-ItemProperty $key -Name AllowDevelopmentWithoutDevLicense -Value 0 -PropertyType DWord -Force | Out-Null
    $name = 'bp927_' + [Guid]::NewGuid().ToString('N').Substring(0,8)
    $password = ConvertTo-SecureString ('Bp!9'+[Guid]::NewGuid().ToString('N')) -AsPlainText -Force
    $account = New-LocalUser -Name $name -Password $password -AccountNeverExpires
    Add-LocalGroupMember -Group (Get-LocalGroup -SID 'S-1-5-32-545') -Member $name
    $report.account = @{name=$name;sid=$account.SID.Value}
    foreach ($folder in @($root,$ArtifactDirectory)) {
        & icacls $folder /grant "${name}:(OI)(CI)RX" /T /Q *> (Join-Path $out 'acl.log')
        if ($LASTEXITCODE -ne 0) { throw 'Cannot grant test input read access' }
    }
    foreach ($folder in @($out)+$manualRoots) {
        & icacls $folder /grant "${name}:(OI)(CI)M" /T /Q *>> (Join-Path $out 'acl.log')
        if ($LASTEXITCODE -ne 0) { throw 'Cannot grant disposable test-folder access' }
    }
    $cfg = @{
        packagePath=$zip;packageSHA256=(SHA $zip);executable=$exe;out=$out
        sourceCommit=$build.sourceCommit;sourceTree=$build.sourceTree;version=$build.serviceVersion
        buildProvenance=$build.buildProvenance;standardUserSid=$account.SID.Value
        acceptanceMode=$(if ($InstallOnly) {'install-only'} else {'full'})
        playwrightModule=(Join-Path $root 'desktop\node_modules\playwright-core')
        programFilesRoot=$programFilesRoot;pathRoot=$pathRoot;manualRoots=$manualRoots
        runtimeVersion=$pin.version;runtimeCliSHA256=$cliSHA;node=(Get-Command node.exe).Source
        python=(Get-Command python.exe).Source
        frozenSourceRoot=(Join-Path $root 'build\frozen-source')
        canonicalFactoryRoot=(Join-Path $root 'build\canonical-946')
        resourcePolicy=$resourcePolicy
        git=(Get-Command git.exe).Source
        bunProbe=$probeBun
        hostedRun=@{githubActions=$env:GITHUB_ACTIONS;runnerOS=$env:RUNNER_OS;repository=$env:GITHUB_REPOSITORY;runId=$env:GITHUB_RUN_ID;runAttempt=$env:GITHUB_RUN_ATTEMPT}
    }
    $configPath = Join-Path $out 'config.json'
    $cfg | ConvertTo-Json -Depth 10 | Set-Content -Encoding utf8 $configPath
    $report.phase = 'standard-user-native-acceptance'; Save-Report
    $shell = (Get-Command pwsh.exe).Source
    # Pass argv through .NET's actual argument list instead of PowerShell's
    # comma/concatenation expressions or Start-Process's joined command line.
    $start = [Diagnostics.ProcessStartInfo]::new($shell)
    $start.UseShellExecute=$false; $start.WorkingDirectory=$out
    $start.UserName=$name; $start.Domain=$env:COMPUTERNAME
    $start.Password=$password; $start.LoadUserProfile=$true
    $start.RedirectStandardOutput=$true; $start.RedirectStandardError=$true
    foreach ($argument in @('-NoProfile','-NonInteractive','-File',(Join-Path $PSScriptRoot 'user.ps1'),'-ConfigPath',$configPath)) {
        $start.ArgumentList.Add($argument)
    }
    $child = [Diagnostics.Process]::new(); $child.StartInfo=$start
    if (!$child.Start()) { throw 'Cannot launch owned standard-user wrapper' }
    # CreateProcessWithLogonW did not inherit our nested limit in the captured
    # prior attempt. Explicitly admit its captured handle to this SAME capped
    # job; the user's wrapper independently rereads its actual inherited limit.
    $report.childAdmittedToCappedJob=[BP927Job]::Admit($child.Handle)
    $queryHandle=[BP927Job]::DuplicateQueryHandle($child.Handle)
    @{pid=$child.Id;sid=$account.SID.Value;queryHandle=$queryHandle;access='JOB_OBJECT_QUERY';capBytes=8589934592} |
        ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $out 'job-admission.json.tmp')
    Move-Item -LiteralPath (Join-Path $out 'job-admission.json.tmp') -Destination (Join-Path $out 'job-admission.json')
    $wrapperOut=$child.StandardOutput.ReadToEndAsync(); $wrapperErr=$child.StandardError.ReadToEndAsync()
    $report.wrapperPid = $child.Id; Save-Report
    $deadline = [DateTime]::UtcNow.AddMinutes(35)
    $samplesPath = Join-Path $out 'paging-samples.jsonl'
    $report.resourcesBeforeConsumer=Get-BP927Resources
    $report.resourcesBeforeConsumer | ConvertTo-Json -Depth 8 -Compress | Add-Content -Encoding utf8 $samplesPath
    while (!$child.WaitForExit(5000)) {
        $sample=Get-BP927Resources
        Assert-BP927Resources $sample $resourcePolicy.allowExistingPagefile
        $sample | ConvertTo-Json -Depth 8 -Compress | Add-Content -Encoding utf8 $samplesPath
        if ([DateTime]::UtcNow -gt $deadline) { $report.timedOut=$true; throw 'Native acceptance exceeded 35 minutes' }
    }
    $child.Refresh(); $report.consumerExitCode=$child.ExitCode
    if ($wrapperOut.Wait(5000)) { $wrapperOut.Result | Set-Content -Encoding utf8 (Join-Path $out 'wrapper-stdout.txt') }
    if ($wrapperErr.Wait(5000)) { $wrapperErr.Result | Set-Content -Encoding utf8 (Join-Path $out 'wrapper-stderr.txt') }
    if ($child.ExitCode -ne 0) { throw 'Native acceptance wrapper failed; preserve partial result/logs' }
    $result = Get-Content (Join-Path $out 'result.json') -Raw | ConvertFrom-Json
    Require-Equal $result.status 'PASS' 'consumer verdict'
    $report.status='PASS'; $report.phase='done'
} catch {
    $report.status = if ($_.Exception.Message.StartsWith('BLOCKED:')) {'BLOCKED'} else {'FAIL'}
    $report.error = $_ | Out-String
} finally {
    if ($account) {
        # Retire only this disposable account's processes, including resident hubs.
        Get-CimInstance Win32_Process | ForEach-Object {
            $item = $_
            try {
                $owner = Invoke-CimMethod -InputObject $item -MethodName GetOwnerSid -ErrorAction Stop
                if ($owner.Sid -eq $account.SID.Value) {
                    $row = @{pid=$item.ProcessId;executable=$item.ExecutablePath;stopped=$false}
                    try { $owned=[Diagnostics.Process]::GetProcessById([int]$item.ProcessId); $owned.Kill(); $row.stopped=$owned.WaitForExit(5000) }
                    catch { $row.error=$_.Exception.Message; $row.stopped=!(Get-Process -Id $item.ProcessId -ErrorAction SilentlyContinue) }
                    $report.cleanup += $row
                }
            } catch {}
        }
        Remove-LocalUser -Name $account.Name
        $report.accountRemoved = !(Get-LocalUser -Name $account.Name -ErrorAction SilentlyContinue)
        if (!$report.accountRemoved -or @($report.cleanup | Where-Object { !$_.stopped }).Count -ne 0) { $report.status='FAIL';$report.cleanupIncomplete=$true }
    }
    try { $report.resourcesEnd=Get-BP927Resources; Assert-BP927Resources $report.resourcesEnd $resourcePolicy.allowExistingPagefile }
    catch { $report.resourceError=$_ | Out-String; if ($report.status -eq 'PASS') {$report.status='FAIL'} }
    Save-Report
}
if ($report.status -ne 'PASS') { throw "Windows #927 $($report.status): $($report.error)" }
