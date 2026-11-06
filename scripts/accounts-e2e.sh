#!/bin/bash
# Accounts, end to end, with real input: the hub binary on a temp home (stand-in agents, no model, no --policy-bin), the GPUI client on a private Xvfb with its own runtime dir
# (nothing on the person's screen), and real XTest keys for all of it: open a thread, the account chord, a digit, Esc, a typed message, then a limit mid-turn and the switch.
# Pictures and results: $R/shots, $R/result.json. Checks: scripts/accounts-e2e.py, which drives and judges.
# usage: accounts-e2e.sh CLIENT_BIN HUB_BIN ASSETS_DIR     (binaries come from a build, never built here; ASSETS_DIR is a build's assets/, with icons.json: without it the tick on the current account does not draw)
set -eu
HERE=$(cd "$(dirname "$0")" && pwd); REPO=$(cd "$HERE/.." && pwd)
CLIENT=$1; HUBBIN=$2; ASSETS=$3
R=${ACCOUNTS_E2E_DIR:-/tmp/accounts-e2e-$$}; PORT=${ACCOUNTS_E2E_PORT:-3841}; DISP=${ACCOUNTS_E2E_DISPLAY:-:93}
X=${XVFB_ROOT:-$HOME/.local/xvfb-root}; XLIB=${XVFB_LIB:-/home/h/Documents/i2cjak/backplane-gpui/build/gpui/xvfb/usr/lib/x86_64-linux-gnu}
[ -x "$CLIENT" ] && [ -x "$HUBBIN" ] || { echo "need the client and hub binaries"; exit 2; }
[ ! -e "/tmp/.X11-unix/X${DISP#:}" ] || { echo "display $DISP is in use"; exit 2; }
if ss -ltn 2>/dev/null | grep -q ":$PORT "; then echo "port $PORT is in use"; exit 2; fi
rm -rf "$R"; mkdir -p "$R"/{home,proj,bin,rec,shots,run,fakehome/.cache}; chmod 700 "$R/run"
for cli in claude codex; do sed "s#@DIR@#$R/rec#g" "$REPO/test/agents/fixtures/failover-standin.py" > "$R/bin/$cli"; chmod +x "$R/bin/$cli"; done
for n in grok muse; do printf '#!/bin/sh\nexit 1\n' > "$R/bin/$n"; chmod +x "$R/bin/$n"; done
python3 - "$R" <<'PY'
import json, os, sys, time, uuid
R = sys.argv[1]; now = int(time.time())
ev = [{"$": "ProjectCreated", "id": "p1", "title": "Board", "root": R + "/proj", "at": now},
      {"$": "ThreadCreated", "id": "t1", "project": "p1", "title": "A board thread", "env": "local", "branch": "", "worktree": "", "provider": "claude", "model": "", "at": now}]
open(R + "/home/events.jsonl", "w").write("".join(json.dumps(e, separators=(",", ":")) + "\n" for e in ev))
root = R + "/home/providers/accounts-v1"; os.makedirs(root, mode=0o700); os.chmod(R + "/home/providers", 0o700)
ps = []
for label, email, used in [("Work", "w@x.com", 62.5), ("Personal", "p@x.com", 10.0)]:
    i, s = str(uuid.uuid4()), str(uuid.uuid4())
    d = f"{root}/{i}/{s}"; os.makedirs(d, mode=0o700); os.chmod(f"{root}/{i}", 0o700)
    json.dump({"email": email, "subscriptionType": "max", "accountUuid": email, "orgUuid": "o"}, open(d + "/login.json", "w"))
    json.dump({"five_hour": {"utilization": used, "resets_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(now + 7200))}}, open(f"{R}/rec/usage.{email}", "w"))
    ps.append({"id": i, "provider": "claude", "label": label, "epoch": 2, "fingerprint": "", "store": s})
f = root + "/profiles.json"; json.dump({"version": 1, "profiles": ps}, open(f, "w")); os.chmod(f, 0o600)
PY
pids=()
cleanup() { for p in "${pids[@]:-}"; do [ -n "$p" ] && kill "$p" 2>/dev/null || true; done; }
trap cleanup EXIT INT TERM
export LC_ALL=C BACKPLANE_AGENTS=$R/bin BACKPLANE_NO_UPDATE=1 HOME=$R/fakehome XDG_CACHE_HOME=$R/fakehome/.cache XDG_CONFIG_HOME=$R/fakehome/.config XDG_DATA_HOME=$R/fakehome/.local XDG_RUNTIME_DIR=$R/run
unset I3SOCK DBUS_SESSION_BUS_ADDRESS BACKPLANE_AGENT_OWNER
cd "$R/proj"; git init -q .
"$HUBBIN" --home "$R/home" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground > "$R/hub.log" 2>&1 & HP=$!; pids+=("$HP")
for _ in $(seq 1 60); do curl -s -m1 -o /dev/null "http://127.0.0.1:$PORT/hello" && break; sleep 0.25; done
tr '\0' '\n' < "/proc/$HP/environ" | grep -q "^BACKPLANE_AGENTS=$R/bin\$" || { echo "NO BACKPLANE_AGENTS on the hub: stopping"; exit 3; }
LD_LIBRARY_PATH=$XLIB "$X/usr/bin/Xvfb" "$DISP" -screen 0 1280x800x24 -nolisten tcp -xkbdir /usr/share/X11/xkb > "$R/xvfb.log" 2>&1 & pids+=($!); sleep 1.5
printf 'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\n' > "$R/i3.conf"
DISPLAY=$DISP i3 -c "$R/i3.conf" > "$R/i3.log" 2>&1 & pids+=($!); sleep 1
export DISPLAY=$DISP VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json LP_NUM_THREADS=2 BACKPLANE_GPUI_ASSETS=$ASSETS
"$CLIENT" --url "http://127.0.0.1:$PORT" --token-file "$R/home/token" > "$R/client.log" 2>&1 & pids+=($!)
python3 "$HERE/accounts-e2e.py" "$R" "$DISP"
