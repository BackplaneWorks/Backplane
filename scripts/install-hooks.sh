#!/bin/sh
# Puts the repo's hooks in place in <common .git>/hooks/ (they serve every worktree) and sets this repo's committer: user.name i2cjak, user.email build@amemb.com (the repo's own config; the global one is never touched).
#   reference-transaction (main.hook): every move of main starts the main builder if it is not running; retired branches are refused; new main commits must pass check-author.sh.
#   pre-commit, pre-push: the same rule at commit and push time (scripts/check-author.sh).
# Refuses to replace a hook it did not write. Run it on every machine and Boat setup that clones this repo.
set -eu
here=$(cd "$(dirname "$0")" && pwd); d=$(git rev-parse --git-common-dir)/hooks
mkdir -p "$d"
# Install the checker with the hooks: the main checkout and other worktrees may still contain an old rule.
chk=$d/check-author.sh
if [ -e "$chk" ] && ! grep -q 'scripts/install-hooks.sh' "$chk"; then echo "another checker is there: $chk" >&2; exit 1; fi
cp "$here/check-author.sh" "$chk"; chmod 755 "$chk"
for p in reference-transaction:main.hook pre-commit:pre-commit.hook pre-push:pre-push.hook; do
  h=$d/${p%%:*}; src=$here/${p#*:}
  if [ -e "$h" ] && ! grep -q 'scripts/install-hooks.sh' "$h"; then echo "another ${p%%:*} hook is there: $h" >&2; exit 1; fi
  cp "$src" "$h"; chmod 755 "$h"; echo "installed $h"
done
git config --local user.name i2cjak; git config --local user.email build@amemb.com
echo "repo config: user.name i2cjak, user.email build@amemb.com"
