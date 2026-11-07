#!/bin/sh
# Puts the repo's hooks in place in <common .git>/hooks/ (they serve every worktree) and sets this repo's committer: user.name i2cjak, user.email build@amemb.com (the repo's own config; the global one is never touched).
#   reference-transaction (main.hook): every move of main starts the main builder if it is not running; retired branches are refused; main moves only to commits authored and committed by build@amemb.com.
#   pre-commit, pre-push: the same email rule at commit and push time (scripts/check-author.sh).
# Refuses to replace a hook it did not write. Run it on every machine and Boat setup that clones this repo.
set -eu
here=$(cd "$(dirname "$0")" && pwd); d=$(git rev-parse --git-common-dir)/hooks
mkdir -p "$d"
for p in reference-transaction:main.hook pre-commit:pre-commit.hook pre-push:pre-push.hook; do
  h=$d/${p%%:*}; src=$here/${p#*:}
  if [ -e "$h" ] && ! grep -q 'scripts/install-hooks.sh' "$h"; then echo "another ${p%%:*} hook is there: $h" >&2; exit 1; fi
  cp "$src" "$h"; chmod 755 "$h"; echo "installed $h"
done
git config --local user.name i2cjak; git config --local user.email build@amemb.com
echo "repo config: user.name i2cjak, user.email build@amemb.com"
