#!/bin/bash
# Generate the legacy-viewer parity snapshot from origin/main's own code (the rail docking PR compares its single-page flows with it).
# Run it from a checkout of a commit that is on origin/main (a worktree: `git worktree add --detach <dir> origin/main`):
#   scripts/parity-snapshot.sh <branch worktree that holds viewer_parity_tests.rs>
# The generator is `generate_main_snapshot` in the branch's native/host/src/present/viewer_parity_tests.rs, run here with the part of
# that file above its `// ---- branch-only` line; it refuses when this tree has branch-only code, and the snapshot names this commit and its content hash.
# PARITY_REMOTE=<lane>: build and run on h@backplane (scripts/bp-build.sh) instead of here.
set -euo pipefail
BRANCH=${1:?usage: parity-snapshot.sh <branch worktree>}
here=$(git rev-parse --show-toplevel); cd "$here"
git fetch -q origin main
commit=$(git rev-parse HEAD)
git merge-base --is-ancestor "$commit" origin/main || { echo "parity-snapshot: HEAD $commit is not on origin/main" >&2; exit 1; }
! grep -q 'fn layout_action' native/host/src/present/viewer.rs || { echo "parity-snapshot: this tree has branch-only code" >&2; exit 1; }
[ -z "$(git status --porcelain --untracked-files=no)" ] || { echo "parity-snapshot: tracked files are modified" >&2; exit 1; }
src=$BRANCH/native/host/src/present/viewer_parity_tests.rs
sed '/^\/\/ ---- branch-only/,$d' "$src" > native/host/src/present/viewer_parity_tests.rs
grep -q 'mod viewer_parity_tests' native/host/src/present/mod.rs || sed -i 's|^pub mod window;|pub mod window;\n#[cfg(test)] mod viewer_parity_tests;|' native/host/src/present/mod.rs
cmd="PARITY_MAIN_COMMIT=$commit cargo test --manifest-path native/host/Cargo.toml --bin backplane-hub present::viewer_parity_tests::generate_main_snapshot -- --ignored"
if [ -n "${PARITY_REMOTE:-}" ]; then
  scripts/bp-build.sh "$PARITY_REMOTE" "$cmd"
  mkdir -p native/host/src/present/testdata
  rsync -a "h@backplane:bp-gpui-build/$PARITY_REMOTE/native/host/src/present/testdata/" native/host/src/present/testdata/
else
  (cd native/host && BP_NO_BOAT=1 PARITY_MAIN_COMMIT=$commit ../../scripts/cargo-q test --offline --bin backplane-hub present::viewer_parity_tests::generate_main_snapshot -- --ignored)
fi
mkdir -p "$BRANCH/native/host/src/present/testdata"
cp native/host/src/present/testdata/viewer_legacy_parity_main.json "$BRANCH/native/host/src/present/testdata/"
git checkout -q -- native/host/src/present/mod.rs; rm -f native/host/src/present/viewer_parity_tests.rs native/host/src/present/testdata/viewer_legacy_parity_main.json
echo "parity-snapshot: written from $commit"
