#!/bin/sh
# Puts main.hook in place as the reference-transaction hook: every move of main then starts the main builder if it is not running. Refuses to replace a hook it did not write.
set -eu
here=$(cd "$(dirname "$0")" && pwd); h=$(git rev-parse --git-common-dir)/hooks/reference-transaction
if [ -e "$h" ] && ! grep -q 'scripts/install-hooks.sh' "$h"; then echo "another reference-transaction hook is there: $h" >&2; exit 1; fi
cp "$here/main.hook" "$h"; chmod 755 "$h"; echo "installed $h"
