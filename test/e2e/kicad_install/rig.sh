#!/bin/bash
# Install KiCad in the real GPUI window on Xvfb: Settings > CAD's row, its consent step, Cancel, a failed run, Try again, progress and the result.
# A stand-in installer (standin.sh) and BACKPLANE_KICAD_NO_SYSTEM=1, temp HOME: the rig's real KiCad is never looked at, removed or installed.
set -u
D=$(cd "$(dirname "$0")" && pwd); WT=$(cd "$D/../../.." && pwd); MAIN=${XVFB_TREE:-/home/h/Documents/i2cjak/backplane-gpui}
HUBBIN=$WT/native/host/target/fast/backplane-hub; CLIENT=$WT/native/gpui/target/fast/backplane-gpui; ASSETS=$WT/native/gpui/assets
K=${K:-$HOME/.cache/bp-tmp/rig-kinst}; R=$K/run; SH=$K/shots; X=$MAIN/build/gpui/xvfb
DN=${DN:-$(for d in $(seq 151 199); do [ -e /tmp/.X11-unix/X$d ] || [ -e /tmp/.X$d-lock ] || { echo $d; break; }; done)}; DISP=:$DN; PORT=$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1])')
rm -rf $R $SH; mkdir -p $R/home $R/proj $R/bin $R/fakehome/.cache $SH
echo '{}' > $R/proj/demo.kicad_pro; (cd $R/proj && git init -q -b main && git add -A && git -c user.email=r@x -c user.name=rig commit -qm board)
for n in claude codex grok muse; do printf '#!/bin/sh\nexit 1\n' > $R/bin/$n; chmod +x $R/bin/$n; done
python3 - "$R" <<'PY'
import json,sys,time
R=sys.argv[1]; N=int(time.time())
ev=[{"$":"SettingSet","key":"restart.continue","value":"off","at":N},{"$":"SettingSet","key":"onboard.done","value":"1","at":N},{"$":"SettingSet","key":"onboard.uber","value":"1","at":N},{"$":"ProjectCreated","id":"p1","title":"demo","root":R+"/proj","at":N},
 {"$":"ThreadCreated","id":"t1","project":"p1","title":"Board work","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":N}]
open(R+"/home/events.jsonl","w").write("".join(json.dumps(e,separators=(",",":"))+"\n" for e in ev))
st=[{"wait":4000},{"click":"ui:Settings"},{"wait":2500},{"click":"set-nav:cad"},{"wait":3000},{"read":"settings"},{"mark":"s-row"},{"wait":2500},
 {"click":"set:kicad-install-ask"},{"wait":2000},{"read":"settings"},{"mark":"s-ask"},{"wait":2500},
 {"click":"set:kicad-install-cancel"},{"wait":2000},{"read":"settings"},{"mark":"s-cancel"},{"wait":2500},
 {"click":"set:kicad-install-ask"},{"wait":1500},{"click":"set:kicad-install"},{"wait":2500},{"read":"settings"},{"mark":"s-running"},{"wait":9000},{"read":"settings"},{"mark":"s-failed"},{"wait":2500},
 {"click":"set:kicad-install-ask"},{"wait":1500},{"click":"set:kicad-install"},{"wait":8000},{"read":"settings"},{"mark":"s-running2"},{"wait":8000},{"read":"settings"},{"mark":"s-done"},{"wait":2500}]
json.dump(st,open(R+"/script.json","w"))
PY
export LC_ALL=C R X HUBBIN CLIENT ASSETS PORT DISP SH K D
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 -E R -E X -E HUBBIN -E CLIENT -E ASSETS -E PORT -E DISP -E SH -E K -E D -- nice -n 10 bash -x $D/inner.sh > $R/driver.log 2>&1; grep -v "^+" $R/driver.log
echo "--- hub kicad lines"; grep -E "kicad: (install|Install)" $R/hub.log
echo "--- runs: $(cat $R/runs 2>/dev/null)"; ls $R/home/kicad 2>&1; echo "--- install log"; cat $R/home/kicad/.install.log
