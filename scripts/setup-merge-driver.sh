#!/bin/sh
# Install the block-wise merge driver for LAWS.bend and PROOF.bend in this
# clone (all its worktrees): scripts/merge-laws.py, copied beside the git dir so
# an old commit being rebased cannot lack it. Replaces `merge=union` lines.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
common=$(git rev-parse --path-format=absolute --git-common-dir)
cp "$here/merge-laws.py" "$common/merge-laws.py"
git config merge.laws.name "LAWS/PROOF merge by block"
git config merge.laws.driver "python3 $common/merge-laws.py %O %A %B"
mkdir -p "$common/info"
attr="$common/info/attributes"
touch "$attr"
grep -v -e '^LAWS.bend ' -e '^PROOF.bend ' "$attr" >"$attr.new" || true
printf 'LAWS.bend merge=laws\nPROOF.bend merge=laws\n' >>"$attr.new"
mv "$attr.new" "$attr"
echo "merge driver 'laws' installed ($common/merge-laws.py)"
