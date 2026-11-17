# bp-winbuild's release runner (docs/release-runners.md): the toolchain release.yml's windows-x64 build needs, then the GitHub Actions runner
# as a Windows service (NETWORK SERVICE) with its work dir on the data disk. Run as an administrator on the VM; it is safe to run again:
# whatever is already in place at its pinned version is kept. scripts/winvm/release-runner-push.sh copies it over and runs it.
#   -TokenFile: a file holding a runner registration token (removed after); without it the runner is not (re)registered.
# Everything is under -Root (D:\bp-rel): rustup, cargo, node, 7-Zip, sccache, the runner, its work dir and sccache's cache, so KiCad's
# build folders (D:\bpk) and the runner's never meet. Every download is checked against a pinned sha256.
param(
  [string]$Root = 'D:\bp-rel',
  [string]$Label = 'bp-winbuild-release',
  [string]$Name = 'bp-winbuild',
  [string]$TokenFile = ''
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$pins = @{
  runner  = @{ v = '2.338.2'; sha = 'a1f17aa987fc8158bc7521d284912267a0d01b7c2c4684b33c86783b5e8221cc' }
  rustup  = @{ sha = '38027d51d69a5d05493627a0634dbea52d6cd793c8f072d2373d3f4163eec7db' }
  node    = @{ v = '22.23.5'; sha = '4a191082e9706866830379d577d2cc8a50c776fcd9436a8d061b063ed0d9a0e3' }
  sevenz  = @{ v = '25.01'; sha = 'e7eb0b7ed5efa4e087b7b17f191797f7af5b7f442d1290c66f3a21777005ef57' }
  sccache = @{ v = '0.19.0'; sha = '130e4f8eb672e76590e9993c2ab890c4763b88238577bb11d8062158c7698353' }
}
$tools = "$Root\tools"; $dl = "$Root\dl"; $runner = "$Root\runner"
New-Item -ItemType Directory -Force $Root, $tools, $dl, "$tools\bin", "$Root\sccache", "$Root\_work", "$Root\home" | Out-Null

function Get-Pinned([string]$url, [string]$sha, [string]$out) {
  if ((Test-Path $out) -and ((Get-FileHash -Algorithm SHA256 $out).Hash -ieq $sha)) { return $out }
  Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $out
  $got = (Get-FileHash -Algorithm SHA256 $out).Hash
  if ($got -ine $sha) { Remove-Item $out; throw "$url : sha256 $got, pinned $sha" }
  $out
}

# Rust: rustup in $Root, stable with the MSVC target (release.yml's dtolnay/rust-toolchain then picks stable through it)
$env:RUSTUP_HOME = "$Root\rustup"; $env:CARGO_HOME = "$Root\cargo"
if (-not (Test-Path "$Root\cargo\bin\rustup.exe")) {
  $ri = Get-Pinned 'https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe' $pins.rustup.sha "$dl\rustup-init.exe"
  & $ri -y --no-modify-path --profile minimal --default-toolchain stable --default-host x86_64-pc-windows-msvc
  if ($LASTEXITCODE) { throw "rustup-init failed ($LASTEXITCODE)" }
}
& "$Root\cargo\bin\rustup.exe" --version

# Node (npm: release.yml's last try for bun)
$nodeDir = "$tools\node-v$($pins.node.v)-win-x64"
if (-not (Test-Path "$nodeDir\node.exe")) {
  $z = Get-Pinned "https://nodejs.org/dist/v$($pins.node.v)/node-v$($pins.node.v)-win-x64.zip" $pins.node.sha "$dl\node.zip"
  Expand-Archive -Force $z $tools
}

# 7-Zip (the Windows package step zips with 7z): the MSI unpacked (msiexec /a), nothing installed system-wide
$sz = "$tools\7zip"
if (-not (Test-Path "$sz\Files\7-Zip\7z.exe")) {
  $m = Get-Pinned "https://github.com/ip7z/7zip/releases/download/$($pins.sevenz.v)/7z$($pins.sevenz.v -replace '\.','')-x64.msi" $pins.sevenz.sha "$dl\7z.msi"
  $p = Start-Process msiexec.exe -Wait -PassThru -ArgumentList '/a', "`"$m`"", '/qn', "TARGETDIR=`"$sz`""
  if ($p.ExitCode) { throw "msiexec /a 7z.msi: $($p.ExitCode)" }
}

# sccache (RUSTC_WRAPPER on self-hosted runners; its cache in $Root\sccache)
if (-not (Test-Path "$tools\bin\sccache.exe") -or -not ((& "$tools\bin\sccache.exe" --version) -match [regex]::Escape($pins.sccache.v))) {
  $t = Get-Pinned "https://github.com/mozilla/sccache/releases/download/v$($pins.sccache.v)/sccache-v$($pins.sccache.v)-x86_64-pc-windows-msvc.tar.gz" $pins.sccache.sha "$dl\sccache.tgz"
  tar -xzf $t -C $dl
  Copy-Item -Force "$dl\sccache-v$($pins.sccache.v)-x86_64-pc-windows-msvc\sccache.exe" "$tools\bin\sccache.exe"
}

# python3: release.yml calls python3 (GitHub's Windows image has one); a copy of python.exe beside the installed Python, as that image does
$py = (Get-Command python -ErrorAction Stop).Source
$py3 = Join-Path (Split-Path $py) 'python3.exe'
if (-not (Test-Path $py3)) { Copy-Item $py $py3 }

# fxc.exe (gpui's build compiles its HLSL shaders with it): the newest Windows SDK's; gpui looks only on PATH and in one SDK version's folder
$fxc = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin\*\x64\fxc.exe' -ErrorAction SilentlyContinue |
  Sort-Object { [version]($_.Directory.Parent.Name) } | Select-Object -Last 1
if (-not $fxc) { throw 'no fxc.exe in any Windows 10/11 SDK: install the Windows SDK with Visual Studio Build Tools' }

# The runner
if (-not (Test-Path "$runner\config.cmd")) {
  $z = Get-Pinned "https://github.com/actions/runner/releases/download/v$($pins.runner.v)/actions-runner-win-x64-$($pins.runner.v).zip" $pins.runner.sha "$dl\runner.zip"
  New-Item -ItemType Directory -Force $runner | Out-Null
  Expand-Archive -Force $z $runner
}
# the guard: scripts/runner-guard.ps1, copied beside this script by release-runner-push.sh; release-only: no nightly runs here
Copy-Item -Force "$PSScriptRoot\runner-guard.ps1" "$runner\runner-guard.ps1"
Set-Content -Encoding ascii "$runner\runner-guard.release-only" 'bp-winbuild takes release.yml only (tags, dry runs by hand), never a nightly'
# The job's environment (the service's own environment is NETWORK SERVICE's). Git's bin first for bash, Git's usr\bin for find/sort is added by release.yml.
$path = @("$Root\cargo\bin", "$tools\bin", $nodeDir, "$sz\Files\7-Zip", 'C:\Program Files\Git\bin', 'C:\Program Files\Git\cmd', (Split-Path $py),
  (Join-Path (Split-Path $py) 'Scripts'), 'C:\Windows\System32', 'C:\Windows', 'C:\Windows\System32\WindowsPowerShell\v1.0', 'C:\Program Files\PowerShell\7') -join ';'
@(
  "PATH=$path",
  "HOME=$Root\home",   # NETWORK SERVICE has none in Git's bash (~/.cache: the OpenCascade build, bun)
  "RUSTUP_HOME=$Root\rustup",
  "CARGO_HOME=$Root\cargo",
  "SCCACHE_DIR=$Root\sccache",
  "GPUI_FXC_PATH=$($fxc.FullName)",
  'SCCACHE_CACHE_SIZE=20G',
  "ACTIONS_RUNNER_HOOK_JOB_STARTED=$runner\runner-guard.ps1",
  'LANG=en_US.UTF-8'
) | Set-Content -Encoding ascii "$runner\.env"

# NETWORK SERVICE writes everything under $Root, nothing else
icacls $Root /grant '*S-1-5-20:(OI)(CI)M' /T /Q | Out-Null

if ($TokenFile) {
  $tok = (Get-Content -Raw $TokenFile).Trim(); Remove-Item -Force $TokenFile
  Push-Location $runner
  try {
    if (Test-Path "$runner\.runner") { & .\config.cmd remove --token $tok; if ($LASTEXITCODE) { throw "config remove: $LASTEXITCODE" } }
    & .\config.cmd --unattended --url https://github.com/BackplaneWorks/Backplane --token $tok --name $Name --labels $Label --no-default-labels `
      --work "$Root\_work" --runasservice --windowslogonaccount 'NT AUTHORITY\NETWORK SERVICE' --replace
    if ($LASTEXITCODE) { throw "config.cmd: $LASTEXITCODE" }
  } finally { Pop-Location }
}
Get-Service -Name 'actions.runner.*' | Format-Table Name, Status, StartType
