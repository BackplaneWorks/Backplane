param([Parameter(Mandatory=$true)][string]$ZipPath)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$directory = Join-Path ([IO.Path]::GetTempPath()) ('Backplane ZIP ' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $directory | Out-Null
Expand-Archive -LiteralPath $ZipPath -DestinationPath $directory
$root = Join-Path $directory 'backplane-windows-x64'
$manifest = Get-Content (Join-Path $root 'BUILD.json') -Raw | ConvertFrom-Json
foreach ($entry in $manifest.sha256.PSObject.Properties) {
 $actual = (Get-FileHash (Join-Path $root $entry.Name) -Algorithm SHA256).Hash.ToLowerInvariant()
 if ($actual -ne $entry.Value) { throw "Extracted package checksum mismatch: $($entry.Name)" }
}
foreach ($name in @('backplane.exe','backplane-kipc.exe','backplane-step2glb.exe','Install-Providers.ps1','licenses\bun\LICENSE.md')) {
 if (!(Test-Path (Join-Path $root $name))) { throw "Missing packaged component: $name" }
}
$env:BACKPLANE_HOME = Join-Path $directory 'isolated-home'
$env:BACKPLANE_NO_UPDATE = '1'
$help = & (Join-Path $root 'Backplane.cmd') --help 2>&1
if ($LASTEXITCODE -ne 0 -or ($help -join "`n") -notmatch 'Usage:') { throw ("Packaged launcher failed: " + ($help -join "`n")) }
if (Test-Path $env:BACKPLANE_HOME) { throw 'Packaged --help unexpectedly created its home' }
& (Join-Path $root 'backplane-step2glb.exe') --version
if ($LASTEXITCODE -ne 0) { throw 'Packaged STEP helper cannot execute' }
Remove-Item -LiteralPath $directory -Recurse
Write-Output 'WINDOWS_EXTRACTED_ZIP_CHECKSUMS_AND_SPACE_PATH_LAUNCHER_PASS'
