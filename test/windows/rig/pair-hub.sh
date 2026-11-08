#!/bin/bash
# pair-hub.sh HOST PORT: start a tailnet-reachable hub on a temp home on the Windows test machine HOST (pair-hub.ps1 in its desktop session),
# print its URL and copy its token here (./pair-hub-PORT.token). Stop: pair-hub.sh HOST PORT stop
set -euo pipefail
host=$1; port=$2; here=$(cd "$(dirname "$0")" && pwd); j=$(mktemp --suffix=.ps1)
if [ "${3:-}" = stop ]; then printf '& C:\\bp\\rig\\down.ps1 -Name pair-%s\n' "$port" > "$j"; else printf '& C:\\bp\\rig\\pair-hub.ps1 -Port %s\n' "$port" > "$j"; fi
scp -q "$here/pair-hub.ps1" "$here/up.ps1" "$here/down.ps1" "$host:C:/bp/rig/"
"$here/sess.sh" "$host" "$j" 120; rm -f "$j"
[ "${3:-}" = stop ] || scp -q "$host:C:/bp/runs/pair-$port/home/token" "./pair-hub-$port.token" && echo "token -> ./pair-hub-$port.token"
