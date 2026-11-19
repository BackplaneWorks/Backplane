#!/usr/bin/env bash
# Headless scorecard screenshot and step-check runner for the right rail and viewer pane.
# Usage: test/viewers/rail-shots.sh CLIENT_BIN HUB_BIN ASSETS_DIR OUTDIR
#
# Runs four isolated project kinds (full, schematic-only, mech-only, no-design) against a
# private Xvfb (:500+) + i3 + llvmpipe display, temp homes, and stand-in agents only.
# Captures every rail page (settled + ~300 ms early state + Split after Mech + scorecard tasks)
# externally via `import -window root` synchronized on `shot-ready` / `<name>.done`, then writes:
#   OUTDIR/<kind>/<name>.png and captures.jsonl
#   OUTDIR/contact-<kind>.png   (ImageMagick montage per kind)
#   OUTDIR/headers.png          (tight pane-header crop strip across all pages)
#   OUTDIR/rails.png            (tight rail crop strip across all pages)
#   OUTDIR/steps.json           (outcomes, presses vs budget, obstructions, scorecard checks)
set -euo pipefail

# Strict display isolation: nothing inherits host DISPLAY or WAYLAND_DISPLAY.
unset DISPLAY WAYLAND_DISPLAY

if [ "$#" -lt 4 ]; then
  echo "usage: $0 CLIENT_BIN HUB_BIN ASSETS_DIR OUTDIR" >&2
  exit 2
fi

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd "$SCRIPT_DIR/../.." && pwd)

CLIENT_BIN=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
HUB_BIN=$(cd "$(dirname "$2")" && pwd)/$(basename "$2")
ASSETS_DIR=$(cd "$3" && pwd)

# Refuse a non-empty OUTDIR so stale captures can never contaminate a run.
if [ -e "$4" ] && [ -n "$(ls -A "$4" 2>/dev/null)" ]; then
  echo "rail-shots: refusing non-empty OUTDIR: $4 (use a fresh directory)" >&2
  exit 2
fi
mkdir -p "$4"
OUTDIR=$(cd "$4" && pwd)

[ -x "$CLIENT_BIN" ] || { echo "rail-shots: CLIENT_BIN not executable: $CLIENT_BIN" >&2; exit 2; }
[ -x "$HUB_BIN" ]    || { echo "rail-shots: HUB_BIN not executable: $HUB_BIN" >&2; exit 2; }
[ -d "$ASSETS_DIR" ] || { echo "rail-shots: ASSETS_DIR not found: $ASSETS_DIR" >&2; exit 2; }

REAL_HOME="$HOME"

# shellcheck source=../../scripts/lib-bptmp.sh
. "$REPO_ROOT/scripts/lib-bptmp.sh"
# shellcheck source=../e2e/lib/own-group.sh
. "$REPO_ROOT/test/e2e/lib/own-group.sh"

RUN=$(mktemp -d "$BP_TMP/bp-rail-shots.XXXXXX")
mkdir -p "$RUN/bin" "$RUN/fakehome/.cache" "$RUN/fakehome/.config" "$RUN/fakehome/.local/share"
mkdir -p -m 700 "$RUN/run" "$RUN/tmp"

stop_group() {
  local pg=$1
  [ -n "$pg" ] || return 0
  kill -TERM -- "-$pg" 2>/dev/null || return 0
  for _ in $(seq 20); do
    kill -0 -- "-$pg" 2>/dev/null || return 0
    sleep 0.15
  done
  kill -KILL -- "-$pg" 2>/dev/null || true
}

cleanup() {
  local rc=$?
  set +e
  own_stop || true
  if [ "${RAIL_KEEP_RUN:-0}" != "1" ]; then
    rm -rf "$RUN" 2>/dev/null || true
  fi
  exit "$rc"
}
trap cleanup EXIT INT TERM

# Locate bun for the stand-in agent wrapper
BUN_BIN=$(command -v bun 2>/dev/null || true)
[ -n "$BUN_BIN" ] || { [ -x "$REAL_HOME/.bun/bin/bun" ] && BUN_BIN="$REAL_HOME/.bun/bin/bun"; }
[ -n "$BUN_BIN" ] || { echo "rail-shots: bun not found (needed for stand-in agent)" >&2; exit 2; }

# Stand-in agents only: never invoke a real model CLI
cat >"$RUN/bin/claude" <<EOF
#!/bin/sh
exec "$BUN_BIN" "$REPO_ROOT/test/tools/claude_standin.ts" "\$@"
EOF
chmod +x "$RUN/bin/claude"
for agent in codex grok muse opencode pi omp antigravity; do
  printf '#!/bin/sh\nexit 1\n' >"$RUN/bin/$agent"
  chmod +x "$RUN/bin/$agent"
done

# Locate KiCad CLI for Gerber generation and hub plotting
KICAD_CLI="${BACKPLANE_KICAD_CLI:-}"
if [ -z "$KICAD_CLI" ] || [ ! -x "$KICAD_CLI" ]; then
  for cand in \
    "$REAL_HOME/.backplane-gpui-fresh/home/kicad"/10.99.0-bp.*/bin/kicad-cli \
    "$REAL_HOME/.local/bin/kicad-cli" \
    "$(command -v kicad-cli 2>/dev/null || true)"; do
    if [ -n "$cand" ] && [ -x "$cand" ]; then
      KICAD_CLI="$cand"
      break
    fi
  done
fi

# Locate Xvfb (prefer ~/.local/xvfb-root with LD_LIBRARY_PATH)
XVFB_BIN=""
XVFB_LD=""
for cand in "${XVFB_ROOT:-}" "$REAL_HOME/.local/xvfb-root" "$REAL_HOME/.cache/xvfb-root"; do
  if [ -n "$cand" ] && [ -x "$cand/usr/bin/Xvfb" ]; then
    XVFB_BIN="$cand/usr/bin/Xvfb"
    XVFB_LD="$cand/usr/lib/x86_64-linux-gnu"
    break
  fi
done
if [ -z "$XVFB_BIN" ]; then
  XVFB_BIN=$(command -v Xvfb 2>/dev/null || true)
fi
[ -n "$XVFB_BIN" ] || { echo "rail-shots: Xvfb not found" >&2; exit 2; }

# Pick a free private display :500+
d=$((520 + $$ % 200))
while [ -e "/tmp/.X11-unix/X$d" ] || [ -e "/tmp/.X$d-lock" ]; do d=$((d + 1)); done
DISP=":$d"

own env LD_LIBRARY_PATH="$XVFB_LD" "$XVFB_BIN" "$DISP" -screen 0 1440x900x24 -nolisten tcp -xkbdir /usr/share/X11/xkb >"$RUN/xvfb.log" 2>&1
for _ in $(seq 40); do [ -e "/tmp/.X11-unix/X$d" ] && break; sleep 0.1; done
[ -e "/tmp/.X11-unix/X$d" ] || { echo "rail-shots: Xvfb failed to start on $DISP" >&2; cat "$RUN/xvfb.log" >&2; exit 1; }

cat >"$RUN/i3.conf" <<EOF
font pango:monospace 8
new_window none
for_window [class=".*"] border none
default_border none
hide_edge_borders both
ipc-socket $RUN/i3.sock
EOF
own env DISPLAY="$DISP" XDG_RUNTIME_DIR="$RUN/run" i3 -c "$RUN/i3.conf" >"$RUN/i3.log" 2>&1
for _ in $(seq 40); do [ -S "$RUN/i3.sock" ] && break; sleep 0.1; done

ICD=$(ls /usr/share/vulkan/icd.d/lvp_icd*.json 2>/dev/null | head -1 || true)

# Pick a free loopback port >= 3799
free_port() {
  python3 - <<'PY'
import os, socket
start = 3800 + (os.getpid() % 1000)
for p in range(start, 60000):
    s = socket.socket()
    s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    try:
        s.bind(("127.0.0.1", p))
        print(p)
        s.close()
        raise SystemExit(0)
    except OSError:
        s.close()
raise SystemExit(1)
PY
}

PLOT_FIX="$REPO_ROOT/test/files/fixtures/plot"
MESH_FIX="$REPO_ROOT/test/files/fixtures/mesh"

seed_project() {
  local kind=$1 dir=$2
  rm -rf "$dir"
  mkdir -p "$dir"
  printf '# %s fixture\n\nInitial text for rail-shots.\n' "$kind" >"$dir/README.md"

  case "$kind" in
    full)
      cp "$PLOT_FIX/ecc83-pp.kicad_pcb" "$PLOT_FIX/ecc83-pp.kicad_sch" "$PLOT_FIX/ecc83-pp.kicad_prl" "$dir/"
      printf '{}\n' >"$dir/ecc83-pp.kicad_pro"
      mkdir -p "$dir/mech" "$dir/mfg/gerbers"
      cp "$MESH_FIX/bracket.step" "$dir/mech/bracket.step"
      if [ -n "$KICAD_CLI" ] && [ -x "$KICAD_CLI" ]; then
        env -u DISPLAY -u WAYLAND_DISPLAY \
          HOME="$RUN/fakehome" XDG_CONFIG_HOME="$RUN/fakehome/.config" XDG_CACHE_HOME="$RUN/fakehome/.cache" TMPDIR="$RUN/tmp" \
          "$KICAD_CLI" pcb export gerbers --layers "F.Cu,B.Cu,F.SilkS,F.Mask,B.Mask,Edge.Cuts" -o "$dir/mfg/gerbers/" "$dir/ecc83-pp.kicad_pcb" >/dev/null 2>&1 || true
        env -u DISPLAY -u WAYLAND_DISPLAY \
          HOME="$RUN/fakehome" XDG_CONFIG_HOME="$RUN/fakehome/.config" XDG_CACHE_HOME="$RUN/fakehome/.cache" TMPDIR="$RUN/tmp" \
          "$KICAD_CLI" pcb export drill -o "$dir/mfg/gerbers/" "$dir/ecc83-pp.kicad_pcb" >/dev/null 2>&1 || true
      fi
      if [ -z "$(ls -A "$dir/mfg/gerbers" 2>/dev/null)" ]; then
        cat >"$dir/mfg/gerbers/ecc83-pp-F_Cu.gbr" <<'GBR'
%FSLAX46Y46*%
%MOMM*%
%ADD10C,0.250000*%
D10*
X0Y0D02*
X20000000Y0D01*
X20000000Y20000000D01*
M02*
GBR
        cat >"$dir/mfg/gerbers/ecc83-pp-Edge_Cuts.gbr" <<'GBR'
%FSLAX46Y46*%
%MOMM*%
%ADD10C,0.100000*%
D10*
X0Y0D02*
X30000000Y0D01*
X30000000Y20000000D01*
X0Y20000000D01*
X0Y0D01*
M02*
GBR
        cat >"$dir/mfg/gerbers/ecc83-pp-PTH.drl" <<'DRL'
M48
METRIC,TZ
T1C0.800
%
T1
X10.0Y10.0
M30
DRL
      fi
      ;;
    schematic-only)
      cp "$PLOT_FIX/ecc83-pp.kicad_sch" "$dir/"
      printf '{}\n' >"$dir/ecc83-pp.kicad_pro"
      ;;
    mech-only)
      mkdir -p "$dir/mech"
      cp "$MESH_FIX/bracket.step" "$dir/mech/bracket.step"
      ;;
    no-design)
      ;;
    *)
      echo "rail-shots: unknown kind: $kind" >&2
      exit 2
      ;;
  esac

  (
    cd "$dir"
    git init -q -b main
    git config user.name "Fixture"
    git config user.email "fixture@example.invalid"
    git add -A
    git commit -qm init
  )
}

# After the thread `t1` exists, write its turn/0 checkpoint and modify README.md so Diff has real changes.
seed_thread_change() {
  local dir=$1 thread_id=$2
  python3 - "$dir" "$thread_id" <<'PY'
import base64, os, subprocess, sys
d, tid = sys.argv[1:3]
g = lambda *a: subprocess.check_output(["git", *a], cwd=d, text=True).strip()
git_dir = g("rev-parse", "--absolute-git-dir")
idx = os.path.join(git_dir, "backplane-seed.index")
env = dict(os.environ, GIT_INDEX_FILE=idx)
if os.path.exists(idx):
    os.remove(idx)
subprocess.check_call(["git", "add", "-A"], cwd=d, env=env)
tree = subprocess.check_output(["git", "write-tree"], cwd=d, env=env, text=True).strip()
sha = subprocess.check_output(["git", "commit-tree", "-m", "checkpoint", tree], cwd=d, text=True).strip()
if os.path.exists(idx):
    os.remove(idx)
b64 = base64.urlsafe_b64encode(tid.encode()).decode().rstrip("=")
subprocess.check_call(["git", "update-ref", f"refs/backplane/checkpoints/{b64}/turn/0", sha], cwd=d)
with open(os.path.join(d, "README.md"), "a", encoding="utf-8") as f:
    f.write("\nChange made after thread creation for Diff check.\n")
PY
}

if [ -n "${RAIL_KINDS:-}" ]; then
  read -r -a KINDS <<<"$RAIL_KINDS"
else
  mapfile -t KINDS < <(python3 -c "import sys; sys.path.insert(0, '$SCRIPT_DIR'); import rail_shots_manifest as M; print('\n'.join(M.KINDS))")
fi

for kind in "${KINDS[@]}"; do
  echo "rail-shots: === kind=$kind ==="
  KIND_OUT="$OUTDIR/$kind"
  mkdir -p "$KIND_OUT"
  PROJ="$RUN/proj-$kind"
  HHOME="$RUN/home-$kind"
  SHOT_DIR="$RUN/shots-$kind"
  STOP_FLAG="$RUN/stop-$kind"
  rm -rf "$HHOME" "$SHOT_DIR" "$STOP_FLAG"
  mkdir -p "$HHOME" "$SHOT_DIR"

  seed_project "$kind" "$PROJ"

  python3 - "$HHOME" "$PROJ" "$kind" <<'PY'
import json, sys, time
hhome, proj, kind = sys.argv[1:4]
now = int(time.time())
ev = [
    {"$": "ProjectCreated", "id": "p1", "title": kind, "root": proj, "at": now},
    {"$": "ThreadCreated", "id": "t1", "project": "p1", "title": f"{kind} rail check", "env": "local", "branch": "", "worktree": "", "provider": "claude", "model": "", "at": now},
]
with open(f"{hhome}/events.jsonl", "w", encoding="utf-8") as f:
    for e in ev:
        f.write(json.dumps(e, separators=(",", ":")) + "\n")
PY

  # Make the fixture's git change AFTER the thread exists so Diff compares against turn/0.
  seed_thread_change "$PROJ" "t1"

  PORT=$(free_port)
  own env -u DISPLAY -u WAYLAND_DISPLAY \
    HOME="$RUN/fakehome" \
    XDG_CONFIG_HOME="$RUN/fakehome/.config" \
    XDG_CACHE_HOME="$RUN/fakehome/.cache" \
    XDG_DATA_HOME="$RUN/fakehome/.local/share" \
    XDG_RUNTIME_DIR="$RUN/run" \
    TMPDIR="$RUN/tmp" \
    LC_ALL=C \
    BACKPLANE_AGENTS="$RUN/bin" \
    BACKPLANE_NO_UPDATE=1 \
    BP_STANDIN_MODE=stream \
    BACKPLANE_KICAD_CLI="${KICAD_CLI:-}" \
    "$HUB_BIN" --home "$HHOME" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground >"$KIND_OUT/hub.log" 2>&1
  HUB_PG=$!

  for _ in $(seq 80); do
    curl -s -m 1 -o /dev/null "http://127.0.0.1:$PORT/hello" && break
    sleep 0.15
  done
  curl -s -m 2 -o /dev/null "http://127.0.0.1:$PORT/hello" || {
    echo "rail-shots: hub did not start for $kind (port $PORT)" >&2
    tail -n 20 "$KIND_OUT/hub.log" >&2 || true
    exit 1
  }
  tr '\0' '\n' <"/proc/$HUB_PG/environ" | grep -q "^BACKPLANE_AGENTS=$RUN/bin\$" || {
    echo "rail-shots: BACKPLANE_AGENTS missing on hub pid $HUB_PG" >&2
    exit 3
  }

  SCRIPT_JSON="$RUN/script-$kind.json"
  python3 "$SCRIPT_DIR/rail_shots_driver.py" gen-script "$kind" "$SCRIPT_JSON" "${RAIL_EARLY_MS:-300}" "${RAIL_SETTLE_TIMEOUT_MS:-15000}"

  # Start external capture watcher (only watcher's `import` and `xwininfo` receive DISPLAY=$DISP)
  own python3 "$SCRIPT_DIR/rail_shots_driver.py" watch "$DISP" "$KIND_OUT/client.log" "$SHOT_DIR" "$KIND_OUT" "$STOP_FLAG" >"$KIND_OUT/watch.log" 2>&1
  WATCH_PG=$!

  # Launch client in its own process group and check its real exit status
  own env \
    HOME="$RUN/fakehome" \
    XDG_CONFIG_HOME="$RUN/fakehome/.config" \
    XDG_CACHE_HOME="$RUN/fakehome/.cache" \
    XDG_DATA_HOME="$RUN/fakehome/.local/share" \
    XDG_RUNTIME_DIR="$RUN/run" \
    TMPDIR="$RUN/tmp" \
    LC_ALL=C \
    DISPLAY="$DISP" \
    LIBGL_ALWAYS_SOFTWARE=1 \
    GALLIUM_DRIVER=llvmpipe \
    LP_NUM_THREADS=2 \
    VK_ICD_FILENAMES="$ICD" \
    BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" \
    BACKPLANE_GPUI_RECTS=1 \
    BACKPLANE_GPUI_SHOT_DIR="$SHOT_DIR" \
    "$CLIENT_BIN" --url "http://127.0.0.1:$PORT" --token-file "$HHOME/token" --script "$SCRIPT_JSON" --exit-after 300 >"$KIND_OUT/client.log" 2>&1
  CLIENT_PG=$!

  set +e
  wait "$CLIENT_PG"
  CLIENT_RC=$?
  set -e
  printf '%d\n' "$CLIENT_RC" >"$KIND_OUT/client.exit"
  if [ "$CLIENT_RC" -ne 0 ]; then
    echo "rail-shots: client exited with $CLIENT_RC for kind=$kind" >&2
  fi

  touch "$STOP_FLAG"
  for _ in $(seq 30); do
    kill -0 -- "-$WATCH_PG" 2>/dev/null || break
    sleep 0.1
  done
  stop_group "$WATCH_PG"
  stop_group "$CLIENT_PG"
  stop_group "$HUB_PG"
done

python3 "$SCRIPT_DIR/rail_shots_post.py" "$OUTDIR" "${KINDS[@]}"
POST_RC=$?

echo "rail-shots: done (post_rc=$POST_RC) -> $OUTDIR"
for kind in "${KINDS[@]}"; do
  echo "  contact: $OUTDIR/contact-$kind.png"
done
echo "  headers: $OUTDIR/headers.png"
echo "  rails:   $OUTDIR/rails.png"
echo "  steps:   $OUTDIR/steps.json"
exit "$POST_RC"
