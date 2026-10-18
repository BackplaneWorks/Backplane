param([Parameter(ValueFromRemainingArguments=$true)][string[]]$BackplaneArgs)
$ErrorActionPreference = 'Stop'
$roots = @($env:BACKPLANE_CYGWIN_ROOT,
    (Join-Path $env:LOCALAPPDATA 'Backplane\cygwin64'), 'C:\cygwin64')
$bin = $null
foreach ($root in $roots) {
    if ($root -and (Test-Path (Join-Path $root 'bin\bash.exe'))) {
        $bin = Join-Path $root 'bin'; break
    }
}
if (!$bin) { throw 'Run Install-Runtime.ps1 once, then open Backplane.cmd again.' }
$env:PATH = $bin + ';' + $env:PATH
$script = & (Join-Path $bin 'cygpath.exe') -u (Join-Path $PSScriptRoot 'launch.sh')
if ($LASTEXITCODE -ne 0) { throw 'Cannot resolve the application directory' }
& (Join-Path $bin 'bash.exe') --login $script @BackplaneArgs
exit $LASTEXITCODE
