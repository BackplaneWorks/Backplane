#!/bin/sh
# Settings › Accounts, screenshot on a temp home (#1337): the Rust hub with stand-in agents only (agy answers `-p /quota` as agy 1.5 does, grok answers ACP
# `_x.ai/billing` as Grok Build does, pi reports nothing), loopback, the client in a private Xvfb (nothing maps on a real display). The hub's own usage reader
# reads the stand-ins after its start delay; the shot is taken once the page shows them. One shot per theme: OUTDIR/accounts-<theme>.png.
# usage: desktop-accounts-shot.sh CLIENT_BIN HUB_BIN ASSETS_DIR OUTDIR   env: GPUI_PORT (3847), GPUI_DISP (:69), THEMES ("dark light"), XVFB, VK_ICD, SHOT_AT (40)
set -eu
ME=$(cd "$(dirname "$0")/.." && pwd); CLIENT=$1; HUBBIN=$2; ASSETS=$3; OUT=$4; . "$ME/scripts/lib-bptmp.sh"; R=$(mktemp -d "$BP_TMP/accounts-shot.XXXX")
PORT=${GPUI_PORT:-3847}; DISP=${GPUI_DISP:-:69}; XV=${XVFB:-$HOME/.local/xvfb-root/usr/bin/Xvfb}
[ -x "$CLIENT" ] && [ -x "$HUBBIN" ] && [ -x "$XV" ] || { echo "need CLIENT_BIN, HUB_BIN and Xvfb"; exit 2; }
[ ! -e "/tmp/.X11-unix/X${DISP#:}" ] || { echo "display $DISP is in use (set GPUI_DISP)"; exit 2; }
mkdir -p "$R/home" "$R/proj" "$R/bin" "$R/fakehome/.cache" "$R/run" "$OUT"; chmod 700 "$R/run"
for n in claude codex muse; do printf '#!/bin/sh\nexit 1\n' > "$R/bin/$n"; chmod +x "$R/bin/$n"; done
python3 - "$R" <<'PY'
import json, os, sys, time
R = sys.argv[1]; now = int(time.time())
iso = lambda t: time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(t))
r5, r7 = iso(now + 2 * 3600 + 1200), iso(now + 5 * 86400)
def put(name, body):
    p = os.path.join(R, "bin", name); open(p, "w").write(body); os.chmod(p, 0o755)
put("agy", "#!/bin/sh\n[ \"$1\" = \"-p\" ] && [ \"$2\" = \"/quota\" ] || exit 9\nprintf '"
    f"Gemini Models\\tWeekly Limit Remaining\\t94%%\\t{r7}\\nGemini Models\\tFive Hour Limit Remaining\\t90%%\\t{r5}\\n"
    f"Claude and GPT models\\tWeekly Limit Remaining\\t72%%\\t{r7}\\nClaude and GPT models\\tFive Hour Limit Remaining\\t8%%\\t{r5}\\n'\n")
put("grok", "#!/usr/bin/env python3\nimport sys, json\nif sys.argv[1:] != ['agent', 'stdio']: sys.exit(9)\nfor line in sys.stdin:\n    q = json.loads(line)\n"
    "    r = {'protocolVersion': 1} if q['method'] == 'initialize' else {'config': {'creditUsagePercent': 19.0, 'currentPeriod': {'type': 'USAGE_PERIOD_TYPE_WEEKLY', 'end': '" + r7 + "'}}, 'subscription_tier': 'SuperGrok'}\n"
    "    print(json.dumps({'jsonrpc': '2.0', 'id': q['id'], 'result': r}), flush=True)\n")
put("pi", "#!/bin/sh\nexit 0\n")
ev = [{"$": "ProjectCreated", "id": "p1", "title": "demo", "root": R + "/proj", "at": now},
      {"$": "ThreadCreated", "id": "t1", "project": "p1", "title": "On Pi", "env": "local", "branch": "", "worktree": "", "provider": "pi", "model": "", "at": now},
      {"$": "TurnUsage", "thread": "t1", "provider": "pi", "model": "", "key": "k", "input": 41000, "cread": 0, "cwrite": 0, "output": 2600, "reasoning": 0, "cost": 0, "known": "o", "basis": "b", "at": now - 600},
      {"$": "TurnUsage", "thread": "t1", "provider": "pi", "model": "", "key": "k", "input": 52000, "cread": 0, "cwrite": 0, "output": 3100, "reasoning": 0, "cost": 0, "known": "o", "basis": "b", "at": now - 120},
      {"$": "SettingSet", "key": "allowance.seen.pi", "value": json.dumps({"scope": "pi", "reason": "429 Too Many Requests", "reset": now + 1500, "at": now - 100}), "at": now - 100}]
open(R + "/home/events.jsonl", "w").write("".join(json.dumps(e, separators=(",", ":")) + "\n" for e in ev))
PY
PIDS=; cleanup() { for p in $PIDS; do kill "$p" 2>/dev/null || true; done; }; trap cleanup EXIT INT TERM
unset I3SOCK DBUS_SESSION_BUS_ADDRESS DISPLAY WAYLAND_DISPLAY
export XDG_RUNTIME_DIR="$R/run" LC_ALL=C BACKPLANE_AGENTS="$R/bin" BACKPLANE_NO_UPDATE=1 HOME="$R/fakehome" XDG_CACHE_HOME="$R/fakehome/.cache" XDG_CONFIG_HOME="$R/fakehome/.config" XDG_DATA_HOME="$R/fakehome/.local/share"
cd "$R/proj"
"$HUBBIN" --home "$R/home" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground > "$R/hub.log" 2>&1 & H=$!; PIDS="$H"; sleep 4
tr '\0' '\n' < "/proc/$H/environ" | grep -q "^BACKPLANE_AGENTS=$R/bin\$" || { echo "the hub has no BACKPLANE_AGENTS: stopping"; exit 3; }
"$XV" "$DISP" -screen 0 1440x900x24 -nolisten tcp -extension GLX -xkbdir /usr/share/X11/xkb > "$R/xvfb.log" 2>&1 & V=$!; PIDS="$V $PIDS"; sleep 2
printf 'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\nbar {\n  mode invisible\n}\nipc-socket %s/i3.sock\n' "$R" > "$R/i3.conf"
DISPLAY="$DISP" i3 -c "$R/i3.conf" > "$R/wm.log" 2>&1 & I=$!; PIDS="$I $PIDS"; sleep 1
export DISPLAY="$DISP" VK_ICD_FILENAMES=${VK_ICD:-/usr/share/vulkan/icd.d/lvp_icd.json} LP_NUM_THREADS=4 BACKPLANE_GPUI_ASSETS="$ASSETS"
# the reader's first look is 20 s after the hub starts
for k in $(seq 1 60); do grep -q "provider usage: " "$R/hub.log" && break; sleep 1; done
grep "provider usage: " "$R/hub.log" || { echo "the hub read no provider usage"; tail -20 "$R/hub.log"; exit 4; }
for th in ${THEMES:-dark light}; do
  echo '[{"wait":2500},{"click":"ui:Accounts"},{"wait":60000}]' > "$R/script.json"
  "$CLIENT" --url "http://127.0.0.1:$PORT" --token-file "$R/home/token" --theme "$th" --script "$R/script.json" --exit-after 40 > "$R/client-$th.log" 2>&1 & C=$!
  sleep ${SHOT_AT:-12}; import -window root "$OUT/accounts-$th.png"; kill "$C" 2>/dev/null || true; wait "$C" 2>/dev/null || true
  echo "shot $OUT/accounts-$th.png"
done
[ -n "${KEEP:-}" ] && echo "kept $R" || rm -rf "$R"
