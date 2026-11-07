#!/bin/sh
# Several hubs in the GPUI client, for real: two temp-home hubs (ports 3811, 3812, loopback, no tailscale), a private X display, a private config.
# Pair A, add B from the sidebar picker, switch back to A, then to B again; judged from the client's log and the saved hubs.json.
# NOT YET RUN: needs a software Vulkan driver (lavapipe, lvp_icd.json) for a private X display; the machine it was written on had only hardware ICDs.
#   HUB=<backplane-hub> GPUI=<backplane-gpui> test/e2e/pairing/gpui_hubs.sh        (BP_TMP names the scratch folder)
set -u
HERE=$(cd "$(dirname "$0")" && pwd); . "$HERE/../../../scripts/lib-bptmp.sh"
W=$(mktemp -d "$BP_TMP/hubs-e2e.XXXXXX"); L=$W/logs; mkdir -p "$L" "$W/home" "$W/cfg/backplane-gpui"
PIDS=""; fails=0; passes=0
ok() { if [ "$2" = 0 ]; then echo "pass     $1"; passes=$((passes+1)); else echo "fail     $1"; fails=$((fails+1)); fi; }
cleanup() { for p in $PIDS; do kill "$p" 2>/dev/null; done; [ -n "${KEEP:-}" ] || rm -rf "$W"; }
trap cleanup EXIT
A=http://127.0.0.1:3811; B=http://127.0.0.1:3812
for n in a b; do
  mkdir -p "$W/hub-$n"; port=$([ $n = a ] && echo 3811 || echo 3812)
  "$HUB" --home "$W/hub-$n" --port $port --host 127.0.0.1 --no-tailscale --foreground >"$L/hub-$n.log" 2>&1 & PIDS="$PIDS $!"
done
Xvfb :97 -screen 0 1400x900x24 >"$L/xvfb.log" 2>&1 & PIDS="$PIDS $!"
rig() { PYTHONDONTWRITEBYTECODE=1 python3 "$HERE/rig.py" "$1" "$W/hub-$2/token" "${@:3}" 2>/dev/null; }
for n in a b; do for i in $(seq 1 100); do [ -f "$W/hub-$n/token" ] && rig "$([ $n = a ] && echo $A || echo $B)" $n wait && break; sleep 0.3; done; done
code() { rig "$1" "$2" invite "$W/inv.json" >/dev/null; python3 -I -c "import json;print(json.load(open('$W/inv.json'))['code'])"; }
client() { # NAME SECONDS STEPS
  printf '%s' "$3" > "$W/$1.steps.json"
  DISPLAY=:97 VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json MESA_VK_WSI_DEBUG=sw LP_NUM_THREADS=2 HOME="$W/home" BACKPLANE_GPUI_LOCAL="$W/cfg/backplane-gpui/local-settings.json" BACKPLANE_GPUI_SCRIPT="$W/$1.steps.json" "$GPUI" --exit-after "$2" >"$L/$1.out" 2>&1
}
H=$W/cfg/backplane-gpui/hubs.json
first() { python3 -I -c "import json;print(json.load(open('$H'))['hubs'][0]['key'])" 2>/dev/null; }
says() { grep -q "$2" "$L/$1.out"; echo $?; }
ca=$(code $A a); client one 14 "[{\"wait\":1500},{\"pair_fill\":[\"\",\"$A\",\"$ca\"]},{\"click\":\"pair:go\"},{\"wait\":2500}]"
ok "pair A: connected to A" "$(says one '"hub_in_use":"127.0.0.1:3811"')"; ok "pair A: A is in use" "$([ "$(first)" = 127.0.0.1:3811 ]; echo $?)"
cb=$(code $B b); client two 14 "[{\"wait\":3000},{\"click\":\"hub:menu\"},{\"click\":\"hub:add\"},{\"pair_fill\":[\"\",\"$B\",\"$cb\"]},{\"click\":\"pair:go\"},{\"wait\":2500}]"
ok "add B from the picker: connected to B" "$(says two '"hub_in_use":"127.0.0.1:3812"')"; ok "B is in use, A kept" "$([ "$(first)" = 127.0.0.1:3812 ] && grep -q 127.0.0.1:3811 "$H"; echo $?)"
ok "two credential files, owner-only" "$([ "$(ls "$W/cfg/backplane-gpui/hubs" | wc -l)" = 2 ] && [ "$(stat -c %a "$W"/cfg/backplane-gpui/hubs/*.token | sort -u)" = 600 ]; echo $?)"
client three 14 '[{"wait":3000},{"click":"hub:menu"},{"click":"hub:use:127.0.0.1:3811"},{"wait":2500}]'
ok "switch to A: connected to A again" "$(grep -c '"hub_in_use":"127.0.0.1:3811"' "$L/three.out" | grep -q . && [ "$(grep -c '"hub_in_use"' "$L/three.out")" -ge 2 ]; echo $?)"; ok "the choice persists (A first)" "$([ "$(first)" = 127.0.0.1:3811 ]; echo $?)"
client four 8 '[{"wait":3000}]'
ok "a new start opens on A" "$(says four '"hub_in_use":"127.0.0.1:3811"')"
echo "summary pass=$passes fail=$fails"; [ "$fails" = 0 ]
