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
# a session bus of the run's own (dbus-run-session; it ends with the run): the login's user bus has no desktop portal here, and GTK's
# portal activation on it waits D-Bus's 25 s timeout before KiCad's API socket opens (bench-artifacts/handoff-prewarm: hpt063646 against
# hpt063830). The bus's identity is written from inside it, beside the suite's output.
# numpy for the suites' picture measures (require_numpy): the system Python here is externally managed and has no pip, so a venv made by
# hand beside the runs, ~/.cache/bp-kembed-remote/venv-numpy (never made by this script), goes on PYTHONPATH when it is there and matches
# python3's version; KEMBED_REMOTE_VENV=0 among the run's VAR=value withholds it (to see the guard fire). The hub is started without it.
VENV_ON=1; for a in "$@"; do [ "$a" = KEMBED_REMOTE_VENV=0 ] && VENV_ON=0; done
PYV=$(python3 -c 'import sys; print("python%d.%d" % sys.version_info[:2])' 2>/dev/null)
SP=$R/venv-numpy/lib/$PYV/site-packages
if [ "$VENV_ON" = 1 ] && [ -n "$PYV" ] && [ -d "$SP" ]; then
  export PYTHONPATH="$SP${PYTHONPATH:+:$PYTHONPATH}" KEMBED_PYTHONPATH_ADDED=1
  echo "remote_run: PYTHONPATH += $SP (numpy for the picture measures)" >&2
elif [ -d "$R/venv-numpy" ]; then
  echo "remote_run: venv-numpy not on PYTHONPATH (${PYV:-no python3}$([ "$VENV_ON" = 0 ] && echo ", withheld: KEMBED_REMOTE_VENV=0"))" >&2
fi
command -v dbus-run-session >/dev/null || { echo "remote_run: dbus-run-session is missing on $(hostname): no private session bus for the run" >&2; exit 70; }
BUSPROBE='{ echo "inside: DBUS_SESSION_BUS_ADDRESS=$DBUS_SESSION_BUS_ADDRESS"; echo "inside: bus id $(dbus-send --session --print-reply --dest=org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus.GetId 2>&1 | sed -n "s/^ *string \"\(.*\)\"/\1/p")"; echo "inside: portal owner $(dbus-send --session --print-reply --dest=org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus.NameHasOwner string:org.freedesktop.portal.Desktop 2>&1 | sed -n "s/^ *boolean //p")"; } > "$BP_KEMBED_BUS_OUT" 2>&1; exec "$@"'
export BP_KEMBED_BUS_OUT=$RUN/out/remote-bus.txt
PGID=
stop_run() { [ -n "$PGID" ] && { kill -TERM -- "-$PGID" 2>/dev/null; sleep 3; kill -KILL -- "-$PGID" 2>/dev/null; }; }
trap 'stop_run; exit 143' HUP TERM INT
setsid env "$@" dbus-run-session -- bash -c "$BUSPROBE" busprobe bash "$SUITE" "${ARGS[@]}" &
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
