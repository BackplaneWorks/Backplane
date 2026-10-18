# Install actual native CLIs, but never authenticate or run a model turn.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
foreach ($provider in @(
    @{ Name='claude'; Url='https://claude.ai/install.ps1' },
    @{ Name='codex'; Url='https://chatgpt.com/codex/install.ps1' }
)) {
    $file = Join-Path $env:TEMP ("backplane-" + $provider.Name + '-install.ps1')
    Invoke-WebRequest $provider.Url -OutFile $file
    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $file
    if ($LASTEXITCODE -ne 0) { throw "$($provider.Name) installer failed: $LASTEXITCODE" }
}
$userBin = Join-Path $env:USERPROFILE '.local\bin'
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$env:PATH = $userBin + ';' + $userPath + ';' + $env:PATH
foreach ($name in @('claude.exe', 'codex.exe')) {
    $command = Get-Command $name -CommandType Application -ErrorAction Stop
    Write-Output ("NATIVE_PROVIDER=" + $command.Source)
    & $command.Source --version
    if ($LASTEXITCODE -ne 0) { throw "$name version check failed" }
}
if ($env:GITHUB_PATH) {
    $userBin | Out-File $env:GITHUB_PATH -Encoding utf8 -Append
    foreach ($directory in ($userPath -split ';')) {
        if ($directory) { $directory | Out-File $env:GITHUB_PATH -Encoding utf8 -Append }
    }
}
