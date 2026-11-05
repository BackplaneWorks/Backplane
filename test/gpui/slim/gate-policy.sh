#!/bin/sh
# The GATING policy build, from a clean checkout of a tip: usage (from a worktree whose HEAD is the tip, tree clean): sh test/gpui/slim/gate-policy.sh
# One Boat job (warm sandbox): split build, policy_local, policy_compact, policy_room on a generated synthetic log. On success the binary is copied to
# build/gpui/slim/bp-policy-gate with bp-policy-gate.commit beside it (the commit, its gpui base, the sha256, the check results, "GATING (Boat)").
set -eu
# untracked files count: the Boat job uploads the working tree, so an untracked .bend or test file would be in the build (ignored files are not uploaded)
[ -z "$(git status --porcelain)" ] || { echo "refused: the tree is not clean (tracked changes or untracked files); the commit written beside the binary would not describe it" >&2; exit 1; }
TIP=$(git rev-parse HEAD); mkdir -p build/gpui/slim build/gpui/results/slim
# the old pair goes first: a failed run must not leave a binary beside a commit file that describes another build (nor the reverse)
rm -f build/gpui/slim/bp-policy-gate build/gpui/slim/bp-policy-gate.commit build/gpui/slim/bp-policy-gate.tmp build/gpui/slim/bp-policy-gate.commit.tmp
LOG=build/gpui/results/slim/gate-policy-$(git rev-parse --short HEAD).log
set +e; sh test/gpui/slim/boat-policy-check-job.sh > "$LOG" 2>&1; rc=$?; set -e
JID=$(grep -o 'j2026[0-9-]*' "$LOG" | head -1); A=$HOME/.local/state/backplane/boat/jobs/$JID/artifacts
echo "boat job $JID exit $rc (log $LOG)"
grep -E "^ok|^FAIL|ALL OK|FAILED|TOTAL" "$LOG" | cut -c1-200 | tail -40
[ "$rc" = 0 ] || { echo "GATE FAILED: no binary recorded" >&2; exit "$rc"; }
cp "$A/split/bp-policy.stripped" build/gpui/slim/bp-policy-gate.tmp; mkdir -p build/gpui/results/slim/gate-policy-out; cp "$A"/pl/* build/gpui/results/slim/gate-policy-out/
SUM=$(sha256sum build/gpui/slim/bp-policy-gate.tmp | cut -c1-64)
cat > build/gpui/slim/bp-policy-gate.commit.tmp <<EOT
bp-policy-gate  sha256 $SUM
built from commit $TIP (branch $(git rev-parse --abbrev-ref HEAD), tree clean), gpui at build time $(git rev-parse --short gpui), merge-base with gpui $(git rev-parse --short "$(git merge-base gpui HEAD)")
GATING: Boat job $JID, pinned Bend, clang, $(date -u +%Y-%m-%dT%H:%MZ)
checks: policy_local $(grep -c '^ok' "$A/pl/policy_local.txt") ok, $(grep -E 'ALL OK|FAILED' "$A/pl/policy_local.txt" | tail -1); policy_compact $(grep -E 'ALL OK|FAILED' "$A/pl/policy_compact.txt" | tail -1 | cut -c1-120); policy_room $(grep -E 'ALL OK|FAILED' "$A/pl/policy_room.txt" | tail -1)
EOT
# both are written; only now do they take their names, binary first and its description right after
mv build/gpui/slim/bp-policy-gate.tmp build/gpui/slim/bp-policy-gate && mv build/gpui/slim/bp-policy-gate.commit.tmp build/gpui/slim/bp-policy-gate.commit
cat build/gpui/slim/bp-policy-gate.commit
