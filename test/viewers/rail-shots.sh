#!/usr/bin/env bash
# Headless scorecard screenshot and step-check runner for the right rail and viewer pane.
# Usage: test/viewers/rail-shots.sh CLIENT_BIN HUB_BIN ASSETS_DIR OUTDIR
#
# Runs four isolated project kinds (full, schematic-only, mech-only, no-design) against a
# private Xvfb (:500+) + i3 + llvmpipe display, temp homes, and stand-in agents only.
# Captures every rail page (settled + ~300 ms loading state + Split after Mech), then writes:
#   OUTDIR/<kind>/<page>.png and <page>-loading.png
#   OUTDIR/contact-<kind>.png   (ImageMagick montage per kind)
#   OUTDIR/headers.png          (tight pane-header crop strip across all pages)
#   OUTDIR/rails.png            (tight rail crop strip across all pages)
#   OUTDIR/steps.json           (presses, hub open/active_kind, and scorecard task checks)
set -euo pipefail

if [ "$#" -lt 4 ]; then
  echo "usage: $0 CLIENT_BIN HUB_BIN ASSETS_DIR OUTDIR" >&2
  exit 2
fi

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd "$SCRIPT_DIR/../.." && pwd)

CLIENT_BIN=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
HUB_BIN=$(cd "$(dirname "$2")" && pwd)/$(basename "$2")
ASSETS_DIR=$(cd "$3" && pwd)
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

HUB_PID=""
cleanup() {
  local rc=$?
  set +e
  if [ -n "$HUB_PID" ]; then
    kill -TERM "$HUB_PID" 2>/dev/null || true
    sleep 0.3
    kill -KILL "$HUB_PID" 2>/dev/null || true
  fi
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

free_port() {
  python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()'
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
        HOME="$RUN/fakehome" XDG_CONFIG_HOME="$RUN/fakehome/.config" XDG_CACHE_HOME="$RUN/fakehome/.cache" TMPDIR="$RUN/tmp" \
          "$KICAD_CLI" pcb export gerbers --layers "F.Cu,B.Cu,F.SilkS,F.Mask,B.Mask,Edge.Cuts" -o "$dir/mfg/gerbers/" "$dir/ecc83-pp.kicad_pcb" >/dev/null 2>&1 || true
        HOME="$RUN/fakehome" XDG_CONFIG_HOME="$RUN/fakehome/.config" XDG_CACHE_HOME="$RUN/fakehome/.cache" TMPDIR="$RUN/tmp" \
          "$KICAD_CLI" pcb export drill -o "$dir/mfg/gerbers/" "$dir/ecc83-pp.kicad_pcb" >/dev/null 2>&1 || true
      fi
      # Fallback minimal valid Gerber + drill if kicad-cli did not emit files
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
    git -c user.name=Test -c user.email=test@example.com add -A
    git -c user.name=Test -c user.email=test@example.com commit -qm init
    printf '\nUncommitted change for Diff page.\n' >>README.md
  )
}

write_script_json() {
  local kind=$1 kind_out=$2 script_path=$3
  python3 - "$kind_out" "$script_path" "${RAIL_LOAD_MS:-300}" "${RAIL_SETTLE_MS:-3000}" <<'PY'
import json, sys
from pathlib import Path

kind_out = Path(sys.argv[1])
script_path = Path(sys.argv[2])
load_ms = int(sys.argv[3])
settle_ms = int(sys.argv[4])
heavy_ms = max(settle_ms, 5500)

steps = [
    {"select": 0},
    {"wait": 1500},
    {"mark": "page:idle"},
    {"read": "pane"},
    {"shot": str(kind_out / "idle.png")},
]

pages = [
    ("board", "rail:Board", settle_ms),
    ("schematic", "rail:Schematic", settle_ms),
    ("3d", "rail:3D", heavy_ms),
    ("gerbers", "rail:Gerbers", settle_ms),
    ("split", "rail:Split", heavy_ms),
    ("mech", "rail:Mech", heavy_ms),
    ("split-after-mech", "rail:Split", heavy_ms),
    ("files", "rail:Files", settle_ms),
    ("diff", "rail:Diff", settle_ms),
    ("review", "rail:Review", settle_ms),
    ("browser", "rail:Browser", settle_ms),
]

for page, rail_id, wait_ms in pages:
    steps.extend([
        {"mark": f"press:{page}"},
        {"press": rail_id},
        {"wait": load_ms},
        {"mark": f"loading:{page}"},
        {"read": "pane"},
        {"shot": str(kind_out / f"{page}-loading.png")},
        {"wait": wait_ms},
        {"mark": f"settled:{page}"},
        {"read": "pane"},
        {"shot": str(kind_out / f"{page}.png")},
    ])

steps.extend([
    {"mark": "press:close"},
    {"press": "rail:Browser"},
    {"wait": 1200},
    {"mark": "settled:close"},
    {"read": "pane"},
])

script_path.write_text(json.dumps(steps, indent=2) + "\n", encoding="utf-8")
PY
}

KINDS=(${RAIL_KINDS:-full schematic-only mech-only no-design})

for kind in "${KINDS[@]}"; do
  echo "rail-shots: === kind=$kind ==="
  KIND_OUT="$OUTDIR/$kind"
  mkdir -p "$KIND_OUT"
  PROJ="$RUN/proj-$kind"
  HHOME="$RUN/home-$kind"
  rm -rf "$HHOME"
  mkdir -p "$HHOME"

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

  PORT=$(free_port)
  env -u DISPLAY -u WAYLAND_DISPLAY \
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
    "$HUB_BIN" --home "$HHOME" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground >"$KIND_OUT/hub.log" 2>&1 &
  HUB_PID=$!

  for _ in $(seq 80); do
    curl -s -m 1 -o /dev/null "http://127.0.0.1:$PORT/hello" && break
    sleep 0.15
  done
  curl -s -m 2 -o /dev/null "http://127.0.0.1:$PORT/hello" || {
    echo "rail-shots: hub did not start for $kind (port $PORT)" >&2
    tail -n 20 "$KIND_OUT/hub.log" >&2 || true
    exit 1
  }
  tr '\0' '\n' <"/proc/$HUB_PID/environ" | grep -q "^BACKPLANE_AGENTS=$RUN/bin\$" || {
    echo "rail-shots: BACKPLANE_AGENTS missing on hub pid $HUB_PID" >&2
    exit 3
  }

  SCRIPT_JSON="$RUN/script-$kind.json"
  write_script_json "$kind" "$KIND_OUT" "$SCRIPT_JSON"

  env \
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
    "$CLIENT_BIN" --url "http://127.0.0.1:$PORT" --token-file "$HHOME/token" --script "$SCRIPT_JSON" --exit-after 240 >"$KIND_OUT/client.log" 2>&1 || true

  kill -TERM "$HUB_PID" 2>/dev/null || true
  wait "$HUB_PID" 2>/dev/null || true
  HUB_PID=""
done

python3 "$SCRIPT_DIR/rail_shots_post.py" "$OUTDIR" "${KINDS[@]}"

echo "rail-shots: done -> $OUTDIR"
for kind in "${KINDS[@]}"; do
  echo "  contact: $OUTDIR/contact-$kind.png"
done
echo "  headers: $OUTDIR/headers.png"
echo "  rails:   $OUTDIR/rails.png"
echo "  steps:   $OUTDIR/steps.json"
