#!/bin/bash
# deploy.sh HOST BUILD_DIR ASSETS_DIR: copy a Windows build (windows-check.sh --xwin) and its assets (with the generated icons.json and cat-rigs)
# to C:\bp\build\<commit> on HOST, and point C:\bp\build\cur at it.
set -euo pipefail
host=$1; b=$2; a=$3; c=$(cut -c1-10 "$b/COMMIT"); d="C:/bp/build/$c"
ssh -o BatchMode=yes "$host" "New-Item -ItemType Directory -Force '$d' | Out-Null" 2>&1 | grep -av '^\*\*' || true
scp -q "$b"/* "$host:$d/"; tar -C "$(dirname "$a")" -cf - "$(basename "$a")" | ssh -o BatchMode=yes "$host" "tar -xf - -C '$d'" 2>&1 | grep -av '^\*\*' || true
ssh -o BatchMode=yes "$host" "cmd /c rmdir C:\\bp\\build\\cur 2>\$null; New-Item -ItemType Junction -Path C:\\bp\\build\\cur -Target '$d' | Out-Null" 2>&1 | grep -av '^\*\*' || true
echo "deployed $c"
