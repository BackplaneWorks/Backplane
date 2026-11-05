param([Parameter(Mandatory=$true)][string]$ArtifactDirectory,[switch]$InstallOnly,[switch]$Combined)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_OS -ne 'Windows') { throw 'GitHub Windows runner only' }
if ($Combined -and $InstallOnly) { throw 'Combined and historical InstallOnly modes are mutually exclusive' }
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$out = Join-Path $root 'build\kicad-927-native'
New-Item -ItemType Directory -Force $out | Out-Null
$report = @{status='INCOMPLETE';phase='resource-gate';combined=$Combined.IsPresent;cleanup=@();published=$false;releaseBuildApproved=$false}
# Explicit person authorization: retain the existing hosted Windows pagefile
# for this #927/#946 test, keep the same cap, and measure paging throughout.
$resourcePolicy = @{maxJobMemoryBytes=8589934592;allowExistingPagefile=$true;pagefileModificationAllowed=$false;authorization='User option 1: allow existing GitHub Windows pagefile; retain cap and record paging; user assigned packaged OMP acceptance to the same guarded consumer';scope='Combined #927 KiCad, separate canonical #946 and packaged #949 OMP acceptance only'}
$report.resourcePolicy=$resourcePolicy
$account = $null
$phase = 'setup'
function Save-Report { $report | ConvertTo-Json -Depth 12 | Set-Content -Encoding utf8 (Join-Path $out 'bootstrap.json') }
function SHA($file) { (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant() }
function Require-Equal($actual,$expected,$label) { if ($actual -cne $expected) { throw "Mismatch: $label" } }
function Stop-OwnedProcesses {
    # Only the disposable SID, never a runner service or another account.
    Get-CimInstance Win32_Process | ForEach-Object {
        $item = $_
        try {
            $owner = Invoke-CimMethod -InputObject $item -MethodName GetOwnerSid -ErrorAction Stop
            if ($owner.Sid -eq $account.SID.Value) {
                $row = @{phase=$phase;pid=$item.ProcessId;executable=$item.ExecutablePath;stopped=$false}
                try { $owned=[Diagnostics.Process]::GetProcessById([int]$item.ProcessId); $owned.Kill(); $row.stopped=$owned.WaitForExit(5000) }
                catch { $row.error=$_.Exception.Message; $row.stopped=!(Get-Process -Id $item.ProcessId -ErrorAction SilentlyContinue) }
                $report.cleanup += $row
            }
        } catch {}
    }
}
try {
    . (Join-Path $PSScriptRoot 'job.ps1')
    [BP927Job]::CapSelf()
    $report.resourcesStart = Get-BP927Resources; Save-Report
    Assert-BP927Resources $report.resourcesStart $resourcePolicy.allowExistingPagefile
    $report.phase = 'authenticated-artifact-verification'; Save-Report
    $zip = Join-Path $ArtifactDirectory 'Backplane\Backplane\build\native\package\dist\backplane-v0.14.2-windows-x64.zip'
    $receipt = Join-Path $ArtifactDirectory 'Backplane\Backplane\build\native\package\dist\PACKAGE.json'
    Require-Equal (SHA $zip) '32a7685dc41c8647f19340f012e79e8e94fa2e08e338bdf1a756809feb78a563' 'Windows ZIP SHA256'
    # Bind only the actual new artifact; preserve the old-package refusal.
    if ($Combined -and (SHA $zip) -eq 'de613310949b72b0a70bebf9a07aff021cdea98f894856fabdc42134d678cd22') { throw 'Combined acceptance refuses the historical package' }
    Require-Equal (SHA $receipt) '039214b3d05e00a19135c3daef57a1a8c22777963338592fafd057536f4d6ebc' 'package receipt SHA256'
    Require-Equal (Get-Item -LiteralPath $zip).Length 340101986 'Windows ZIP byte size'
    $packageReceipt = Get-Content -LiteralPath $receipt -Raw | ConvertFrom-Json
    Require-Equal $packageReceipt.sourceCommit 'be904cd4cb21ad6febe17e674918c96945113d07' 'package receipt source'
    Require-Equal $packageReceipt.sourceTree '1034e9dc35fb517f1186ebcdb2d8f3351beb7a59' 'package receipt tree'
    Require-Equal $packageReceipt.channel 'release' 'package channel'
    Require-Equal $packageReceipt.byteQualification 'NO_CURRENT_SOURCE_STOCK_COMPARISON_CARRIED_OVER_OPTION2_AFA96792' 'byte qualification scope'
    if (!$packageReceipt.releaseUseAuthorized -or !$packageReceipt.testOnly -or $packageReceipt.releaseQualified -or $packageReceipt.stockEquivalent) { throw 'Original flags or separate authorization mismatch' }
    $report.artifact = @{runId=41956764458;artifactId=13383063412;name='configured-desktop-package-windows-x64';zipSHA256=(SHA $zip);receiptSHA256=(SHA $receipt)}
    Copy-Item -LiteralPath $receipt -Destination (Join-Path $out 'PACKAGE.json')
    $package = Join-Path $out 'package'
    Expand-Archive -LiteralPath $zip -DestinationPath $package
    $exe = Join-Path $package 'backplane-windows-x64\Backplane.exe'
    Require-Equal (SHA $exe) 'ede316a237497aecc025cc31ac994b5af269a6ba81bf3cb2b612b4f2df4977d0' 'Electron executable'
    Require-Equal (SHA (Join-Path (Split-Path $exe) 'resources\DESKTOP-BUILD.json')) 'fc89366ce2f2dc95f2c7567905e8af761c05bc981f6312c50dd05443b0af1bdf' 'desktop build receipt'
    Require-Equal (SHA (Join-Path (Split-Path $exe) 'resources\backplane-service\BUILD.json')) 'f63b4727dc6aeb956de4fbb5a3664e6b75c532bee1d57376e403d37956930c9d' 'original service build receipt'
    Require-Equal (SHA (Join-Path (Split-Path $exe) 'resources\backplane-service\backplane-serve.exe')) 'd0cb5b3b2d572a072cd41d639147ca93e9b1c1e981272ee7fce82735c5572bc4' 'service executable'
    $build = Get-Content -LiteralPath (Join-Path (Split-Path $exe) 'resources\DESKTOP-BUILD.json') -Raw | ConvertFrom-Json
    Require-Equal $build.sourceCommit 'be904cd4cb21ad6febe17e674918c96945113d07' 'source commit'
    Require-Equal $build.sourceTree '1034e9dc35fb517f1186ebcdb2d8f3351beb7a59' 'source tree'
    Require-Equal $build.serviceVersion '0.14.2' 'version'
    Require-Equal $build.buildProvenance.compilerMode 'composite-settle-41262875' 'compiler mode'
    Require-Equal $build.buildProvenance.producerWorkId 'w-t1792701529-415584-1677595' 'raw producer'
    if (!$build.testOnly -or $build.releaseQualified -or $build.buildProvenance.releaseBuildApproved) { throw 'Test-only provenance changed' }
    Copy-Item -LiteralPath (Join-Path (Split-Path $exe) 'resources\DESKTOP-BUILD.json') -Destination (Join-Path $out 'DESKTOP-BUILD.json')
    $report.phase = 'vendor-runtime-fixtures'; Save-Report
    $pin = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'runtime-pin.json') -Raw | ConvertFrom-Json
    Require-Equal $pin.sourceCommit $build.sourceCommit 'runtime catalog source binding'
    Require-Equal $pin.sourceTree $build.sourceTree 'runtime catalog tree binding'
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
    Require-Equal $cliSHA $pin.runtimeCliSHA256 'vendor CLI member SHA256'
    $report.runtime = @{zipSHA256=(SHA $vendorZip);runtimeCliSHA256=$cliSHA;manifest=$manifest}
    $programFilesRoot = Join-Path $env:ProgramFiles ('KiCad\'+$pin.version)
    $pathRoot = Join-Path $out 'PATH runtime'
    $manualRoots = @('C:\backplane_kicad\','C:\bp927\new tools\test\')
    $destinations = @($programFilesRoot,$pathRoot)+$manualRoots
    if ($InstallOnly -or $Combined) {
        if (Test-Path -LiteralPath (Join-Path $env:ProgramFiles 'KiCad')) { throw 'Install-only runner must have no Program Files KiCad' }
        if ($InstallOnly) { $destinations = @(); $manualRoots = @() }
    }
    function Install-RuntimeFixtures {
        foreach ($destination in $destinations) {
            if (Test-Path -LiteralPath $destination) { throw "Fixture destination already exists: $destination" }
            New-Item -ItemType Directory -Force $destination | Out-Null
            Copy-Item -Path (Join-Path $runtime '*') -Destination $destination -Recurse
        }
    }
    if (!$Combined) { Install-RuntimeFixtures }
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
    $ompCLI = Join-Path $probeDir 'omp-18.16.0.exe'
    if ($Combined) {
        Invoke-WebRequest -Uri 'https://github.com/can1357/oh-my-pi/releases/download/v18.16.0/omp-windows-x64.exe' -OutFile $ompCLI
        Require-Equal (SHA $ompCLI) 'c249a9ed0a57c748fb1247d8ee9e2d0fe1b13229266ce74d8810a76a591e21fc' 'actual OMP 18.16.0 CLI'
    }
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
    $writableFolders = @($out)
    if (!$Combined) { $writableFolders += $manualRoots }
    foreach ($folder in $writableFolders) {
        & icacls $folder /grant "${name}:(OI)(CI)M" /T /Q *>> (Join-Path $out 'acl.log')
        if ($LASTEXITCODE -ne 0) { throw 'Cannot grant disposable test-folder access' }
    }
    $cfg = @{
        packagePath=$zip;packageSHA256=(SHA $zip);executable=$exe;out=$out
        sourceCommit=$build.sourceCommit;sourceTree=$build.sourceTree;version=$build.serviceVersion
        buildProvenance=$build.buildProvenance;standardUserSid=$account.SID.Value
        releaseUseAuthorized=$packageReceipt.releaseUseAuthorized;byteQualification=$packageReceipt.byteQualification
        producerArtifact=$report.artifact
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
    $phases = if ($Combined) { @('install-only','discovery-only') } else { @($cfg.acceptanceMode) }
    $report.phaseResults = @()
    $deadline = [DateTime]::UtcNow.AddMinutes(35)
    foreach ($phase in $phases) {
    $phaseOut = if ($Combined) { Join-Path $out ('phase-'+$phase) } else { $out }
    New-Item -ItemType Directory -Force $phaseOut | Out-Null
    if ($Combined -and $phase -eq 'discovery-only') {
        # The genuine fresh-home installation has completed and its owned
        # processes have stopped before any discovery fixture is installed.
        Install-RuntimeFixtures
        foreach ($folder in $manualRoots) {
            & icacls $folder /grant "${name}:(OI)(CI)M" /T /Q *>> (Join-Path $out 'acl.log')
            if ($LASTEXITCODE -ne 0) { throw 'Cannot grant disposable manual-path fixture access' }
        }
    }
    $phaseCfg = $cfg.Clone()
    $phaseCfg.out = $phaseOut
    $phaseCfg.acceptanceMode = $phase
    $phaseCfg.runCanonical = ($phase -ne 'discovery-only')
    $phaseCfg.omp = @{enabled=($Combined -and $phase -eq 'discovery-only');newSourceBoundCandidate=$Combined.IsPresent;downloadedCLI=$ompCLI}
    $configPath = Join-Path $phaseOut 'config.json'
    $phaseCfg | ConvertTo-Json -Depth 10 | Set-Content -Encoding utf8 $configPath
    $report.phase = 'standard-user-'+$phase; Save-Report
    $shell = (Get-Command pwsh.exe).Source
    # Pass argv through .NET's actual argument list instead of PowerShell's
    # comma/concatenation expressions or Start-Process's joined command line.
    $start = [Diagnostics.ProcessStartInfo]::new($shell)
    $start.UseShellExecute=$false; $start.WorkingDirectory=$phaseOut
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
        ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $phaseOut 'job-admission.json.tmp')
    Move-Item -LiteralPath (Join-Path $phaseOut 'job-admission.json.tmp') -Destination (Join-Path $phaseOut 'job-admission.json')
    $wrapperOut=$child.StandardOutput.ReadToEndAsync(); $wrapperErr=$child.StandardError.ReadToEndAsync()
    $report.wrapperPid = $child.Id; Save-Report
    $samplesPath = Join-Path $out 'paging-samples.jsonl'
    $report.resourcesBeforeConsumer=Get-BP927Resources
    $report.resourcesBeforeConsumer.acceptancePhase=$phase
    $report.resourcesBeforeConsumer | ConvertTo-Json -Depth 8 -Compress | Add-Content -Encoding utf8 $samplesPath
    while (!$child.WaitForExit(5000)) {
        $sample=Get-BP927Resources
        $sample.acceptancePhase=$phase
        Assert-BP927Resources $sample $resourcePolicy.allowExistingPagefile
        $sample | ConvertTo-Json -Depth 8 -Compress | Add-Content -Encoding utf8 $samplesPath
        if ([DateTime]::UtcNow -gt $deadline) { $report.timedOut=$true; throw 'Native acceptance exceeded 35 minutes' }
    }
    $child.Refresh(); $report.consumerExitCode=$child.ExitCode
    if ($wrapperOut.Wait(5000)) { $wrapperOut.Result | Set-Content -Encoding utf8 (Join-Path $phaseOut 'wrapper-stdout.txt') }
    if ($wrapperErr.Wait(5000)) { $wrapperErr.Result | Set-Content -Encoding utf8 (Join-Path $phaseOut 'wrapper-stderr.txt') }
    if ($child.ExitCode -ne 0) { throw 'Native acceptance wrapper failed; preserve partial result/logs' }
    $result = Get-Content (Join-Path $phaseOut 'result.json') -Raw | ConvertFrom-Json
    Require-Equal $result.status 'PASS' 'consumer verdict'
    if ($Combined -and $phase -eq 'install-only') {
        $installCase = @($result.cases | Where-Object { $_.name -eq 'real-install' })
        if ($installCase.Count -ne 1 -or !$installCase[0].noManualOverride -or
            !$installCase[0].realInstallBytesAndCliVerified -or !$installCase[0].installedResolverVerified) { throw 'Genuine empty-home install evidence missing' }
        $canonical = Get-Content -LiteralPath (Join-Path $phaseOut 'canonical-946.json') -Raw | ConvertFrom-Json
        Require-Equal $canonical.status 'PASS' 'independent canonical #946 verdict'
    }
    if ($Combined -and $phase -eq 'discovery-only') {
        foreach ($caseName in @('program-files','native-path','manual-0','manual-1','packaged-omp')) {
            if (@($result.cases | Where-Object { $_.name -eq $caseName }).Count -ne 1) { throw "Required native case missing: $caseName" }
        }
        $ompCase = @($result.cases | Where-Object { $_.name -eq 'packaged-omp' })[0]
        Require-Equal $ompCase.status 'PASS' 'actual packaged OMP thread verdict'
    }
    $report.phaseResults += @{phase=$phase;out=$phaseOut;status=$result.status;wrapperPid=$child.Id;exitCode=$child.ExitCode}
    Stop-OwnedProcesses
    if (@($report.cleanup | Where-Object { !$_.stopped }).Count -ne 0) { throw 'Cannot retire disposable phase processes' }
    Save-Report
    }
    $report.status='PASS'; $report.phase='done'
} catch {
    $report.status = if ($_.Exception.Message.StartsWith('BLOCKED:')) {'BLOCKED'} else {'FAIL'}
    $report.error = $_ | Out-String
} finally {
    if ($account) {
        Stop-OwnedProcesses
        Remove-LocalUser -Name $account.Name
        $report.accountRemoved = !(Get-LocalUser -Name $account.Name -ErrorAction SilentlyContinue)
        if (!$report.accountRemoved -or @($report.cleanup | Where-Object { !$_.stopped }).Count -ne 0) { $report.status='FAIL';$report.cleanupIncomplete=$true }
    }
    try { $report.resourcesEnd=Get-BP927Resources; Assert-BP927Resources $report.resourcesEnd $resourcePolicy.allowExistingPagefile }
    catch { $report.resourceError=$_ | Out-String; if ($report.status -eq 'PASS') {$report.status='FAIL'} }
    Save-Report
}
if ($report.status -ne 'PASS') { throw "Windows #927 $($report.status): $($report.error)" }
