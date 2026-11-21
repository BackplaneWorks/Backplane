#!/bin/bash
# The catalogs generated from i18n/*.msg match the tree: scripts/land-gate.sh runs this for a PR touching the .msg files, a generator or a
# generated catalog, and scripts/land.sh again in the land lock when its last rebase moved the branch (main took other keys meanwhile).
# HEAD's i18n/, the generators and the mobile folders they write into are copied into a temporary git repo; scripts/i18n-check.py,
# scripts/i18n-android.py and scripts/i18n-ios.py run there; any file they change or add is stale (or missing) in the tree: named, with how
# many lines differ and the generator to run. No build, about a second.
# Nov 20: main's Msg.kt went 5,890 lines stale (TrCatalog.swift 23) when 18e4a80f2e, its catalogs made before its rebase, landed after
# e8b147b362's web.clone.* keys: the .msg files merged cleanly, the generated files did not know the new keys.
# usage: i18n-gate.sh [REV] (default HEAD). Exit 0 current, 1 stale or a generator failed.
set -u
rev=${1:-HEAD}; ROOT=$(git rev-parse --show-toplevel) || exit 1; cd "$ROOT"
say() { echo "gate: i18n: $*" >&2; }
gens="scripts/i18n-android.py scripts/i18n-ios.py"
ins="i18n scripts/i18n-check.py $gens mobile/android/app/src/main/java/dev/backplane/mobile mobile/ios/Shared mobile/ios/Backplane"
tmp=$(mktemp -d "${TMPDIR:-/tmp}/i18n-gate.XXXXXX") || { say "no temporary folder"; exit 1; }
trap 'rm -rf "$tmp"' EXIT; trap 'exit 130' INT; trap 'exit 143' TERM HUP
paths=$(for p in $ins; do git cat-file -e "$rev:$p" 2>/dev/null && echo "$p"; done)
[ -n "$paths" ] || { say "nothing to check at $rev"; exit 0; }
# shellcheck disable=SC2086
git archive "$rev" -- $paths > "$tmp/in.tar" && tar -x -C "$tmp" -f "$tmp/in.tar" && rm -f "$tmp/in.tar" || { say "could not copy $rev's files"; exit 1; }
git -C "$tmp" init -q && git -C "$tmp" add -A && git -C "$tmp" -c user.name=g -c user.email=g@g commit -qm in || { say "could not set up the copy"; exit 1; }
bad=
[ ! -f "$tmp/scripts/i18n-check.py" ] || (cd "$tmp" && python3 -I scripts/i18n-check.py) >&2 || bad="the .msg files disagree (python3 scripts/i18n-check.py)"
for g in $gens; do [ -f "$tmp/$g" ] || continue
  out=$(cd "$tmp" && python3 -I "$g" 2>&1 > /dev/null) || { printf '%s\n' "$out" | sed 's/^/gate: /' >&2; bad="${bad:+$bad; }$g failed (above)"; continue; }
  # what this generator changed or added: stale in the tree
  while IFS= read -r f; do [ -n "$f" ] || continue
    n=$(git -C "$tmp" diff --numstat -- "$f" | awk '{print $1 + $2}')
    [ -n "$n" ] || n="new file, $(wc -l < "$tmp/$f")"
    bad="${bad:+$bad; }$f is stale ($n lines differ): run python3 $g and commit it"
  done < <(git -C "$tmp" status --porcelain --untracked-files=all | cut -c4-)
  git -C "$tmp" add -A && git -C "$tmp" -c user.name=g -c user.email=g@g commit -qm "$g" --allow-empty
done
if [ -z "$bad" ]; then say "the catalogs are what i18n/*.msg makes"; exit 0; fi
say "FAILED: $bad"; exit 1
