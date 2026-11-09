# Source this: functions that drive the sidebar rig with real X input.
R=$HOME/.cache/bp-tmp/give; . $R/env; export DISPLAY=$DISP; XI=$(git rev-parse --show-toplevel 2>/dev/null)/test/cef/xinput.py; [ -f "$XI" ] || XI=${WT:-.}/test/cef/xinput.py
SH=${SH:-$(git rev-parse --show-toplevel 2>/dev/null)/build/gpui/shots/give}; mkdir -p $SH
# the centre of a control's rect in the latest controls metric (exit 1 when absent)
at() { python3 -I -c '
import json,sys
last=None
for l in open(sys.argv[1]):
    if "GPUI_METRIC" in l and "\"event\":\"controls\"" in l: last=l
c=json.loads(last.split("GPUI_METRIC ",1)[1])["controls"]
for x in c:
    if x["id"]==sys.argv[2] and x.get("rect"):
        r=x["rect"]; print(int(r["x"]+min(r["w"]/2, 60)), int(r["y"]+r["h"]/2)); sys.exit(0)
sys.exit(1)' $R/client.log "$1"; }
ids() { python3 -I -c '
import json,sys,re
last=None
for l in open(sys.argv[1]):
    if "GPUI_METRIC" in l and "\"event\":\"controls\"" in l: last=l
for x in json.loads(last.split("GPUI_METRIC ",1)[1])["controls"]:
    if re.search(sys.argv[2], x["id"]): print(x["id"], "|", x["label"], "|", x.get("rect"))' $R/client.log "${1:-.}"; }
click() { local p; python3 -I $XI move 1430 $((880 + RANDOM % 8)); sleep 0.5; p=$(at "$1") || { echo "no control $1"; return 1; }; python3 -I $XI click ${=p} ${2:-1}; sleep ${3:-1.2}; }
xy() { python3 -I $XI click "$@"; sleep 1.2; }
keys() { python3 -I $XI type "$1"; sleep 0.8; }
key() { python3 -I $XI key "$1"; sleep 0.8; }
shot() { import -window root $SH/$1.png; echo "$SH/$1.png"; }
ev() { grep -E "$1" $R/home/events.jsonl | tail -${2:-5} | cut -c1-260; }
drag() { local a b; a=$(at "$1") || return 1; b=$(at "$2") || return 1; python3 -I $XI drag ${=a} ${=b}; sleep 1.5; }
wheel() { python3 -I $XI wheel "$@"; sleep 0.4; }
