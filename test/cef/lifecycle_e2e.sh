#!/bin/sh
# lifecycle_e2e.sh CLIENT_BIN HUB_BIN : the Linux browser checklist for the embedded Chromium, on a private Xvfb (no window manager) with a local hub and stand-in agents.
#   an https page loads (the engine's own title, over CDP)           a local file:// page loads
#   navigation, then back                                            real X typing reaches a field, real X wheel scrolls the page
#   the Browser tab closed and reopened 3 times: no helper grows     the page dropped (another thread) and made again 3 times: its helpers go with it
#   quitting the client leaves none of its CEF helpers behind (every pid checked is one this script saw as a descendant of the client; nothing is killed by name)
# usage: test/cef/lifecycle_e2e.sh CLIENT_BIN HUB_BIN  -> PASS/FAIL lines, exit 0 when all pass. Artifacts in $OUT (default $TMPDIR/cef-life). E2E_NO_NET=1: no https row.
set -eu
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../.." && pwd)
fp() { python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1])'; }
CLIENT=$1; HUB=$2; OUT=${OUT:-${TMPDIR:-/tmp}/cef-life}; DISP=${DISP:-:81}; HPORT=${HPORT:-$(fp)}; PPORT=${PPORT:-$(fp)}; CDP=${CDP:-$(fp)}
RT=${BACKPLANE_CEF:-$HOME/.cache/backplane/cef-runtime/154.0.34}; HTTPS=${E2E_HTTPS_URL:-https://example.com/}
ASSETS=${E2E_ASSETS:-$(dirname "$CLIENT")/assets}; [ -d "$ASSETS" ] || ASSETS=$ROOT/assets
ICD=${E2E_VK_ICD:-$(ls /usr/share/vulkan/icd.d/lvp_icd*.json 2>/dev/null | head -1)}
[ -n "${E2E_DISPLAY:-}" ] || [ ! -e /tmp/.X11-unix/X${DISP#:} ] || { echo "display $DISP in use"; exit 2; }
rm -rf "$OUT"; mkdir -p "$OUT/home" "$OUT/proj" "$OUT/bin" "$OUT/fh/.cache"
for n in claude codex grok muse; do printf '#!/bin/sh\nexit 1\n' > "$OUT/bin/$n"; chmod +x "$OUT/bin/$n"; done
printf '#!/bin/sh\nexit 1\n' > "$OUT/bin/backplane-browser"; chmod +x "$OUT/bin/backplane-browser"   # the hub's own helper is not under test here
python3 - "$OUT" <<'PY'
import json,sys,time
R=sys.argv[1]; N=int(time.time())
ev=[{"$":"ProjectCreated","id":"p1","title":"life","root":R+"/proj","at":N}]+[{"$":"ThreadCreated","id":t,"project":"p1","title":t,"env":"local","branch":"","worktree":"","provider":"claude","model":"","at":N+i} for i,t in enumerate(["t1","t2"])]
open(R+"/home/events.jsonl","w").write("".join(json.dumps(e,separators=(",",":"))+"\n" for e in ev))
PY
PIDS=""; cleanup() { for p in $PIDS; do kill $p 2>/dev/null || true; done; }; trap cleanup EXIT INT TERM
(cd "$HERE/page" && exec python3 -m http.server $PPORT --bind 127.0.0.1 > "$OUT/page.log" 2>&1) & PIDS="$PIDS $!"
if [ -n "${E2E_DISPLAY:-}" ]; then DISP=$E2E_DISPLAY; else XVB=${XVFB:-$(command -v Xvfb)}; $XVB $DISP -screen 0 1400x900x24 -nolisten tcp > "$OUT/xvfb.log" 2>&1 & PIDS="$PIDS $!"
  # a window manager gives the window the keyboard focus (as on a desktop); i3 as in cef_e2e.sh, when there is one
  if command -v i3 > /dev/null; then sleep 1; printf 'font pango:monospace 8\nnew_window none\ndefault_border none\nhide_edge_borders both\nipc-socket %s/i3.sock\n' "$OUT" > "$OUT/i3.conf"; mkdir -p -m 700 "$OUT/run"
    XDG_RUNTIME_DIR=$OUT/run DISPLAY=$DISP i3 -c "$OUT/i3.conf" > "$OUT/i3.log" 2>&1 & PIDS="$PIDS $!"; fi; fi   # E2E_DISPLAY: an X server already running (a headless sway's Xwayland). The window's size: it sits at 0,0 with no window manager
( cd "$OUT/proj" && BACKPLANE_AGENTS=$OUT/bin BACKPLANE_BROWSER=$OUT/bin/backplane-browser BACKPLANE_NO_UPDATE=1 HOME=$OUT/fh XDG_CACHE_HOME=$OUT/fh/.cache \
  exec "$HUB" --home "$OUT/home" --port $HPORT --host 127.0.0.1 --no-tailscale --foreground > "$OUT/hub.log" 2>&1 ) & PIDS="$PIDS $!"
n=0; until curl -s -m1 -o /dev/null http://127.0.0.1:$HPORT/hello; do sleep 0.2; n=$((n+1)); [ $n -gt 150 ] && { echo "FAIL hub did not start"; tail -5 "$OUT/hub.log"; exit 1; }; done
n=0; until [ -e /tmp/.X11-unix/X${DISP#:} ]; do sleep 0.2; n=$((n+1)); [ $n -gt 50 ] && { echo "FAIL Xvfb did not start"; cat "$OUT/xvfb.log"; exit 1; }; done
FILEURL="file://$HERE/page/scroll.html"; IDX="http://127.0.0.1:$PPORT/index.html"
# the script: each `mark` is where this shell looks (the waits after it give it the time)
cyc() { printf '{"click":"rail:Browser"},{"wait":2500},{"mark":"closed%s"},{"wait":2500},{"click":"rail:Browser"},{"wait":3500},{"cef":{"report":true}},{"mark":"reopened%s"},{"wait":2500},' "$1" "$1"; }
drop() { printf '{"select":1},{"wait":2500},{"mark":"dropped%s"},{"wait":2500},{"click":"rail:Browser"},{"wait":3000},{"select":0},{"wait":3500},{"cef":{"goto":"%s"}},{"wait":3000},{"cef":{"report":true}},{"mark":"made%s"},{"wait":2500},' "$1" "$IDX" "$1"; }
{ printf '[{"select":0},{"wait":1500},{"click":"rail:Browser"},{"wait":3500},'
  [ -n "${E2E_NO_NET:-}" ] || printf '{"cef":{"goto":"%s"}},{"wait":8000},{"cef":{"report":true}},{"mark":"https"},{"wait":1500},' "$HTTPS"
  printf '{"cef":{"goto":"%s"}},{"wait":3500},{"cef":{"report":true}},{"mark":"file"},{"wait":9000},' "$FILEURL"
  printf '{"cef":{"goto":"%s"}},{"wait":3500},{"cef":{"report":true}},{"mark":"nav"},{"cef":{"back":true}},{"wait":3000},{"cef":{"report":true}},{"mark":"back"},{"wait":2500},' "$IDX"
  cyc 1; cyc 2; cyc 3; drop 1; drop 2; drop 3
  printf '{"mark":"end"},{"wait":2500}]'; } > "$OUT/script.json"
DISPLAY=$DISP BACKPLANE_CEF=$RT LD_LIBRARY_PATH=$RT BACKPLANE_CEF_CDP_PORT=$CDP BACKPLANE_CEF_NO_GPU=1 BACKPLANE_CEF_PROFILE=$OUT/profile LP_NUM_THREADS=2 \
  BACKPLANE_GPUI_ASSETS=$ASSETS HOME=$OUT/fh XDG_CACHE_HOME=$OUT/fh/.cache env ${ICD:+VK_ICD_FILENAMES=$ICD} "$CLIENT" --url http://127.0.0.1:$HPORT --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 240 > "$OUT/client.log" 2>&1 & CP=$!
export DISPLAY=$DISP
ok=0; fail=0; check() { if [ "$2" = 1 ]; then echo "PASS $1"; ok=$((ok+1)); else echo "FAIL $1 ($3)"; fail=$((fail+1)); fi; }
waitmark() { n=0; until grep -q "\"name\":\"$1\",\"event\":\"mark\"" "$OUT/client.log" 2>/dev/null; do kill -0 $CP 2>/dev/null || return 1; sleep 0.3; n=$((n+1)); [ $n -gt ${2:-300} ] && return 1; done; return 0; }
report() { grep '"non_white_samples"' "$OUT/client.log" | tail -1; }   # the newest {"cef":{"report":true}}
field() { python3 -c 'import json,sys;l=sys.argv[1];r=json.loads(l[l.index("{"):]) if "{" in l else {};print(r.get(sys.argv[2],""))' "$1" "$2"; }
cdp_title() { curl -s -m3 http://127.0.0.1:$CDP/json/list | python3 -c "import json,sys;[print(t['title']) for t in json.load(sys.stdin) if t.get('type')=='page' and t.get('url','').startswith(sys.argv[1])]" "$1" 2>/dev/null | head -1; }
# every descendant of the client and its --type (CEF's helpers are this executable re-run with --type=renderer|gpu-process|zygote|utility...)
helpers() { python3 - "$CP" <<'PY'
import os,sys
root=int(sys.argv[1]); kids={}
for p in os.listdir('/proc'):
    if not p.isdigit(): continue
    try: pp=int(open('/proc/%s/stat'%p).read().rsplit(')',1)[1].split()[1]); kids.setdefault(pp,[]).append(int(p))
    except Exception: pass
out=[]; todo=list(kids.get(root,[]))
while todo:
    p=todo.pop(); todo+=kids.get(p,[])
    try: cl=open('/proc/%d/cmdline'%p,'rb').read().split(b'\0')
    except Exception: continue
    t=next((a.decode()[7:] for a in cl if a.startswith(b'--type=')),'')
    oz=next((a.decode()[17:] for a in cl if a.startswith(b'--ozone-platform=')),'')
    if t: out.append('%d %s%s'%(p,t,' ozone='+oz if oz else ''))
print('\n'.join(sorted(out)))
PY
}
count() { helpers | wc -l | tr -d ' '; }
renderers() { helpers | grep -c ' renderer' || true; }
ALL=$OUT/all-helpers.txt; : > "$ALL"; seen() { helpers >> "$ALL"; }
if [ -z "${E2E_NO_NET:-}" ]; then
  waitmark https 200 || { echo "FAIL no https mark"; tail -20 "$OUT/client.log"; exit 1; }; seen
  R=$(report); T=$(cdp_title "$HTTPS"); U=$(field "$R" url); E=$(grep -o '"load_error":"[^"]*"' "$OUT/client.log" | tail -1)
  check "an https page loads ($HTTPS: title '$T', painted samples $(field "$R" non_white_samples))" "$([ -n "$T" ] && case "$U" in https://*) echo 1;; *) echo 0;; esac)" "url '$U' $E"
fi
waitmark file 200 || { echo "FAIL no file mark"; tail -20 "$OUT/client.log"; exit 1; }; seen
R=$(report); VX=$(field "$R" view_x | cut -d. -f1); VY=$(field "$R" view_y | cut -d. -f1); T=$(cdp_title file://)
check "a local file:// page loads (title '$T')" "$(case "$T" in scroll:*) echo 1;; *) echo 0;; esac)" "$(field "$R" url)"
python3 "$HERE/xinput.py" click $((VX+260)) $((VY+76)); sleep 0.5; python3 "$HERE/xinput.py" type hello; sleep 1
T=$(cdp_title file://); AE=$(bun "$HERE/cdp_eval.mjs" $CDP 'document.activeElement&&document.activeElement.id' 2>/dev/null | grep scroll.html | cut -f2)
check "real X typing reaches the page's field (title '$T')" "$(case "$T" in *typed:hello*) echo 1;; *) echo 0;; esac)" "focused element $AE"
for i in 1 2 3 4 5; do python3 "$HERE/xinput.py" wheel $((VX+500)) $((VY+400)) -1; sleep 0.2; done; sleep 1
T=$(cdp_title file://); Y=$(echo "$T" | sed -n 's/^scroll:\([0-9]*\).*/\1/p')
check "real X wheel scrolls the page (title '$T')" "$([ "${Y:-0}" -gt 0 ] && echo 1 || echo 0)" "$T"
waitmark nav 200 || { echo "FAIL no nav mark"; exit 1; }; R=$(report); check "navigation to a second page" "$(case "$(field "$R" url)" in *index.html) echo 1;; *) echo 0;; esac)" "$(field "$R" url)"
waitmark back 200 || { echo "FAIL no back mark"; exit 1; }; R=$(report); U=$(field "$R" url)
check "back returns to the file:// page ($U)" "$(case "$U" in file://*scroll.html) echo 1;; *) echo 0;; esac)" "$U"
seen; N0=$(count); R0=$(renderers); helpers > "$OUT/helpers-open.txt"; echo "helpers with the page open: $N0 ($R0 renderers)"
check "the open page runs CEF helpers under the client (the count below means something)" "$([ "$N0" -gt 0 ] && echo 1 || echo 0)" "$N0"
for c in 1 2 3; do
  waitmark closed$c 200 || { echo "FAIL no closed$c mark"; break; }; seen
  waitmark reopened$c 200 || { echo "FAIL no reopened$c mark"; break; }; sleep 1; seen; N=$(count); R=$(report)
  check "close and reopen $c: the page is back ($(field "$R" url)), helpers $N (open: $N0)" "$([ "$N" -le "$N0" ] && [ "$(field "$R" non_white_samples)" -gt 0 ] 2>/dev/null && echo 1 || echo 0)" "$(helpers | tr '\n' ' ')"
done
for c in 1 2 3; do
  waitmark dropped$c 200 || { echo "FAIL no dropped$c mark"; break; }; sleep 1; seen; ND=$(renderers)
  waitmark made$c 200 || { echo "FAIL no made$c mark"; break; }; sleep 1; seen; N=$(count); RN=$(renderers); R=$(report)
  check "page dropped and made again $c: renderers $RN (open: $R0, dropped: $ND), helpers $N, page $(field "$R" url)" "$([ "$RN" -le "$R0" ] && [ "$N" -le "$N0" ] && case "$(field "$R" url)" in *index.html) echo 1;; *) echo 0;; esac)" "$(helpers | tr '\n' ' ')"
done
waitmark end 200 || echo "FAIL no end mark"
seen; sort -u "$ALL" | awk '{print $1}' | sort -u > "$OUT/helper-pids.txt"
# quit: the script's end quits the client (cx.quit, the path a closed window takes)
n=0; while kill -0 $CP 2>/dev/null; do sleep 0.5; n=$((n+1)); [ $n -gt 200 ] && break; done
kill -0 $CP 2>/dev/null && { check "the client quit" 0 "still running after its --exit-after"; kill $CP; } || { wait $CP; RC=$?; check "the client quit cleanly (exit $RC)" "$([ $RC = 0 ] && echo 1 || echo 0)" "$(grep -v GPUI_METRIC "$OUT/client.log" | tail -3)"; }
sleep 3; LEFT=""; for p in $(cat "$OUT/helper-pids.txt"); do if kill -0 "$p" 2>/dev/null && tr '\0' ' ' < /proc/$p/cmdline 2>/dev/null | grep -q -- "--type="; then LEFT="$LEFT $p"; fi; done
check "quitting left none of the $(wc -l < "$OUT/helper-pids.txt") CEF helpers it ran" "$([ -z "$LEFT" ] && echo 1 || echo 0)" "left:$LEFT $(for p in $LEFT; do tr '\0' ' ' < /proc/$p/cmdline | cut -c1-80; echo; done)"
for p in $LEFT; do kill "$p" 2>/dev/null || true; done   # (captured pids only)
echo "passed=$ok failed=$fail; artifacts $OUT"; [ $fail -eq 0 ]
