#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH=/usr/bin:/bin:$PATH LC_ALL=C.UTF-8
export BACKPLANE_JOBS=${BACKPLANE_JOBS:-2} BACKPLANE_UNITS=32
mkdir -p build/windows-input dist
python3 test/tools/windows_source_test.py
python3 test/tools/windows_google_test.py
python3 scripts/windows-source.py check
(cd build/windows-emitted && sha256sum -c SHA256SUMS)
gzip -dc build/windows-emitted/backplane.c.gz > build/windows-input/backplane.c
gzip -dc build/windows-emitted/runtime.c.gz > build/windows-input/runtime.c
gzip -dc build/windows-emitted/providers.c.gz > build/windows-input/providers.c
uname -a
clang --version
scripts/build-windows.sh build/windows-input/backplane.c dist/backplane.exe
scripts/build-windows.sh build/windows-input/runtime.c build/windows-runtime.exe
scripts/build-windows.sh build/windows-input/providers.c build/windows-providers.exe
python3 scripts/prepare-windows-helpers.py
python3 test/tools/windows-kipc.py dist/backplane-kipc.exe
for test in child stdio hublock; do
  clang -std=c11 -D___wchar_t=__WCHAR_TYPE__ -O2 -Wno-extra-tokens \
    "test/tools/windows-$test.c" -o "build/windows-$test.exe" -lpthread
  "build/windows-$test.exe"
done
python3 test/tools/windows-smoke.py dist/backplane.exe build/windows-runtime.exe
python3 - <<'PY'
import re, subprocess
r = subprocess.run(['build/windows-providers.exe'], capture_output=True, text=True,
                   encoding='utf-8', check=True, timeout=90)
print(r.stdout)
for name in ['CLAUDE_DISCOVERY', 'CLAUDE_VERSION', 'CODEX_DISCOVERY', 'CODEX_VERSION']:
    assert re.search(r'^' + name + r' code=0 out=\S', r.stdout, re.MULTILINE), r.stdout
print('WINDOWS_INSTALLED_PROVIDER_DISCOVERY_AND_NATIVE_LAUNCH_PASS')
PY
revision=$(git rev-parse HEAD)
package_version=${BACKPLANE_VERSION:-ci-${revision:0:12}}
python3 scripts/package-windows.py "$package_version"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$(cygpath -w "$PWD/test/tools/windows-package.ps1")" -ZipPath "$(cygpath -w "$PWD/dist/backplane-$package_version-windows-x64.zip")"
