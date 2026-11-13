#!/bin/sh
# Streamer mode, seen: the Rust hub on a temp home seeded with identifying text (an account with an email label, a thread naming an email, this
# machine's name, a tailnet address, a home path, a phone number, a pairing code), stand-in agents only, loopback, the client in a private Xvfb with
# its own i3 (nothing on a real display). The client runs twice, streamer mode off then on, and each run shoots the thread, the account menu,
# Settings (This device) and Settings → Accounts. usage: desktop-streamer-shot.sh CLIENT_BIN HUB_BIN ASSETS_DIR OUTDIR
#   env: GPUI_PORT (3847), GPUI_DISP (:69), XVFB (default ~/.local/xvfb-root/usr/bin/Xvfb), VK_ICD (default lavapipe's)
set -eu
ME=$(cd "$(dirname "$0")/.." && pwd); CLIENT=$1; HUBBIN=$2; ASSETS=$3; OUT=$4; . "$ME/scripts/lib-bptmp.sh"; R=$(mktemp -d "$BP_TMP/streamer-shot.XXXX")
PORT=${GPUI_PORT:-3847}; DISP=${GPUI_DISP:-:69}; XV=${XVFB:-$HOME/.local/xvfb-root/usr/bin/Xvfb}
[ -x "$CLIENT" ] && [ -x "$HUBBIN" ] && [ -x "$XV" ] || { echo "need CLIENT_BIN, HUB_BIN and Xvfb"; exit 2; }
[ "$DISP" != ":0" ] && [ ! -e "/tmp/.X11-unix/X${DISP#:}" ] || { echo "display $DISP is in use (set GPUI_DISP)"; exit 2; }
mkdir -p "$R/home" "$R/proj" "$R/bin" "$R/fakehome/.cache" "$R/run" "$OUT"; chmod 700 "$R/run"
printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun)" "$ME/test/tools/claude_standin.ts" > "$R/bin/claude"; chmod +x "$R/bin/claude"
for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > "$R/bin/$n"; chmod +x "$R/bin/$n"; done
HOST=$(hostname | cut -d. -f1)
python3 - "$R" "$HOST" <<'PY'
import json, sys, time
R, host = sys.argv[1], sys.argv[2]; now = int(time.time())
inv = {"version": 1, "revision": 1, "profiles": [{"id": "w", "provider": "claude", "label": "helmut-max", "state": "connected", "epoch": 1, "identity": {"email": "helmut.rig@example.com", "plan": "max"}}]}
user = f"Check the board on {host} (100.64.84.41:3799, {host}.tail9f3a.ts.net). Mail helmut.rig@example.com or call +1 415 555 0123."
agent = (f"Done. Saved `{R}/proj/main.kicad_pcb` as **helmut-max** on _{host}_.\n\n- home: `/home/someone/.config/backplane`\n- Short code K7QP2MXA, for ten minutes\n"
         f"- key: sk-ant-api03-AbCdEfGhIjKlMnOpQrSt\n\n```\nh@{host}:~$ ssh build@100.64.84.41\n```")
ev = [{"$": "ProjectCreated", "id": "p1", "title": "streamer", "root": R + "/proj", "at": now},
      {"$": "ThreadCreated", "id": "t1", "project": "p1", "title": f"board on {host}", "env": "local", "branch": "", "worktree": "", "provider": "claude", "model": "opus", "at": now},
      {"$": "SettingSet", "key": "accounts.inventory", "value": json.dumps(inv), "at": now},
      {"$": "MessagePosted", "thread": "t1", "msg": "m1", "role": "user", "text": user, "at": now + 1},
      {"$": "MessagePosted", "thread": "t1", "msg": "m2", "role": "assistant", "text": agent, "at": now + 2}]
import os
acc = R + "/home/providers/accounts-v1"; os.makedirs(acc, mode=0o700); os.chmod(R + "/home/providers", 0o700); os.chmod(acc, 0o700)
fd = os.open(acc + "/profiles.json", os.O_WRONLY | os.O_CREAT, 0o600)
os.write(fd, json.dumps({"version": 1, "profiles": [{"id": "0b1c2d3e-4f50-6172-8394-a5b6c7d8e9f0", "provider": "claude", "label": "helmut-max", "epoch": 1}]}).encode()); os.close(fd)
open(R + "/home/events.jsonl", "w").write("".join(json.dumps(e, separators=(",", ":")) + "\n" for e in ev))
steps = [{"select": 0}, {"wait": 2500}, {"mark": "thread"}, {"wait": 1200}, {"click": "ui:Account"}, {"wait": 2500}, {"mark": "picker"}, {"wait": 1200}, {"click": "ui:Account"}, {"wait": 800},
         {"click": "ui:Settings"}, {"wait": 2500}, {"mark": "settings"}, {"wait": 1200}, {"click": "ui:Accounts"}, {"wait": 2500}, {"mark": "accounts"}, {"wait": 1500}]
json.dump(steps, open(R + "/script.json", "w"))
PY
PIDS=; cleanup() { for p in $PIDS; do kill "$p" 2>/dev/null || true; done; }; trap cleanup EXIT INT TERM
unset I3SOCK DBUS_SESSION_BUS_ADDRESS WAYLAND_DISPLAY
export XDG_RUNTIME_DIR="$R/run" LC_ALL=C BACKPLANE_AGENTS="$R/bin" BACKPLANE_NO_UPDATE=1 BP_STANDIN_MODE=plain HOME="$R/fakehome" XDG_CACHE_HOME="$R/fakehome/.cache" XDG_CONFIG_HOME="$R/fakehome/.config" XDG_DATA_HOME="$R/fakehome/.local/share"
cd "$R/proj"
"$HUBBIN" --home "$R/home" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground > "$R/hub.log" 2>&1 & H=$!; PIDS="$H"; sleep 5
tr '\0' '\n' < "/proc/$H/environ" | grep -q "^BACKPLANE_AGENTS=$R/bin\$" || { echo "NO BACKPLANE_AGENTS on the hub"; exit 3; }
"$XV" "$DISP" -screen 0 1440x900x24 -nolisten tcp -xkbdir /usr/share/X11/xkb > "$R/xvfb.log" 2>&1 & V=$!; PIDS="$V $PIDS"; sleep 2
printf 'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\nipc-socket %s/i3.sock\n' "$R" > "$R/i3.conf"
DISPLAY="$DISP" i3 -c "$R/i3.conf" > "$R/i3.log" 2>&1 & I=$!; PIDS="$I $PIDS"; sleep 1
export DISPLAY="$DISP" VK_ICD_FILENAMES=${VK_ICD:-/usr/share/vulkan/icd.d/lvp_icd.json} LP_NUM_THREADS=2 BACKPLANE_GPUI_ASSETS="$ASSETS" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_METRICS=1
for mode in off on; do
  if [ $mode = on ]; then echo '{"streamer":true,"streamer_share":false}' > "$R/local.json"; else echo '{"streamer_share":false}' > "$R/local.json"; fi
  BACKPLANE_GPUI_LOCAL="$R/local.json" "$CLIENT" --url "http://127.0.0.1:$PORT" --token-file "$R/home/token" --script "$R/script.json" --exit-after 60 --theme dark > "$OUT/client-$mode.log" 2>&1 & C=$!
  for m in thread picker settings accounts; do
    for k in $(seq 1 90); do grep -q "\"name\":\"$m\"" "$OUT/client-$mode.log" && break; sleep 1; done
    sleep 0.5; import -window root "$OUT/$mode-$m.png"; echo "shot: $OUT/$mode-$m.png"
  done
  for k in $(seq 1 10); do kill -0 $C 2>/dev/null || break; sleep 1; done; kill $C 2>/dev/null || true; wait $C 2>/dev/null || true
done
cp "$R/hub.log" "$OUT/hub.log"; rm -rf "$R"
