#!/bin/sh
# BACKPLANE-0.15-SHIM: 0.14's backplane-serve, after deploy/compat/install-release.sh. 0.14 re-execs this with its own arguments
# (--home DIR --port N --foreground ...); from here the guard runs Backplane (docs/deploy.md, "From 0.14"):
#   - the guard starts on 0.14's port, so paired phones and a tailscale serve mapping still reach it;
#   - it starts on 0.14 itself (builds/v0.14-legacy runs backplane-serve.0.14 with 0.14's home and arguments), then is asked to move to
#     <prefix>/NEXT on probation: the identity check against EXPECT, and one failure goes straight back to 0.14, so something always runs;
#   - 0.15 gets its own home; BACKPLANE_IMPORT_FROM names 0.14's, which 0.15's setup offers to import (nothing is written until the person says).
# A second start while the guard runs for that home says so and stops. --render, --help and --version stay 0.14's.
set -eu
self=$0
while [ -L "$self" ]; do l=$(readlink "$self"); case $l in /*) self=$l ;; *) self=$(dirname "$self")/$l ;; esac; done
prefix=$(CDPATH= cd -- "$(dirname -- "$self")" && pwd)
case ${1:-} in --render|--help|-h|--version) exec "$prefix/backplane-serve.0.14" "$@" ;; esac
old= port= prev= rest=
for a in "$@"; do
  case $prev in --home) old=$a ;; --port) port=$a ;; esac
  prev=$a
done
old=${old:-$HOME/.backplane-bend}; port=${port:-3787}
case $(uname -s) in Darwin) home=${BACKPLANE_HOME:-$HOME/Library/Application Support/Backplane/hub} ;; *) home=${BACKPLANE_HOME:-$HOME/.backplane/hub} ;; esac
tag=$(cat "$prefix/NEXT" 2>/dev/null || true)
G="$prefix/backplane"
mkdir -p "$home"
if [ -f "$home/guard/lock" ] && ! "$G" hub status --home "$home" 2>/dev/null | grep -q 'No guard is running'; then
  echo "Backplane runs under its guard for $home: backplane hub status"; exit 0
fi
# 0.14 as a build the guard can run and go back to: its own server, home and arguments, whatever the guard passes
leg="$prefix/builds/v0.14-legacy"
if [ -x "$prefix/backplane-serve.0.14" ]; then
  mkdir -p "$leg"
  q() { printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\\\\''/g")"; }
  printf '#!/bin/sh\n# Backplane 0.14 under the guard (deploy/compat/backplane-serve.sh)\nexec %s --home %s --port %s --foreground\n' \
    "$(q "$prefix/backplane-serve.0.14")" "$(q "$old")" "$port" > "$leg/backplane-hub.new"
  chmod 755 "$leg/backplane-hub.new" && mv -f "$leg/backplane-hub.new" "$leg/backplane-hub"
fi
cur=$(cat "$prefix/CURRENT" 2>/dev/null || true)
[ -n "$cur" ] && [ -d "$prefix/builds/$cur" ] || { cur=v0.14-legacy; printf '%s\n' "$cur" > "$prefix/CURRENT"; }
export BACKPLANE_IMPORT_FROM="$old"
# once the guard runs, the move to 0.15 on probation (the same request a normal update makes)
if [ -n "$tag" ] && [ "$cur" != "$tag" ] && [ -d "$prefix/builds/$tag" ]; then
  ( i=0; while [ $i -lt 300 ]; do
      if "$G" hub status --home "$home" 2>/dev/null | grep -q -E 'Running|running'; then "$G" hub update "$tag" --home "$home" --now >> "$home/update-from-0.14.log" 2>&1; exit 0; fi
      sleep 1; i=$((i+1)); done ) < /dev/null > /dev/null 2>&1 &
fi
exec "$G" hub run --home "$home" --port "$port" --builds "$prefix" --build "$cur" --default --log "$home/hub.log" -- --foreground
