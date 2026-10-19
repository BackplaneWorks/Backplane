param([Parameter(ValueFromRemainingArguments=$true)][string[]]$BackplaneArgs)
$ErrorActionPreference = 'Stop'
if ($env:BACKPLANE_CYGWIN_ROOT) {
    $roots = @($env:BACKPLANE_CYGWIN_ROOT)
} else {
    $roots = @((Join-Path $env:LOCALAPPDATA 'Backplane\cygwin64'), 'C:\cygwin64')
}
$bin = $null
foreach ($root in $roots) {
    if ($root -and (Test-Path (Join-Path $root 'bin\bash.exe'))) {
        $bin = Join-Path $root 'bin'; break
    }
}
if (!$bin) { throw 'Configured Cygwin runtime missing. Run Install-Runtime.ps1, then open Backplane.cmd again.' }
foreach ($name in @('cygpath.exe','cygwin1.dll','cygfontconfig-1.dll','cygfreetype-6.dll')) {
    if (!(Test-Path (Join-Path $bin $name))) {
        throw "Cygwin runtime is incomplete ($name missing). Run Install-Runtime.ps1."
    }
}
$env:PATH = $bin + ';' + $env:PATH
$script = & (Join-Path $bin 'cygpath.exe') -u (Join-Path $PSScriptRoot 'launch.sh')
if ($LASTEXITCODE -ne 0) { throw 'Cannot resolve the application directory' }
& (Join-Path $bin 'bash.exe') --login $script @BackplaneArgs
exit $LASTEXITCODE
