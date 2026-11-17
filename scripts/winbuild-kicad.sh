#!/usr/bin/env bash
# Build the Backplane KiCad Windows runtime for a fork commit on bp-winbuild (32 cores; vcpkg's binary
# cache and ccache on its data disk), as .github/workflows/backplane-windows.yml builds it, and copy the
# package back. Takes the winbuild-vm lock itself, starts the VM and deallocates it when done.
#
#   scripts/winbuild-kicad.sh <commit> [--scripts-from <ref>] [--clean-build] [--libraries] [--keep-up]
#
#   --scripts-from REF  the Windows build scripts from REF over the commit (a commit older than them)
#   --clean-build       a fresh build folder (caches kept): what a new release commit costs
#   --libraries         stage the pinned libraries and 3D models too (a release package needs them)
#   --keep-up           leave the VM running (it still deallocates itself 20 min after the build)
# The package lands in ~/.cache/bp-winbuild/out/<commit>/; the log in ~/.cache/bp-winbuild/logs/.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
if [ -z "${BP_WINBUILD_LOCKED:-}" ]; then
  BP_WINBUILD_LOCKED=1 exec "$here/bp-lock.sh" winbuild-vm -- "$0" "$@"
fi
commit=${1:?usage: winbuild-kicad.sh COMMIT [--scripts-from REF] [--clean-build] [--libraries] [--keep-up]}; shift
scripts_from=; clean=; libs=-SkipLibraries; keep=
while [ $# -gt 0 ]; do
  case $1 in
    --scripts-from) scripts_from=$2; shift 2 ;;
    --clean-build) clean=1; shift ;;
    --libraries) libs=; shift ;;
    --keep-up) keep=1; shift ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
done
out=$HOME/.cache/bp-winbuild; mkdir -p "$out/logs"
log=$out/logs/$(date -u +%Y%m%dT%H%M%SZ)-${commit:0:10}.log
exec > >(tee -a "$log") 2>&1
stamp() { echo "== $(date -u +%FT%TZ) $*"; }
host=$("$here/winbuild-up.sh")
stamp "host $host, commit $commit${scripts_from:+, scripts from $scripts_from}${clean:+, clean build}"
[ -n "$keep" ] || trap '"$here/winbuild-down.sh" || true' EXIT
# sshd keeps the PATH it started with: each command reads the machine's current one (the toolchain adds to it)
win() { ssh -o ServerAliveInterval=30 -o ServerAliveCountMax=10 "$host" \
  "\$env:Path=[Environment]::GetEnvironmentVariable('Path','Machine')+';'+[Environment]::GetEnvironmentVariable('Path','User'); $1"; }

# The toolchain first (git comes with it), from the ref that has the build scripts
stamp toolchain
ref=${scripts_from:-$commit}
win "\$ErrorActionPreference='Stop'; \$f=\"\$env:TEMP\\backplane-windows-toolchain.ps1\"
Invoke-WebRequest -UseBasicParsing https://raw.githubusercontent.com/BackplaneWorks/Backplane_KiCad/$ref/scripts/backplane-windows-toolchain.ps1 -OutFile \$f
pwsh -NoProfile -File \$f"

stamp source
win "\$ErrorActionPreference='Stop'; \$PSNativeCommandUseErrorActionPreference=\$true
if (!(Test-Path C:\\src\\kicad\\.git)) { git clone --quiet --no-checkout https://github.com/BackplaneWorks/Backplane_KiCad.git C:\\src\\kicad }
Set-Location C:\\src\\kicad
git fetch --quiet origin $commit
git checkout --quiet --force --detach $commit
git clean -fdxq -e release
if ('$scripts_from') {
  git fetch --quiet origin $scripts_from
  git checkout --quiet FETCH_HEAD -- scripts/backplane-windows.ps1 scripts/backplane-windows-local.ps1 scripts/backplane-windows-toolchain.ps1
}
git log --oneline -1; git status --short"

if [ -n "$clean" ]; then
  stamp 'clean build folder'
  # MSVC's PDB server and a finished build's helpers can still hold files in the build folder
  win "Get-Process mspdbsrv, vctip, ninja, cl, link, ccache -ErrorAction SilentlyContinue | Stop-Process -Force
foreach (\$d in 'D:\\bpk\\build', 'D:\\bpk\\stage') { if (Test-Path \$d) { cmd /c rd /s /q \$d } }
if ((Test-Path D:\\bpk\\build) -or (Test-Path D:\\bpk\\stage)) { throw 'the build folder could not be removed' }
'removed D:\\bpk\\build and D:\\bpk\\stage'"
fi
stamp build
start=$(date +%s)
win "pwsh -NoProfile -File C:\\src\\kicad\\scripts\\backplane-windows-local.ps1 -Root D:\\bpk -Jobs 32 -Priority Normal -Ccache $libs"
stamp "build done in $(( ($(date +%s) - start) / 60 )) min $(( ($(date +%s) - start) % 60 )) s"
mkdir -p "$out/out/$commit"
# What the workflow uploads: the runtime and symbol zips, the source archive, SHA256SUMS (not the unpacked folder)
for f in $(win "Get-ChildItem C:\\src\\kicad\\release\\windows -File | ForEach-Object Name" | tr -d '\r'); do
  scp -q "$host:C:/src/kicad/release/windows/$f" "$out/out/$commit/"
done
(cd "$out/out/$commit" && sha256sum -c --ignore-missing SHA256SUMS)
ls -la "$out/out/$commit"
