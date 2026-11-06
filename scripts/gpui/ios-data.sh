#!/bin/sh
# The data/1 app's journeys on the simulator: against the stub hub (test/gpui/phone/stub-hub.ts, default) or a Rust hub binary (HUB=path to it).
#   scripts/gpui/ios-data.sh [JOURNEY...]   default: data-flow     env: HUB (a Rust hub binary)
# Start with work_run {lock: "mac-heavy"}. The hub runs on h@backplane (tailnet address, 3700M cap) in a throwaway folder that is deleted at the end.
cd "$(dirname "$0")/../.." || exit 1
B=$PWD/build/gpui; M=$B/mac-ssh; SB='ssh -o BatchMode:yes h@backplane'; SB='ssh -o BatchMode=yes h@backplane'; U=${SIM_UDID:-E3BC1B9E-AE4D-498C-97AF-B22FE897B41F}
V=/home/h/bp-data-e2e; HOSTIP=100.96.192.111; PORT=3860; HOSTDNS=backplane.tail9e0cf2.ts.net; OUT=$B/results/ios/e2e-data; mkdir -p $OUT
J=${*:-data-flow}
# REAL=1: a hub that is already up (HUB_PAIR_LINK=<invitation link>, HUB_PROJECT_PATH=<folder on that hub>, HUB_FIRST_PROMPT/EXPECT, HUB_ASK_PROMPT/EXPECT, HUB_REPLY_WAIT):
# nothing is started or stopped here, and nothing is printed that carries the invitation
if [ "${REAL:-}" = 1 ]; then
  RSYNC_RSH=$M rsync -a --delete test/gpui/e2e/ios/ hs-mac-mini:bp-gpui-ios/e2e/
  $M hs-mac-mini "cd ~/bp-gpui-ios/e2e && rm -rf ~/bp-gpui-ios/e2e-out && SIM_UDID=$U OUT=\$HOME/bp-gpui-ios/e2e-out HUB_HOST=${HUB_HOST:-100.64.84.41} HUB_PORT=${HUB_PORT:-3917} HUB_TOKEN_FILE=/dev/null HUB_PAIR_LINK='${HUB_PAIR_LINK:-}' HUB_ALREADY_PAIRED='${HUB_ALREADY_PAIRED:-}' HUB_PROJECT_PATH='$HUB_PROJECT_PATH' HUB_FIRST_PROMPT='${HUB_FIRST_PROMPT:-}' HUB_FIRST_EXPECT='${HUB_FIRST_EXPECT:-}' HUB_ASK_PROMPT='${HUB_ASK_PROMPT:-}' HUB_ASK_EXPECT='${HUB_ASK_EXPECT:-}' HUB_REPLY_WAIT='${HUB_REPLY_WAIT:-}' sh ./run.sh $J; xcrun simctl shutdown $U" > /dev/null 2>&1
  RSYNC_RSH=$M rsync -a hs-mac-mini:bp-gpui-ios/e2e-out/ $OUT/
  for j in $J; do [ -s "$OUT/$j.bpperf" ] && { echo "== $j: thread open timings"; python3 scripts/gpui/ios-open-table.py "$OUT/$j.bpperf"; }; done
python3 - <<PY
import json,glob
for f in sorted(glob.glob("$OUT/*.json")):
    if f.endswith("summary.json"): continue
    j=json.load(open(f)); st=j["metrics"].get("steps",[])
    print(j["journey"], j["status"], [s["do"] for s in st if s.get("ok") is False], [(s["do"][:34], s.get("result_ms")) for s in st])
PY
  exit 0
fi
$SB "rm -rf $V; mkdir -p $V/projects"
if [ -n "${HUB:-}" ]; then
  rsync -a "$HUB" h@backplane:bp-data-e2e/hub; rsync -a test/tools/claude_standin.ts h@backplane:bp-data-e2e/claude_standin.ts
  # (a hub started inside the ssh session keeps it open: start it, wait inside the same command, and let the command end)
  $SB "mkdir -p $V/home $V/bin; printf '#!/bin/sh\\nexec \$HOME/.bun/bin/bun $V/claude_standin.ts \"\\$@\"\\n' > $V/bin/claude; for n in codex grok muse; do printf '#!/bin/sh\\nexit 1\\n' > $V/bin/\$n; done; chmod +x $V/bin/*; cd $V; BACKPLANE_AGENTS=$V/bin BACKPLANE_NO_UPDATE=1 nohup systemd-run --user --scope -q -p MemoryMax=3700M -p MemorySwapMax=0 $V/hub --home $V/home --port $PORT --host $HOSTIP --no-tailscale --foreground > $V/hub.log 2>&1 < /dev/null & sleep 8; P=\$(ss -ltnp | grep ':$PORT ' | grep -o 'pid=[0-9]*' | head -1 | cut -d= -f2); echo hub pid \$P; tr '\\0' '\\n' < /proc/\$P/environ | grep BACKPLANE_AGENTS"
  TOKFILE=$V/home/token
else
  rsync -a test/gpui/phone/stub-hub.ts h@backplane:bp-data-e2e/stub-hub.ts
  $SB "head -c 16 /dev/urandom | od -An -tx1 | tr -d ' \n' > $V/token; cd $V; nohup \$HOME/.bun/bin/bun stub-hub.ts --port $PORT --host $HOSTIP --token-file $V/token --root $V/projects > $V/stub.log 2>&1 < /dev/null & sleep 3; echo stub started"
  TOKFILE=$V/token
fi
$SB "cat $TOKFILE" > /tmp/data-token
RSYNC_RSH=$M rsync -a /tmp/data-token hs-mac-mini:bp-gpui-ios/data-token
RSYNC_RSH=$M rsync -a --delete test/gpui/e2e/ios/ hs-mac-mini:bp-gpui-ios/e2e/
$M hs-mac-mini "cd ~/bp-gpui-ios/e2e && rm -rf ~/bp-gpui-ios/e2e-out && SIM_UDID=$U OUT=\$HOME/bp-gpui-ios/e2e-out HUB_HOST=$HOSTDNS HUB_PORT=$PORT HUB_TOKEN_FILE=\$HOME/bp-gpui-ios/data-token HUB_PROJECT_PATH=$V/projects/e2e1 sh ./run.sh $J; xcrun simctl shutdown $U"
RSYNC_RSH=$M rsync -a hs-mac-mini:bp-gpui-ios/e2e-out/ $OUT/
$SB "tail -5 $V/stub.log $V/hub.log 2>/dev/null | cut -c1-300" > $OUT/hub-log-tail.txt 2>&1
$SB "for p in \$(ss -ltnp | grep ':$PORT ' | grep -o 'pid=[0-9]*' | cut -d= -f2 | sort -u); do kill \$p; done; sleep 2; ss -ltn | grep -c ':$PORT ' ; rm -rf $V; ls -d $V 2>&1 | tail -1"
python3 - <<PY
import json,glob
for f in sorted(glob.glob("$OUT/*.json")):
    if f.endswith("summary.json"): continue
    j=json.load(open(f)); st=j["metrics"].get("steps",[])
    print(j["journey"], j["status"], [s["do"] for s in st if s.get("ok") is False], [(s["do"][:34], s.get("result_ms")) for s in st])
PY
