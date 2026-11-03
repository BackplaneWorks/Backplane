#!/bin/sh
set -eu
mkdir -p build
export BUN_JSC_forceRAMSize=4294967296 BUN_JSC_maxPerThreadStackUsage=536870912
ulimit -s 1048576
. scripts/bendok.sh
python3 scripts/laws-shape.py
python3 - <<'PY'
from pathlib import Path
import re
laws='\n'.join(re.findall(r'^law autosettle_\w+:[\s\S]*?(?=^law |\Z)', Path('LAWS.bend').read_text(), re.M))
proof='\n'.join(re.findall(r'^def Laws\.autosettle_\w+\([^\n]*\):[\s\S]*?(?=^def |\Z)', Path('PROOF.bend').read_text(), re.M))
Path('build/autosettle-laws.bend').write_text('import Base\nimport ../src/core/model.bend as M\nimport ../src/core/hub.bend as H\nimport ../src/core/autosettle.bend as AUTOS\n\n'+laws)
Path('build/autosettle-proof.bend').write_text('import Base\nimport ../src/core/model.bend as M\nimport ./autosettle-laws.bend as Laws\n\n'+proof)

PY
rc=0
bend build/autosettle-proof.bend > build/autosettle-scoped-proof.log 2>&1 || rc=$?
cat build/autosettle-scoped-proof.log
bend_verdict "$rc" "$(cat build/autosettle-scoped-proof.log)"
if [ "${1:-}" = --scoped ]; then exit 0; fi
rc=0
bend PROOF.bend > build/autosettle-proof.log 2>&1 || rc=$?
cat build/autosettle-proof.log
bend_verdict "$rc" "$(cat build/autosettle-proof.log)"
