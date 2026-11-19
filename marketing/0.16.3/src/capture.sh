#!/bin/sh
# Real Backplane 0.16.3 UI for the release graphics, captured headless. Nothing here touches the person's display, home or chats:
#   - a private Xvfb display (default :93) with its own i3, software Vulkan (lavapipe)
#   - the released 0.16.3 build (APP: the unpacked linux-x64 archive of v0.16.3), a temp hub home, a temp HOME for KiCad
#   - demo projects: a copy of the RoyalBlue54L Feather KiCad demo (CERN-OHL-P, by Lord's Boards; it ships with Backplane KiCad), and an
#     empty folder that the demo turn fills with Altium files through Backplane's own altium_write
#   - a stand-in agent (the repo's test/tools/claude_standin.ts, no model runs) that answers with the demo chat in demo.json
# usage: capture.sh APP_DIR KICAD_RUNTIME_DIR REPO_AT_0.16.3 STEPS.json OUT_DIR
#   STEPS.json: the client's --script steps; each {"mark":"name"} is a screenshot OUT_DIR/name.png
set -eu
HERE=$(cd "$(dirname "$0")" && pwd); APP=$1; KRT=$2; REPO=$3; STEPS=$4; OUT=$5
PORT=${PORT:-3893}; DISP=${DISP:-:93}; SIZE=${SIZE:-1440x900}; XV=${XVFB:-$HOME/.local/xvfb-root/usr/bin/Xvfb}
[ "$DISP" != ":0" ] && [ "$DISP" != ":1" ] || { echo "never the person's display"; exit 2; }
[ ! -e "/tmp/.X11-unix/X${DISP#:}" ] || { echo "display $DISP is in use"; exit 2; }
R=${R:-$(mktemp -d "${BP_TMP:-$HOME/.cache/bp-tmp}/mkt163.XXXX")}; mkdir -p "$OUT" "$R/home" "$R/bin" "$R/fakehome/.cache" "$R/run" "$R/projects"; chmod 700 "$R/run"
BUN=$(command -v bun || echo "$HOME/.bun/bin/bun")
# the demo projects
F="$R/projects/feather"; A="$R/projects/sensor-node"
if [ ! -d "$F" ]; then
  mkdir -p "$F" "$A"; cp -r "$KRT/share/kicad/demos/royalblue54L_feather/." "$F/"; rm -rf "$F/img"
  printf '%s\n' '*.kicad_prl' 'fp-info-cache' '*-backups/' '.history/' '*.lck' '~*.lck' > "$F/.gitignore"
  printf '# Sensor node\n\nAn Altium Designer project.\n' > "$A/README.md"
  for d in "$F" "$A"; do git -C "$d" init -q -b main; git -C "$d" -c user.name=demo -c user.email=demo@example.invalid add -A; git -C "$d" -c user.name=demo -c user.email=demo@example.invalid commit -qm "Start"; done
  # the person's own uncommitted work, so a turn's snapshot has something to keep
  printf '\n## Bring-up notes\n\n- 3V3 rail measured at 3.31 V\n' >> "$F/README.md"
fi
# the stand-in agent, answering from demo.json: a message that contains an entry's "say" runs its "run" (the stand-in's own directives) and is answered with its "reply"
python3 - "$REPO/test/tools/claude_standin.ts" "$R/standin.ts" <<'PY'
import sys
s = open(sys.argv[1]).read()
a = "async function run(c: Cmd, firstOne: boolean) {\n"
b = '  const echo = /echo:([\\s\\S]*?):echo/.exec(c.text)?.[1]?.replace(/\\\\n/g, "\\n");\n'
assert a in s and b in s, "the stand-in changed: update capture.sh"
s = s.replace(a, a + '  const demo = (JSON.parse(require("fs").readFileSync(process.env.BP_DEMO!, "utf8")).chat as any[]).find((d) => c.text.includes(d.say));\n  if (demo) c = { uuid: c.uuid, text: typeof demo.run === "string" ? demo.run : demo.run ? "mcpseq:" + JSON.stringify([...(demo.locks ?? []).map((n: string) => ({ name: "lock_acquire", arguments: { name: n.replace("HASH", process.env.BP_DEMO_HASH!), note: "demo" } })), demo.run]) : "" };\n')
s = s.replace(b, '  const echo = demo ? demo.reply as string : undefined;\n')
c2 = "    say(full);\n    for (const a of agents)"
assert c2 in s, "the stand-in changed: update capture.sh"
s = s.replace(c2, "    if (demo) full = demo.reply;\n" + c2)
s = s.replace('say(`MCPCALL ${JSON.parse(mc![1]).name} ${got.slice(0, 600)}`)', 'if (!demo) say(`MCPCALL ${JSON.parse(mc![1]).name} ${got.slice(0, 600)}`)')
assert 'say("MCPSEQ done")' in s, "the stand-in changed: update capture.sh"
s = s.replace('say("MCPSEQ done")', 'if (!demo) say("MCPSEQ done")')
open(sys.argv[2], "w").write(s)
PY
cp "$REPO/test/tools/"*.ts "$R/" 2>/dev/null || true; python3 - "$R" "$REPO" <<'PY'
import sys, os, shutil
R, REPO = sys.argv[1:]
# the stand-in's own imports resolve beside it
for n in os.listdir(REPO + "/test/tools"):
    p = REPO + "/test/tools/" + n
    if os.path.isdir(p) and not os.path.exists(R + "/" + n): shutil.copytree(p, R + "/" + n)
PY
printf '#!/bin/sh\nexec %s %s "$@"\n' "$BUN" "$R/standin.ts" > "$R/bin/claude"; chmod +x "$R/bin/claude"
for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > "$R/bin/$n"; chmod +x "$R/bin/$n"; done
if [ ! -f "$R/home/events.jsonl" ]; then
  mkdir -p "$R/home/kicad"; ln -s "$KRT" "$R/home/kicad/$(basename "$KRT")"
  python3 - "$R" "$HERE/demo.json" "$KRT" <<'PY'
import json, sys, time
R, demo, krt = sys.argv[1:]; now = int(time.time()); d = json.load(open(demo))
ev = []
for p in d["projects"]:
    ev.append({"$": "ProjectCreated", "id": p["id"], "title": p["title"], "root": R + "/projects/" + p["folder"], "at": now})
    for t in p["threads"]:
        ev.append({"$": "ThreadCreated", "id": t["id"], "project": p["id"], "title": t["title"], "env": "local", "branch": "", "worktree": "", "provider": "claude", "model": "", "at": now})
open(R + "/home/events.jsonl", "w").write("".join(json.dumps(e, separators=(",", ":")) + "\n" for e in ev))
PY
fi
PIDS=; cleanup() { for p in $PIDS; do kill "$p" 2>/dev/null || true; done; }; trap cleanup EXIT INT TERM
unset I3SOCK DBUS_SESSION_BUS_ADDRESS WAYLAND_DISPLAY
export XDG_RUNTIME_DIR="$R/run" LC_ALL=C BACKPLANE_AGENTS="$R/bin" BACKPLANE_NO_UPDATE=1 BP_STANDIN_MODE=${STANDIN_MODE:-stream} BP_STREAM_N=0 FAKE_LOG="$R/standin.log" BP_STANDIN_MCPCALL_MAX=4000 BP_DEMO="$HERE/demo.json" HOME="$R/fakehome" XDG_CACHE_HOME="$R/fakehome/.cache" XDG_CONFIG_HOME="$R/fakehome/.config" XDG_DATA_HOME="$R/fakehome/.local/share" PS1='$ ' SHELL=/bin/bash
export BP_DEMO_HASH=$(python3 "$REPO/test/e2e/kicad_main.py" lock "$A/Power.SchDoc" | sed 's/.*-//')
for f in .bashrc .bash_profile .profile; do printf "PS1='\\W \\$ '\n" > "$R/fakehome/$f"; done; cd "$F"
"$APP/backplane-hub" --home "$R/home" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground >> "$R/hub.log" 2>&1 & PIDS="$! $PIDS"; sleep 4
LD_LIBRARY_PATH="$(dirname "$(dirname "$XV")")/lib/x86_64-linux-gnu" "$XV" "$DISP" -screen 0 "${SIZE}x24" -nolisten tcp -xkbdir /usr/share/X11/xkb > "$R/xvfb.log" 2>&1 & PIDS="$! $PIDS"; sleep 2
printf 'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\nipc-socket %s/i3.sock\n' "$R" > "$R/i3.conf"
DISPLAY="$DISP" i3 -c "$R/i3.conf" > "$R/i3.log" 2>&1 & PIDS="$! $PIDS"; sleep 1
export DISPLAY="$DISP" VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json LP_NUM_THREADS=${LP_NUM_THREADS:-6} BACKPLANE_GPUI_ASSETS="$APP/assets" LIBGL_ALWAYS_SOFTWARE=1
[ -z "${DPI:-}" ] || echo "Xft.dpi: $DPI" | xrdb -merge
: > "$R/client.log"
"$APP/backplane-gpui" --url "http://127.0.0.1:$PORT" --token-file "$R/home/token" --script "$STEPS" --exit-after "${SECS:-240}" --theme dark >> "$R/client.log" 2>&1 & C=$!; PIDS="$C $PIDS"
MARKS=$(python3 -c "import json,sys; print(' '.join(s['mark'] for s in json.load(open(sys.argv[1])) if 'mark' in s))" "$STEPS")
seen=; END=$(( $(date +%s) + ${SECS:-240} ))
while kill -0 $C 2>/dev/null && [ "$(date +%s)" -lt "$END" ]; do
  for m in $MARKS; do case " $seen " in *" $m "*) continue;; esac; grep -q "\"name\":\"$m\"" "$R/client.log" 2>/dev/null && { sleep "${SETTLE:-1}"; import -window root "$OUT/$m.png"; seen="$seen $m"; echo "shot $m"; }; done
  [ "$(echo $seen | wc -w)" -lt "$(echo $MARKS | wc -w)" ] || break; sleep 0.5
done
echo "run folder: $R"
