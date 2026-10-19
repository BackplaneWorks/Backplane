param(
    [string]$Root = $(if ($env:BACKPLANE_CYGWIN_ROOT) { $env:BACKPLANE_CYGWIN_ROOT } else { Join-Path $env:LOCALAPPDATA 'Backplane\cygwin64' }),
    [switch]$BuildTools
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$cache = Join-Path $env:TEMP 'backplane-cygwin-setup'
New-Item -ItemType Directory -Force $cache | Out-Null
$setup = Join-Path $cache 'setup-x86_64.exe'
Invoke-WebRequest 'https://cygwin.com/setup-x86_64.exe' -OutFile $setup
Get-FileHash $setup -Algorithm SHA256 | Format-List
$packages = 'bash,coreutils,findutils,git,curl,fontconfig,libfontconfig1,libfreetype6,dejavu-fonts'
if ($BuildTools) { $packages += ',clang,python3,make' }
# Setup verifies the mirror's package signatures by default. No elevation,
# shortcuts or changes to another Cygwin installation are requested.
$options = @('-q','-n','-N','-B','-O','-R',$Root,'-l',$cache,
    '-s','https://mirrors.kernel.org/sourceware/cygwin/','-P',$packages)
$quoted = $options | ForEach-Object { '"' + $_ + '"' }
$p = Start-Process $setup -ArgumentList $quoted -PassThru -Wait
if ($p.ExitCode -ne 0) { throw "Cygwin setup failed: $($p.ExitCode)" }
foreach ($name in @('bash.exe','cygpath.exe','cygwin1.dll','cygfontconfig-1.dll','cygfreetype-6.dll')) {
    if (!(Test-Path (Join-Path $Root ('bin\' + $name)))) { throw "Cygwin runtime missing $name" }
}
Write-Output "BACKPLANE_CYGWIN_ROOT=$Root"
