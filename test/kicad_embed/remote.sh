#!/bin/bash
# Run a test/kicad_embed suite (or run_all.sh) from THIS worktree on the second venue, h@backplane, and behave like a local run:
# the suite's stdout/stderr and exit code come back as they are, and its output directory (logs, screenshots) is copied under $BP_TMP.
#
#   test/kicad_embed/remote.sh [--host h@backplane] [--lane NAME] <suite.sh|run_all.sh> [VAR=value ...]
#   EMBED_MODE=fork test/kicad_embed/remote.sh run_all.sh BACKPLANE_KICAD_BIN=$HOME/.backplane-gpui-fresh/home/kicad/10.99.0-bp.11/bin/kicad
#
# Lanes start it through work_run with the lock `kicad-embed-e2e-remote` (one run at a time on the remote; this script also takes a
# flock there, so two runs that skipped the lock queue instead of clobbering each other). It never takes the desktop's `kicad-embed-e2e`.
# What it does (docs/kicad-embed-e2e.md, "Remote venue"):
#   1. tools: i3, i3-msg, ImageMagick `import` and bun are copied from here (same glibc) with only the shared libraries the remote lacks, no root.
#   2. KiCad runtimes named in a BACKPLANE_KICAD_BIN/KICAD_BIN/... value (or the released/test-build dirs) are rsynced to the remote cache,
#      checksummed, only when their file list changed; a path to a design (KEMBED_PROJ_SRC=...Hubble_DK...) is copied there too (h@backplane only).
#   3. the exact worktree is synced and the hub and client are built by scripts/bp-build.sh (the same path as every remote build), lane `kembed`.
#   4. the suite runs under a private Xvfb with software GL, HOME/TMPDIR/ports as the suites already do; nothing under ~/.backplane* is read or written.
#   5. the run's processes are asserted gone (each carries BP_KEMBED_RUN=<id>; a straggler is reported, killed by pid, and fails the run).
# Everything lives under ~/.cache/bp-kembed-remote/ on the remote (the build tree is bp-build.sh's ~/bp-gpui-build/kembed).
# Timings from here are software-GL numbers, not real-hardware numbers.
set -uo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
# shellcheck source=../../scripts/lib-bptmp.sh
. "$REPO/scripts/lib-bptmp.sh"

HOST=h@backplane; LANE=kembed
while [ $# -gt 0 ]; do
  case "$1" in
    --host) HOST=$2; shift 2 ;;
    --lane) LANE=$2; shift 2 ;;
    -h|--help) sed -n '2,/^set /p' "$0" | sed '$d;s/^# \{0,1\}//'; exit 0 ;;
    *) break ;;
  esac
done
[ $# -ge 1 ] || { echo "usage: remote.sh [--host H] [--lane L] <suite.sh|run_all.sh> [VAR=value ...]" >&2; exit 64; }
SUITE=$1; shift
say() { echo "remote: $*" >&2; }
die() { say "$*"; exit "${2:-70}"; }

# the suite, relative to the worktree
case "$SUITE" in /*) SUITE_ABS=$SUITE ;; *) if [ -f "$PWD/$SUITE" ]; then SUITE_ABS=$PWD/$SUITE; else SUITE_ABS=$HERE/$SUITE; fi ;; esac
[ -f "$SUITE_ABS" ] || die "no such suite: $SUITE" 64
SUITE_ABS=$(readlink -f "$SUITE_ABS")
case "$SUITE_ABS" in "$REPO"/*) SUITE_REL=${SUITE_ABS#"$REPO"/} ;; *) die "the suite must be in this worktree ($REPO)" 64 ;; esac

RID=$(printf '%06x' $((RANDOM * RANDOM % 16777216)))   # short: unix socket paths are limited to ~108 bytes
T=$BP_TMP/bp-kembed-remote-$RID; mkdir -p "$T"
SSH_OPTS=(-o BatchMode=yes -o ControlMaster=auto -o "ControlPath=$T/cm" -o ControlPersist=120 -o ServerAliveInterval=20)
rssh() { ssh "${SSH_OPTS[@]}" "$HOST" "$@"; }
RHOME=$(rssh 'printf %s "$HOME"') || die "cannot reach $HOST" 69
R=$RHOME/.cache/bp-kembed-remote
TREE=$RHOME/bp-gpui-build/$LANE
case "$R" in "$RHOME"/.backplane*) die "refusing a scratch dir under ~/.backplane*" ;; esac

LP=
cleanup() {
  exec 8>&- 2>/dev/null
  [ -n "$LP" ] && { wait "$LP" 2>/dev/null; }
  ssh "${SSH_OPTS[@]}" -O exit "$HOST" >/dev/null 2>&1
  rm -rf "$T/cm" "$T/lockin" "$T/lockout"
}
trap cleanup EXIT
trap 'exit 130' INT TERM HUP

# ---- the remote lock (one run at a time there): a background ssh holds flock until we close its stdin
mkfifo "$T/lockin"
ssh -o BatchMode=yes -o ServerAliveInterval=20 "$HOST" "mkdir -p $R && exec 9>$R/lock && flock 9 && echo LOCKED && while read -r _; do :; done" <"$T/lockin" >"$T/lockout" 2>&1 &
LP=$!
exec 8>"$T/lockin"
waited=0
until grep -q LOCKED "$T/lockout" 2>/dev/null; do
  kill -0 "$LP" 2>/dev/null || die "could not take the remote lock: $(cat "$T/lockout")" 69
  [ "$waited" = 0 ] && say "waiting for the remote lock ($HOST:$R/lock) ..."
  waited=1; sleep 1
done
say "lock held; run $RID, suite $SUITE_REL on $HOST"
say "NOTE: this venue renders with software GL (llvmpipe); any timing it prints is NOT a real-hardware number."
T0=$(date +%s)

# ---- sync helper: DIR on the remote mirrors SRC, checksummed, only when the file list (names, sizes, mtimes) changed
sync_dir() {
  local src=$1 dst=$2 stamp rstamp
  stamp=$(cd "$src" && find . \( -type f -o -type l \) -printf '%p %s %T@ %l\n' | LC_ALL=C sort | sha256sum | cut -c1-24)
  rstamp=$(rssh "cat '$dst.stamp' 2>/dev/null") || true
  if [ "$stamp" = "$rstamp" ] && [ "${KEMBED_REMOTE_VERIFY:-0}" != 1 ]; then return 0; fi
  say "syncing $(basename "$src") ($(du -sh "$src" 2>/dev/null | cut -f1)) -> $dst"
  rssh "mkdir -p '$dst'" || return 1
  rsync -aH --delete --checksum --no-specials -e "ssh ${SSH_OPTS[*]}" "$src/" "$HOST:$dst/" >&2 || return 1
  rssh "printf %s '$stamp' > '$dst.stamp'"
}

# ---- 1. tools: binaries from here plus only the libraries the remote does not have
STAGE=$BP_TMP/kembed-remote-tools; rm -rf "$STAGE"; mkdir -p "$STAGE/bin" "$STAGE/lib" "$STAGE/magick"
IM=$(readlink -f "$(command -v import)"); BUN=$(readlink -f "$(command -v bun || echo "$HOME/.bun/bin/bun")")
for b in i3 i3-msg; do cp -L "$(command -v $b)" "$STAGE/bin/$b" || die "no $b here"; done
cp -L "$IM" "$STAGE/bin/import" || die "no ImageMagick import here"; cp -L "$BUN" "$STAGE/bin/bun" || die "no bun here"
for n in convert identify compare; do cp "$STAGE/bin/import" "$STAGE/bin/$n"; done   # ImageMagick picks its tool from argv[0]
MAGICK_DIR=$(ls -d /usr/lib/x86_64-linux-gnu/ImageMagick-7* 2>/dev/null | head -1); MAGICK_MOD=$(ls -d "$MAGICK_DIR"/modules-Q16* | head -1)
[ -n "$MAGICK_MOD" ] || die "no ImageMagick modules here"
cp -rL "$MAGICK_MOD" "$STAGE/magick/modules"; cp -rL /etc/ImageMagick-7/. "$STAGE/magick/etc/" 2>/dev/null || mkdir -p "$STAGE/magick/etc"
cp -rL "$MAGICK_DIR"/config-Q16*/. "$STAGE/magick/etc/"; cp -rL /usr/share/ImageMagick-7/. "$STAGE/magick/etc/"
mods=$(find "$STAGE/magick" -name '*.so' 2>/dev/null)
ldd "$STAGE"/bin/* $mods 2>/dev/null | awk '/=> \//{print $3}' | sort -u >"$T/needed"
rssh 'while read -r f; do [ -e "$f" ] || echo "$f"; done' <"$T/needed" >"$T/missing"
while read -r f; do [ -n "$f" ] && cp -L "$f" "$STAGE/lib/$(basename "$f")"; done <"$T/missing"
sync_dir "$STAGE" "$R/tools" || die "tools sync failed"
rm -rf "$STAGE"
TOOLENV="PATH=$R/tools/bin:\$PATH LD_LIBRARY_PATH=\${LD_LIBRARY_PATH:+\$LD_LIBRARY_PATH:}$R/tools/lib MAGICK_CODER_MODULE_PATH=$R/tools/magick/modules/coders MAGICK_FILTER_MODULE_PATH=$R/tools/magick/modules/filters MAGICK_CONFIGURE_PATH=$R/tools/magick/etc"
rssh "export $TOOLENV; i3 --version >/dev/null && bun --version >/dev/null && import -version >/dev/null && echo tools-ok" 2>&1 | grep -q tools-ok || die "the copied tools do not start on $HOST (i3/bun/import)"

# ---- 2. env args: paths are shipped, the rest passes through. The caller's environment supplies these when no argument does.
declare -a ENVARGS=()
have() { local a; for a in "$@"; do :; done; for a in "${ENVARGS[@]}"; do [ "${a%%=*}" = "$KEY" ] && return 0; done; return 1; }
for a in "$@"; do ENVARGS+=("$a"); done
for KEY in EMBED_MODE BACKPLANE_KICAD_BIN BACKPLANE_KICAD_CLI KICAD_BIN KEMBED_PROJ_SRC REAL_HARDWARE; do
  have && continue
  [ -n "${!KEY:-}" ] && ENVARGS+=("$KEY=${!KEY}")
done
ship_path() {   # local absolute path -> remote path (copying what must be there)
  local p=$1 root name
  if [ "$p" = "$REPO" ] || [[ "$p" == "$REPO"/* ]]; then echo "$TREE${p#"$REPO"}"; return; fi
  [ -e "$p" ] || { echo "$p"; return; }
  if [[ "$p" == */bin/* ]] && [ -e "${p%%/bin/*}/RUNTIME_MANIFEST.txt" -o -e "${p%%/bin/*}/manifest.json" ]; then
    root=${p%%/bin/*}; name=$(basename "$root")
    sync_dir "$root" "$R/kicad/$name" || die "runtime sync failed ($root)"
    echo "$R/kicad/$name/bin/${p#*/bin/}"; return
  fi
  case "$HOST" in h@backplane) ;; *) [ "${KEMBED_REMOTE_DATA_OK:-0}" = 1 ] || die "refusing to copy $p to $HOST (only h@backplane takes the person's files; KEMBED_REMOTE_DATA_OK=1 overrides)" ;; esac
  name=$(basename "$p"); local d=$R/data/$(printf %s "$p" | sha256sum | cut -c1-8)-$name
  if [ -d "$p" ]; then sync_dir "$p" "$d" || die "data sync failed ($p)"; else rssh "mkdir -p '$R/data'" && rsync -a --checksum -e "ssh ${SSH_OPTS[*]}" "$p" "$HOST:$d" >&2 || die "data sync failed ($p)"; fi
  echo "$d"
}
declare -a RENV=()
for a in "${ENVARGS[@]}"; do
  k=${a%%=*}; v=${a#*=}
  [ "$k" = "$a" ] && die "not VAR=value: $a" 64
  if [[ "$v" == /* ]]; then v=$(ship_path "$v"); fi
  RENV+=("$k=$v")
done

# ---- 3. the tree and the build, by the same script as every remote build
say "syncing the worktree and building the hub and client on $HOST (scripts/bp-build.sh $LANE)"
BUILD='cd native/host && CARGO_TARGET_DIR=$CARGO_TARGET_DIR/host cargo build --profile fast --bin backplane-hub && cd ../gpui && CARGO_TARGET_DIR=$CARGO_TARGET_DIR/gpui cargo build --profile fast'
( cd "$REPO" && "$REPO/scripts/bp-build.sh" "$LANE" "$BUILD" ) >&2 || die "remote build failed" 70
say "built in $(( $(date +%s) - T0 )) s since the lock"

# ---- 4. run (the remote runner is in the tree we just synced)
mkdir -p "$BP_TMP/bp-kembed-remote-out"; OUTLOCAL=$BP_TMP/bp-kembed-remote-out/$RID
q() { printf '%q ' "$@"; }
rssh "export $TOOLENV; bash $TREE/test/kicad_embed/lib/remote_run.sh $(q "$RID" "$LANE" "$SUITE_REL" "${RENV[@]}")"
RC=$?
[ "$RC" = 255 ] && say "the connection to $HOST dropped (exit 255): the remote run is stopped by its own hangup trap"

# ---- 5. results back, remote run dir removed
mkdir -p "$OUTLOCAL"
rsync -a --no-specials --exclude '/fh/.cache' --exclude '*.sock' -e "ssh ${SSH_OPTS[*]}" "$HOST:$R/r/$RID/out/" "$OUTLOCAL/" >&2 \
  && rssh "rm -rf '$R/r/$RID'; find '$R/r' -mindepth 1 -maxdepth 1 -mtime +2 -exec rm -rf {} + 2>/dev/null; true" >/dev/null 2>&1
say "run output: $OUTLOCAL"
say "exit $RC after $(( $(date +%s) - T0 )) s (software GL on $HOST: not real-hardware timing)"
exit "$RC"
