#!/bin/sh
# Run right before `gh pr merge`: check LAWS.bend/PROOF.bend as they will be
# on main after this branch merges (origin/main + HEAD, merged in memory, the
# repo's merge driver applied), not as they are on the branch. A check.sh that
# passed before the last rebase proves nothing about the merged result.
#   scripts/pre-merge.sh [BASE]     BASE defaults to origin/main
set -eu
cd "$(dirname "$0")/.."
base=${1:-origin/main}
git fetch -q origin
tree=$(git merge-tree --write-tree "$base" HEAD) || {
  echo "pre-merge: $base and HEAD conflict; rebase first" >&2
  printf '%s\n' "$tree" | tail -n +2 >&2
  exit 1
}
tree=$(printf '%s\n' "$tree" | head -n 1)
d=$(mktemp -d)
trap 'rm -rf "$d"' 0 1 2 15
git show "$tree:LAWS.bend" >"$d/LAWS.bend"
git show "$tree:PROOF.bend" >"$d/PROOF.bend"
python3 scripts/laws-shape.py "$d/LAWS.bend" "$d/PROOF.bend" && echo "pre-merge: merged LAWS/PROOF shape ok"
