#!/bin/bash
# Runs ON the remote venue (h@backplane), started by test/kicad_embed/remote.sh from the synced tree. Not for local use.
#   remote_run.sh RUN_ID LANE SUITE_REL [VAR=value ...]
set -uo pipefail
RID=$1; LANE=$2; SUITE=$3; shift 3
R=$HOME/.cache/bp-kembed-remote; TREE=$HOME/bp-gpui-build/$LANE; RUN=$R/r/$RID
case "$RUN" in *.backplane*) echo "remote_run: refusing a run dir under ~/.backplane*" >&2; exit 70 ;; esac
mkdir -p "$RUN/tmp" "$RUN/out" "$RUN/home"; chmod 700 "$RUN/tmp"
cd "$TREE" || exit 70
# the run's own HOME: nothing of the machine's real home (and never a ~/.backplane*) is read by the suites' discovery
export BP_KEMBED_RUN=$RID HOME=$RUN/home BP_TMP=$RUN/tmp TMPDIR=$RUN/tmp LC_ALL=C
export HUB_BIN=$TREE/.target/host/fast/backplane-hub CLIENT_BIN=$TREE/.target/gpui/fast/backplane-gpui ASSETS_DIR=$TREE/assets
[ -x "$HUB_BIN" ] && [ -x "$CLIENT_BIN" ] || { echo "remote_run: no hub/client build in $TREE/.target" >&2; exit 70; }
echo "remote_run: $(hostname) glibc $(ldd --version | head -1 | awk '{print $NF}') cores $(nproc) load $(cut -d' ' -f1 /proc/loadavg)" >&2
ARGS=()
case "$(basename "$SUITE")" in run_all.sh) ARGS=("$RUN/out") ;; *) export OUT=$RUN/out ;; esac
PGID=
stop_run() { [ -n "$PGID" ] && { kill -TERM -- "-$PGID" 2>/dev/null; sleep 3; kill -KILL -- "-$PGID" 2>/dev/null; }; }
trap 'stop_run; exit 143' HUP TERM INT
setsid env "$@" bash "$SUITE" "${ARGS[@]}" &
PGID=$!
wait "$PGID"; RC=$?
# nothing may outlive the run: every process of it carries BP_KEMBED_RUN=<id> in its environment
export -n BP_KEMBED_RUN; ME=$$; left=
for _ in $(seq 20); do
  left=
  for e in /proc/[0-9]*/environ; do
    p=${e#/proc/}; p=${p%/environ}
    [ "$p" = "$ME" ] && continue
    tr '\0' '\n' <"$e" 2>/dev/null | grep -qx "BP_KEMBED_RUN=$RID" && left="$left $p"
  done
  [ -z "$left" ] && break
  sleep 1
done
if [ -n "$left" ]; then
  echo "remote_run: FAIL processes left behind by run $RID:$left" >&2
  for p in $left; do tr '\0' ' ' </proc/$p/cmdline 2>/dev/null | cut -c1-200 >&2; echo >&2; kill -KILL "$p" 2>/dev/null; done
  [ "$RC" = 0 ] && RC=97
else
  echo "remote_run: no processes left behind (run $RID)" >&2
fi
exit "$RC"
