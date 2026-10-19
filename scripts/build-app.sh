#!/bin/sh
# Build a Bend program to a native binary in parallel, gently.
#
#   scripts/build-app.sh src/app/main.bend build/backplane
#
# `bend X -o bin` compiles one ~30 MB C file on one core (minutes, ~20 GB).
# Here bend emits the C, scripts/cc-split.py deals it into units, and clang
# compiles them side by side at low priority, so the machine stays usable.
# Builds on this machine share one lock (/tmp/bp-wt-build.lock, as the
# agents' builds do), so only one runs at a time, and stay on 4 cores.
#   BACKPLANE_JOBS  parallel compiles (default 4; 1 with BACKPLANE_LOW_MEM=1)
#   BACKPLANE_LOW_MEM=1  for small machines: bend's heap told 4 GiB and one
#                     compile at a time (slower; see the emit note below)
#   BACKPLANE_CPUS  the cores builds may use (default 0-3; "" for any)
#   BACKPLANE_CFLAGS  (default: -O3, as bend -o)
#   BACKPLANE_UNITS   C units the segments are dealt into (default 16)
#   BACKPLANE_OBJ_CACHE  objects by hash of unit text + compiler + flags
#                     (default ~/.cache/backplane-bend/obj; 0 = none), so a
#                     rebuild compiles only the units an edit changed
#   BACKPLANE_EMIT_CACHE  the emitted C kept by a hash of everything bend reads
#                     (default ~/.cache/backplane-bend/emit; 0 = none): the
#                     bend binary and its own files (~/.bend/bend2), this
#                     script and cc-split.py, every tracked or untracked
#                     (not ignored) .bend/.c/.h/.js file of the checkout, and
#                     the program named. A build whose key is kept skips
#                     `bend -o` (the ~12 GB, minutes-long step). Only whole
#                     emits are kept (a finished emit, renamed into place);
#                     the C holds no credential (the baked Google client is
#                     a header made after, never part of it). Three are kept.
#   BACKPLANE_VERSION_STAMP  the version this build reports (Version.current),
#                     written into the emitted C (scripts/stamp-version.py);
#                     unset, it reads 0.1.0-dev
# Each build prints one "stages:" line and keeps it in build/cc/<out>/stages.txt:
# wall seconds per stage (emit, split, cc, link), the object cache's hits and
# misses, the emit's peak RSS (bend alone), the largest single compiler's peak
# RSS (one clang), this build's own compile processes summed (sampled every
# 0.5 s, so a short spike can be missed), and, when the build
# runs in a cgroup of its own (systemd-run --scope), that cgroup's peak: every
# process in it together since the cgroup started.
# The Google OAuth client "Connect Google" signs in with is baked in from
# BACKPLANE_GOOGLE_CLIENT_ID and _SECRET: the environment's (CI secrets), else
# a git-ignored .env here or in the main checkout (src/server/effects/google.c).
# BACKPLANE_GOOGLE_BAKE=0 bakes none (test/tools/google_e2e.ts).
set -eu
# (a caller already inside `flock /tmp/bp-wt-build.lock ...` holds it)
held() {
  p=$$
  while [ -n "$p" ] && [ "$p" -gt 1 ]; do
    case $(tr '\0' ' ' < "/proc/$p/cmdline" 2>/dev/null) in
      *flock*bp-wt-build.lock*) return 0 ;;
    esac
    p=$(awk '{print $4}' "/proc/$p/stat" 2>/dev/null)
  done
  return 1
}
if [ -z "${BP_BUILD_LOCKED:-}" ] && command -v flock >/dev/null 2>&1 && ! held; then
  BP_BUILD_LOCKED=1 exec flock /tmp/bp-wt-build.lock "$0" "$@"
fi
src=$1
out=$2
cd "$(dirname "$0")/.."
low=${BACKPLANE_LOW_MEM:-0}
[ "$low" = 1 ] && jobs=${BACKPLANE_JOBS:-1} || jobs=${BACKPLANE_JOBS:-4}
# bend is a Bun program: JavaScriptCore sizes its heap to the machine's RAM
# and let the app's emit grow past 28 GB (systemd-oomd then killed whole
# desktop sessions). BUN_JSC_forceRAMSize tells it a smaller machine; it is
# a sizing hint, not a cap. The app's emit, measured (2026-10-15, bend
# 2.0.36, the same C byte for byte at each hint; peak RSS, wall):
#   12 GiB: 11.2 GB, 157 s   8 GiB: 8.8 GB, 186 s
#   6 GiB: 7.3 GB, ~250 s    5 GiB: 7.2 GB, 225 s    4 GiB: 7.1 GB, 228 s
# Below 8 GiB the peak barely moves, near 7 GB: a ~3 GB live heap plus
# what bend's whole-program passes leave between collections
# (bp-coord/build-memory). The emit and the compiles run one after the
# other; one clang at a time peaks near 3 GB.
[ "$low" = 1 ] && hint=4294967296 || hint=8589934592
export BUN_JSC_forceRAMSize=${BUN_JSC_forceRAMSize:-$hint}
cpus=${BACKPLANE_CPUS-0-3}
pin=""
if [ -n "$cpus" ] && command -v taskset >/dev/null 2>&1; then
  pin="taskset -c $cpus"
fi
cflags=${BACKPLANE_CFLAGS:--O3}
CC=${CC:-clang}
export CC
# The app's C is cut into 16 units whatever the jobs: clang needs about 2.5 GB
# for each (7 GB for each of 4 bigger ones), compile time is the same in
# total, and a unit whose text is unchanged is taken from the object cache.
units=${BACKPLANE_UNITS:-16}
BP_CC_ID=$("$CC" --version 2>&1 | head -n 2 | tr '\n' ' ')
BP_OBJ_CACHE=${BACKPLANE_OBJ_CACHE-${XDG_CACHE_HOME:-$HOME/.cache}/backplane-bend/obj}
[ "$BP_OBJ_CACHE" = 0 ] && BP_OBJ_CACHE=""
if [ -n "$BP_OBJ_CACHE" ] && [ -d "$BP_OBJ_CACHE" ]; then
  # objects not used for 30 days go
  find "$BP_OBJ_CACHE" -name '*.o' -mtime +30 -exec rm -f {} + 2>/dev/null || true
fi
work="build/cc/$(basename "$out")"
mkdir -p "$work"
# stage clocks (ms) and peak RSS (KB) via /usr/bin/time when it is there
now_ms() { echo $(( $(date +%s%N) / 1000000 )); }
rss_cmd() {
  f=$1; shift
  if [ "$(uname -s)" = Darwin ]; then scripts/mac-rss.sh "$f" "$@"
  elif [ -x /usr/bin/time ]; then /usr/bin/time -f %M -o "$f" "$@"
  else "$@"; fi
}
t_start=$(now_ms)
rm -f "$work/emit.rss"
# bend's own emit runs on every core; keep it polite too. Its C is the
# same on every platform (effects choose with #ifdef), so it may be made
# on another machine: BACKPLANE_EMIT_ONLY=1 stops after writing it, and
# BACKPLANE_PREBUILT=1 compiles the one already there
# the emit's key (see BACKPLANE_EMIT_CACHE): "" when it cannot be made
emit_key() {
  scripts/inputs-key.sh "emit:$src" scripts/build-app.sh scripts/cc-split.py scripts/inputs-key.sh 2>/dev/null || true
}
EMIT_CACHE=${BACKPLANE_EMIT_CACHE-${XDG_CACHE_HOME:-$HOME/.cache}/backplane-bend/emit}
[ "$EMIT_CACHE" = 0 ] && EMIT_CACHE=""
ekey=""
[ -n "$EMIT_CACHE" ] && [ "${BACKPLANE_PREBUILT:-}" != 1 ] && ekey=$(emit_key)
emit_how=fresh
if [ "${BACKPLANE_PREBUILT:-}" = 1 ] && [ -f "$work/all.c" ]; then
  echo "prebuilt $work/all.c"
  emit_how=prebuilt
elif [ -n "$ekey" ] && [ -f "$EMIT_CACHE/$ekey.c" ] && [ -f "$EMIT_CACHE/$ekey.ok" ] \
    && [ "$(cat "$EMIT_CACHE/$ekey.ok")" = "$(wc -c <"$EMIT_CACHE/$ekey.c" | tr -d ' ')" ]; then
  # a kept emit: whole (its .ok, written last, names its size)
  cp "$EMIT_CACHE/$ekey.c" "$work/all.tmp.c" && mv "$work/all.tmp.c" "$work/all.c"
  touch "$EMIT_CACHE/$ekey.c" "$EMIT_CACHE/$ekey.ok"
  emit_how=reused
else
  rm -f "$work/all.c"
  # (bend picks what it writes by the name's ending: .c is the C; any other
  # name would compile the whole program to a binary there)
  rss_cmd "$work/emit.rss" nice -n 19 $pin bend "$src" -o "$work/all.tmp.c" >/dev/null
  mv "$work/all.tmp.c" "$work/all.c"
  # kept only if nothing it read changed while it ran
  if [ -n "$ekey" ] && [ "$(emit_key)" != "$ekey" ]; then
    echo "build-app: the sources changed during the emit; it is not kept" >&2
    ekey=""
  fi
  if [ -n "$ekey" ]; then
    mkdir -p "$EMIT_CACHE"
    cp "$work/all.c" "$EMIT_CACHE/$ekey.c.$$" && mv "$EMIT_CACHE/$ekey.c.$$" "$EMIT_CACHE/$ekey.c" \
      && wc -c <"$EMIT_CACHE/$ekey.c" | tr -d ' ' >"$EMIT_CACHE/$ekey.ok.$$" && mv "$EMIT_CACHE/$ekey.ok.$$" "$EMIT_CACHE/$ekey.ok"
    # keep the three newest
    ls -t "$EMIT_CACHE"/*.c 2>/dev/null | tail -n +4 | while read -r old; do rm -f "$old" "${old%.c}.ok"; done
  fi
fi
t_emit=$(now_ms)
# the version (BACKPLANE_VERSION_STAMP) goes into the emitted C, not into
# the source: the kept emit stays the same for every version
if [ -n "${BACKPLANE_VERSION_STAMP:-}" ]; then
  python3 scripts/stamp-version.py "$work/all.c" "$BACKPLANE_VERSION_STAMP"
fi
[ "${BACKPLANE_EMIT_ONLY:-}" = 1 ] && { echo "$work/all.c"; exit 0; }
n=$(nice -n 19 python3 scripts/cc-split.py "$work/all.c" "$work" "$units")
rm -f "$work"/u*.o "$work"/u*.o.rss "$work"/u*.o.how
t_split=$(now_ms)
# the baked Google client, read from .env without running it
envval() {
  for f in .env "$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null)/../.env"; do
    [ -f "$f" ] || continue
    v=$(sed -n "s/^[[:space:]]*\(export[[:space:]]\{1,\}\)\{0,1\}$1=//p" "$f" | tail -n1 | tr -d "\"'\r")
    [ -n "$v" ] && { printf %s "$v"; return; }
  done
}
gid=${BACKPLANE_GOOGLE_CLIENT_ID:-$(envval BACKPLANE_GOOGLE_CLIENT_ID)}
gsec=${BACKPLANE_GOOGLE_CLIENT_SECRET:-$(envval BACKPLANE_GOOGLE_CLIENT_SECRET)}
rm -f "$work/bp_google.h"
if [ "${BACKPLANE_GOOGLE_BAKE:-1}" != 0 ] && [ -n "$gid" ] && [ -n "$gsec" ]; then
  case "$gid$gsec" in
    *[!A-Za-z0-9._-]*) echo "BACKPLANE_GOOGLE_CLIENT_ID/_SECRET: unexpected characters" >&2; exit 1 ;;
  esac
  printf '#define BP_GOOGLE_CLIENT_ID "%s"\n#define BP_GOOGLE_CLIENT_SECRET "%s"\n' "$gid" "$gsec" > "$work/bp_google.h"
  echo "baked Google client ${gid%%-*}-..."
fi
# the biggest units first, so the last ones to start are the small ones
export BP_CFLAGS="$cflags" BP_CC_ID BP_OBJ_CACHE
# the compiles' memory together: the RSS of every process this build
# started (its descendants: the compiles and their helpers) summed every
# half second while they run, the highest kept (a spike between two
# samples is missed)
me=$$
( peak=0; while :; do
    s=$(ps -eo pid=,ppid=,rss= 2>/dev/null | awk -v root="$me" '{ p[$1] = $2; r[$1] = $3 }
      END { for (k in p) { q = k; while (q in p && q != root && q > 1) q = p[q]; if (q == root) t += r[k] } print t + 0 }')
    [ "$s" -gt "$peak" ] && { peak=$s; echo "$peak" >"$work/cc.peak"; }
    sleep 0.5
  done ) &
sampler=$!
# (stopped however the build ends; it is this script's own child)
trap 'kill "$sampler" 2>/dev/null || true' EXIT
rm -f "$work/cc.peak"
ls -S "$work"/u*.c | nice -n 19 $pin xargs -P "$jobs" -I{} scripts/cc-unit.sh {}
kill "$sampler" 2>/dev/null || true
t_cc=$(now_ms)
# the libraries bend -o links: X11 and ALSA when an effect includes them
libs=""
grep -q '#include <X11/' "$work/all.c" && libs="$libs -lX11"
grep -q '#include <alsa/' "$work/all.c" && libs="$libs -lasound"
# The app opens X11 with dlopen. Keep libraries only when the program has
# real symbol references, so its server entry also runs without libX11.
# Other programs using Base's direct X11 calls still retain that library.
[ "$(uname -s)" != Linux ] || libs="-Wl,--as-needed $libs"
nice -n 19 $pin "$CC" $cflags "$work"/u*.o -o "$out" -lpthread -lm $libs
t_link=$(now_ms)
hits=$(cat "$work"/u*.o.how 2>/dev/null | grep -c '^hit$' || true)
misses=$(cat "$work"/u*.o.how 2>/dev/null | grep -c '^cc$' || true)
cc_max=$(cat "$work"/u*.o.rss 2>/dev/null | grep -E '^[0-9]+$' | sort -n | tail -n1)
emit_rss=$(grep -E '^[0-9]+$' "$work/emit.rss" 2>/dev/null | tail -n1)
cc_sum=$(cat "$work/cc.peak" 2>/dev/null || true)
cg=""
cgp=$(cut -d: -f3 /proc/self/cgroup 2>/dev/null | head -n1)
case $cgp in *.scope) [ -r "/sys/fs/cgroup$cgp/memory.peak" ] && cg=$(( $(cat "/sys/fs/cgroup$cgp/memory.peak") / 1024 )) ;; esac
sec() { awk -v a="$1" -v b="$2" 'BEGIN { printf "%.1f", (b - a) / 1000 }'; }
kb() { [ -n "$1" ] && awk -v k="$1" 'BEGIN { printf "%.2fG", k / 1048576 }' || printf -- -; }
line="stages: emit ($emit_how) $(sec "$t_start" "$t_emit")s split $(sec "$t_emit" "$t_split")s cc $(sec "$t_split" "$t_cc")s link $(sec "$t_cc" "$t_link")s total $(sec "$t_start" "$t_link")s | objects hit $hits miss $misses | peak RSS: emit $(kb "$emit_rss"), largest single cc $(kb "$cc_max"), this build's compiles together (sampled) $(kb "$cc_sum"), cgroup all-together $(kb "$cg")"
echo "$line" >"$work/stages.txt"
echo "$line"
echo "$out ($n units, $jobs jobs)"
