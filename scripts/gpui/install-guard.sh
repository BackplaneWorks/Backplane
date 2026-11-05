#!/bin/sh
# Puts ref-guard.hook in place. Refuses to replace a hook it did not write.
set -eu
here=$(dirname "$0"); h=$(git rev-parse --git-common-dir)/hooks/reference-transaction
if [ -e "$h" ] && ! grep -q 'scripts/gpui/install-guard.sh' "$h"; then echo "another reference-transaction hook is there: $h" >&2; exit 1; fi
cp "$here/ref-guard.hook" "$h"; chmod 755 "$h"; echo "installed $h"
