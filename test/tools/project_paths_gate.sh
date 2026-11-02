#!/bin/sh
# Focused registration and agent cwd regressions, on an isolated test hub.
set -eu
cd "$(dirname "$0")/../.."
mkdir -p build/issue667/reports
. scripts/bendok.sh
run() {
  log="build/issue667/reports/$1.log"
  shift
  rc=0
  "$@" >"$log" 2>&1 || rc=$?
  cat "$log"
  return "$rc"
}
run focused env TEST_FILES='test/folder_test.bend test/picker_test.bend test/worktree_test.bend' scripts/test.sh
run required scripts/test.sh
# Check the two new laws directly from their actual declarations/proofs.
# This also gives a focused verdict when the full repository proof cannot
# finish within the sandbox's resource limit.
python3 - <<'PY'
from pathlib import Path

def block(text, head):
    lines = text[text.index(head):].splitlines()
    result = [lines[0]]
    for line in lines[1:]:
        if line and not line[0].isspace():
            break
        result.append(line)
    return '\n'.join(result).rstrip() + '\n\n'

laws, proof = Path('LAWS.bend').read_text(), Path('PROOF.bend').read_text()
text = 'import Base\nimport ../../src/core/hub.bend as H\n\n'
for name in ['project_drive_root_absolute', 'project_unc_root']:
    text += block(laws, 'law ' + name + ':')
    text += block(proof, 'def Laws.' + name + '(').replace('def Laws.', 'def ', 1)
Path('build/issue667/root-laws.bend').write_text(text)
PY
run root-laws bend_check build/issue667/root-laws.bend
run build scripts/build-js.sh src/server/main.bend build/issue667/backplane-serve
run wire-check bend_check test/wire/wire.bend
run wire bend test/wire/index.html -o build/issue667/wire
run registration bun test/tools/proj_thread_e2e.ts build/issue667/backplane-serve build/issue667/wire
run spawn-gone bun test/tools/spawn_gone_e2e.ts build/issue667/backplane-serve build/issue667/wire
run worktrees bun test/tools/worktree_e2e.ts build/issue667/backplane-serve build/issue667/wire
