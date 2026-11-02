#!/bin/sh
# Focused registration and agent cwd regressions, on an isolated test hub.
set -eu
cd "$(dirname "$0")/../.."
mkdir -p build/issue667/reports
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
run build scripts/build-js.sh src/server/main.bend build/issue667/backplane-serve
. scripts/bendok.sh
run wire-check bend_check test/wire/wire.bend
run wire bend test/wire/index.html -o build/issue667/wire
run registration bun test/tools/proj_thread_e2e.ts build/issue667/backplane-serve build/issue667/wire
run spawn-gone bun test/tools/spawn_gone_e2e.ts build/issue667/backplane-serve build/issue667/wire
run worktrees bun test/tools/worktree_e2e.ts build/issue667/backplane-serve build/issue667/wire
