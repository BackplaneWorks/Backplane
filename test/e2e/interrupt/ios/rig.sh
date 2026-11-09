#!/bin/bash
# iOS two-hub interrupt rig (Linux side). Units bpq-*. Hubs on loopback; A<->B only through relays RA1 (B->A) and RA2 (A->B) sharing cut file cutA;
# the Simulator reaches B only through relay RB on the tailnet address (cut file cutB). ctl.py on the tailnet: /cutA /healA /cutB /healB /send /helper.
#   rig.sh up | pair | down | hub A|B
set -u
RS=$(cd "$(dirname "$0")" && pwd); SRC=${SRC:-$(cd "$RS/../../../.." && pwd)}; . "$SRC/scripts/lib-bptmp.sh"; R=${R:-$BP_TMP/ios2r}
HUB=${HUB:-$SRC/native/host/target/fast/backplane-hub}; BUN=${BUN:-$(command -v bun || echo $HOME/.bun/bin/bun)}; IP=$(tailscale ip -4 | head -1)
PA=3981 PB=3982 RA1=3991 RA2=3996 RB=3993 CTL=3998
unit() { local n=$1; shift; systemctl --user stop bpq-$n 2>/dev/null; systemctl --user reset-failed bpq-$n 2>/dev/null; systemd-run --user --unit=bpq-$n -q --collect -p MemoryMax=4G "$@"; }
henv() { echo "-E BACKPLANE_AGENTS=$R/bin -E BACKPLANE_NO_UPDATE=1 -E BP_STANDIN_MODE=stream -E BP_STREAM_N=${N:-700} -E BP_STREAM_MS=150 -E FAKE_LOG=$R/standin-$1.log -E HOME=$R/fh$1 -E XDG_CACHE_HOME=$R/fh$1/.cache -E XDG_CONFIG_HOME=$R/fh$1/.config -E XDG_DATA_HOME=$R/fh$1/.local -E BACKPLANE_LOCK_DIR=$R -E BACKPLANE_NAME=rig-$1 -E BACKPLANE_PEERS=$([ $1 = A ] && echo http://127.0.0.1:$RA2 || echo http://127.0.0.1:$RA1) -E BACKPLANE_URL=$([ $1 = A ] && echo http://127.0.0.1:$RA1 || echo http://127.0.0.1:$RA2) -E LC_ALL=C -E TZ=Etc/UTC"; }
hub() { local w=$1 p; [ $w = A ] && p=$PA || p=$PB; unit hub$w $(henv $w) --working-directory=$R/proj$w $HUB --home $R/home$w --port $p --host 127.0.0.1 --no-tailscale --foreground; }
seed() { python3 - "$R" <<'PY'
import json,sys,time
R=sys.argv[1]; N=int(time.time())
base=lambda: [{"$":"SettingSet","key":k,"value":v,"at":N} for k,v in (("restart.continue","off"),("onboard.done","1"),("onboard.uber","1"))]
a=base()+[{"$":"ProjectCreated","id":"pa","title":"far-lamp","root":R+"/projA","at":N},{"$":"ThreadCreated","id":"ta","project":"pa","title":"Far work","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":N}]
b=base()+[{"$":"ProjectCreated","id":"pb","title":"lamp","root":R+"/projB","at":N},
  {"$":"ThreadCreated","id":"tb","project":"pb","title":"Local work","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":N},
  {"$":"ThreadCreated","id":"th","project":"pb","title":"Helper","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":N+1}]
for w,ev in (("A",a),("B",b)): open(f"{R}/home{w}/events.jsonl","w").write("".join(json.dumps(e,separators=(",",":"))+"\n" for e in ev))
PY
}
case "${1:-}" in
up)
  rm -rf $R; mkdir -p $R/homeA $R/homeB $R/projA $R/projB $R/bin $R/fhA/.cache $R/fhB/.cache
  for p in projA projB; do (cd $R/$p && git init -q -b main && echo hi > README.md && git add -A && git -c user.email=r@x -c user.name=rig commit -qm init); done
  printf '#!/bin/sh\npython3 -I -c %s "$@" >> %s 2>/dev/null\nexec %s %s "$@"\n' "'import json,sys;a=sys.argv[1:];print(json.loads(a[a.index(\"--mcp-config\")+1])[\"mcpServers\"][\"backplane\"][\"url\"])'" "$R/mcp.urls" $BUN $SRC/test/tools/claude_standin.ts > $R/bin/claude; chmod +x $R/bin/claude
  for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > $R/bin/$n; chmod +x $R/bin/$n; done
  seed; hub A; hub B
  unit relayA1 /usr/bin/python3 -I $SRC/scripts/mobile/tcp-relay.py 127.0.0.1 $RA1 127.0.0.1 $PA $R/cutA
  unit relayA2 /usr/bin/python3 -I $SRC/scripts/mobile/tcp-relay.py 127.0.0.1 $RA2 127.0.0.1 $PB $R/cutA
  unit relayB /usr/bin/python3 -I $SRC/scripts/mobile/tcp-relay.py $IP $RB 127.0.0.1 $PB $R/cutB
  unit ctl /usr/bin/python3 -I $RS/ctl.py $IP $CTL $R
  sleep 5; for p in $PA $PB; do curl -s -m3 -o /dev/null -w "hello $p %{http_code}\n" http://127.0.0.1:$p/hello; done
  curl -s -m3 -o /dev/null -w "hello relayB %{http_code}\n" http://$IP:$RB/hello ;;
pair) python3 -I $RS/pair.py $SRC/test/e2e/pairing $R ;;
hub) hub $2 ;;
down) for n in ctl relayB relayA1 relayA2 hubA hubB; do systemctl --user stop bpq-$n 2>/dev/null; done ;;
esac
