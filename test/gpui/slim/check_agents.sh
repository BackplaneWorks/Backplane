#!/bin/sh
# usage: check_agents.sh HUB_PID STANDIN_DIR  -- exit 1 unless the running process's own
# environment names exactly the stand-in folder (read from /proc, not from how it was started).
set -u
pid=$1; want=$2
got=$(tr '\0' '\n' < /proc/$pid/environ | sed -n 's/^BACKPLANE_AGENTS=//p')
[ -n "$got" ] && [ "$got" = "$want" ] && [ -x "$want/claude" ] && grep -q 'FAKE_LOG\|standin\|fake' "$want/claude" || { echo "REFUSED: pid $pid BACKPLANE_AGENTS='$got' (want '$want')" >&2; exit 1; }
echo "ok: pid $pid BACKPLANE_AGENTS=$got"
