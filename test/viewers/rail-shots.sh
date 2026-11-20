#!/usr/bin/env bash
# Headless scorecard screenshot and step-check runner for the right rail and viewer pane.
# Usage:
#   test/viewers/rail-shots.sh [--embed] [--share] CLIENT_BIN HUB_BIN ASSETS_DIR OUTDIR
#   test/viewers/rail-shots.sh --compare [--share] BASE_DIR NEW_DIR [OUT_DIR]
#
# Runs four isolated project kinds (full, schematic-only, mech-only, no-design) against a
# private Xvfb (:500+) + i3 + llvmpipe display, temp homes, neutral /var/tmp/bp-rail-<pid>
# fixture paths (carrying no OS user name), and stand-in agents only.
# Captures every rail page (settled + ~300 ms early state + Split after Mech + scorecard tasks)
# externally via `import -window root` synchronized on `shot-ready` / `<name>.done`, then writes:
#   OUTDIR/<kind>/<name>.png and captures.jsonl
#   OUTDIR/contact-<kind>.png   (ImageMagick montage per kind, excluding private shots)
#   OUTDIR/headers.png          (tight pane-header crop strip across all pages)
#   OUTDIR/rails.png            (tight rail crop strip across all pages)
#   OUTDIR/steps.json           (outcomes, rail states, privacy flags, presses vs budget, checks)
set -euo pipefail

# Strict display isolation: nothing inherits host DISPLAY or WAYLAND_DISPLAY.
unset DISPLAY WAYLAND_DISPLAY

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd "$SCRIPT_DIR/../.." && pwd)

EMBED_MODE_FLAG=0
SHARE_FLAG=0
B2=0
COMPARE_MODE=0
POS_ARGS=()
while [ "$#" -gt 0 ]; do
  case "$1" in
    --b2) B2=1; shift ;;
    --embed) EMBED_MODE_FLAG=1; shift ;;
    --share) SHARE_FLAG=1; shift ;;
    --compare) COMPARE_MODE=1; shift ;;
    -h|--help)
      echo "usage: $0 [--embed] [--share] CLIENT_BIN HUB_BIN ASSETS_DIR OUTDIR" >&2
      echo "       $0 --b2 CLIENT_BIN HUB_BIN ASSETS_DIR OUTDIR" >&2
      echo "       $0 --compare [--share] BASE_DIR NEW_DIR [OUT_DIR]" >&2
      exit 0
      ;;
    --) shift; POS_ARGS+=("$@"); break ;;
    -*) echo "rail-shots: unknown option: $1" >&2; exit 2 ;;
    *) POS_ARGS+=("$1"); shift ;;
  esac
done

if [ "$B2" = 1 ]; then
  [[ ${#POS_ARGS[@]} == 4 && $COMPARE_MODE$SHARE_FLAG$EMBED_MODE_FLAG == 000 ]] || exit 2
  exec python3 "$SCRIPT_DIR/rail_b2.py" "${POS_ARGS[@]}"
fi

if [ "$COMPARE_MODE" = "1" ]; then
  if [ "${#POS_ARGS[@]}" -lt 2 ] || [ "${#POS_ARGS[@]}" -gt 3 ]; then
    echo "usage: $0 --compare [--share] BASE_DIR NEW_DIR [OUT_DIR]" >&2
    exit 2
  fi
  CMP_ARGS=()
  [ "$SHARE_FLAG" = "1" ] && CMP_ARGS+=(--share)
  CMP_ARGS+=("${POS_ARGS[@]}")
  exec python3 "$SCRIPT_DIR/rail_shots_compare.py" "${CMP_ARGS[@]}"
fi

if [ "${#POS_ARGS[@]}" -lt 4 ]; then
  echo "usage: $0 [--embed] [--share] CLIENT_BIN HUB_BIN ASSETS_DIR OUTDIR" >&2
  exit 2
fi

CLIENT_BIN=$(cd "$(dirname "${POS_ARGS[0]}")" && pwd)/$(basename "${POS_ARGS[0]}")
HUB_BIN=$(cd "$(dirname "${POS_ARGS[1]}")" && pwd)/$(basename "${POS_ARGS[1]}")
ASSETS_DIR=$(cd "${POS_ARGS[2]}" && pwd)
RAW_OUTDIR="${POS_ARGS[3]}"

# Validate requested project kinds against the manifest BEFORE creating any directory or file.
if [ -n "${RAIL_KINDS:-}" ]; then
  read -r -a RAW_KINDS <<<"$RAIL_KINDS"
  KINDS_TEXT=$(python3 "$SCRIPT_DIR/rail_shots_manifest.py" validate "${RAW_KINDS[@]}") || exit 2
else
  KINDS_TEXT=$(python3 "$SCRIPT_DIR/rail_shots_manifest.py") || exit 2
fi
mapfile -t KINDS <<<"$KINDS_TEXT"

# Refuse a non-empty OUTDIR so stale captures can never contaminate a run.
if [ -e "$RAW_OUTDIR" ] && [ -n "$(ls -A "$RAW_OUTDIR" 2>/dev/null)" ]; then
  echo "rail-shots: refusing non-empty OUTDIR: $RAW_OUTDIR (use a fresh directory)" >&2
  exit 2
fi
mkdir -p "$RAW_OUTDIR"
OUTDIR=$(cd "$RAW_OUTDIR" && pwd)

[ -x "$CLIENT_BIN" ] || { echo "rail-shots: CLIENT_BIN not executable: $CLIENT_BIN" >&2; exit 2; }
[ -x "$HUB_BIN" ]    || { echo "rail-shots: HUB_BIN not executable: $HUB_BIN" >&2; exit 2; }
[ -d "$ASSETS_DIR" ] || { echo "rail-shots: ASSETS_DIR not found: $ASSETS_DIR" >&2; exit 2; }

REAL_HOME="$HOME"
export REAL_HOME

# shellcheck source=../../scripts/lib-bptmp.sh
. "$REPO_ROOT/scripts/lib-bptmp.sh"
# shellcheck source=../e2e/lib/own-group.sh
. "$REPO_ROOT/test/e2e/lib/own-group.sh"

RUN=""
NEUTRAL_DIR=""
NEUTRAL_RUN=""

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
  # Only remove the literal symlink `r` inside our own mktemp dir under /var/tmp/bp-rail.*
  if [ -n "$NEUTRAL_DIR" ] && [ -d "$NEUTRAL_DIR" ] && [[ "$NEUTRAL_DIR" == /var/tmp/bp-rail.* ]]; then
    [ -L "$NEUTRAL_DIR/r" ] && rm -f "$NEUTRAL_DIR/r" 2>/dev/null || true
    rmdir "$NEUTRAL_DIR" 2>/dev/null || true
  fi
  if [ "${RAIL_KEEP_RUN:-0}" != "1" ] && [ -n "$RUN" ] && [ -d "$RUN" ] && [[ "$RUN" == "$BP_TMP/bp-rail-shots."* ]]; then
    rm -rf "$RUN" 2>/dev/null || true
  fi
  exit "$rc"
}
# Install cleanup trap BEFORE creating any temp directory or /var/tmp alias.
trap cleanup EXIT INT TERM

RUN=$(mktemp -d "$BP_TMP/bp-rail-shots.XXXXXX")
mkdir -p "$RUN/bin" "$RUN/fakehome/.cache" "$RUN/fakehome/.config" "$RUN/fakehome/.local/share"
mkdir -p -m 700 "$RUN/run" "$RUN/tmp"

# Atomic mode-0700 neutral alias directory under /var/tmp so fixture paths carry no OS user name.
NEUTRAL_RUN="$RUN"
if [ -d /var/tmp ] && [ -w /var/tmp ]; then
  if NEUTRAL_DIR=$(mktemp -d /var/tmp/bp-rail.XXXXXX 2>/dev/null); then
    chmod 700 "$NEUTRAL_DIR"
    if ln -s "$RUN" "$NEUTRAL_DIR/r" 2>/dev/null; then
      NEUTRAL_RUN="$NEUTRAL_DIR/r"
    else
      rmdir "$NEUTRAL_DIR" 2>/dev/null || true
      NEUTRAL_DIR=""
    fi
  else
    NEUTRAL_DIR=""
  fi
fi

# Helper: run a KiCad CLI probe with strictly isolated HOME/XDG and no DISPLAY.
kicad_probe_env() {
  env -u DISPLAY -u WAYLAND_DISPLAY \
    HOME="$RUN/fakehome" \
    XDG_CONFIG_HOME="$RUN/fakehome/.config" \
    XDG_CACHE_HOME="$RUN/fakehome/.cache" \
    XDG_DATA_HOME="$RUN/fakehome/.local/share" \
    XDG_RUNTIME_DIR="$RUN/run" \
    TMPDIR="$RUN/tmp" \
    LC_ALL=C \
    "$@"
}

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

# Locate KiCad CLI for Gerber generation and hub plotting (prefer test builds, never ~/.backplane*)
KICAD_CLI="${BACKPLANE_KICAD_CLI:-}"
if [ -z "$KICAD_CLI" ] || [ ! -x "$KICAD_CLI" ]; then
  KICAD_TEST_ROOT="${RAIL_KICAD_TEST_ROOT:-/data/kicad-test/builds}"
  for cand in \
    $(ls -1dt "$KICAD_TEST_ROOT"/10.99.0-bp.*/bin/kicad-cli 2>/dev/null || true) \
    "$REAL_HOME/.local/bin/kicad-cli" \
    "$(command -v kicad-cli 2>/dev/null || true)"; do
    if [ -n "$cand" ] && [ -x "$cand" ]; then
      KICAD_CLI="$cand"
      break
    fi
  done
fi

# When --embed is enabled, discover a Backplane fork KiCad runtime strictly under the test-build root
# (/data/kicad-test/builds/*, matching docs/kicad-embed-e2e.md) — never the user's install or home.
EMBED_KICAD_CLI=""
RUNTIME_CAPS_JSON="$OUTDIR/runtime.json"
if [ "$EMBED_MODE_FLAG" = "1" ]; then
  KICAD_TEST_ROOT="${RAIL_KICAD_TEST_ROOT:-/data/kicad-test/builds}"
  is_fork_kicad() { [ -x "$1" ] && grep -qa 'kiapi.backplane.embed' "$(dirname "$1")/../share/backplane/api.desc" 2>/dev/null; }
  has_sim_frame() { kicad_probe_env "$1" api-server --describe 2>/dev/null | python3 -c 'import json,sys; sys.exit(0 if "sim-frame" in json.load(sys.stdin).get("capabilities", []) else 1)' 2>/dev/null; }
  EMBED_KICAD_BIN=""
  if [ -d "$KICAD_TEST_ROOT" ]; then
    for cand in $(ls -1dt "$KICAD_TEST_ROOT"/10.99.0-bp.*/bin/kicad 2>/dev/null || true); do
      [ -n "$cand" ] || continue
      if is_fork_kicad "$cand" && [ -x "$(dirname "$cand")/kicad-cli" ] && has_sim_frame "$(dirname "$cand")/kicad-cli"; then
        EMBED_KICAD_BIN="$cand"
        EMBED_KICAD_CLI="$(dirname "$cand")/kicad-cli"
        break
      fi
    done
  fi
  if [ -z "$EMBED_KICAD_BIN" ]; then
    echo "rail-shots: --embed requires a Backplane fork KiCad test build with sim-frame under $KICAD_TEST_ROOT" >&2
    exit 2
  fi
  EMBED_RT=$(cd "$(dirname "$EMBED_KICAD_BIN")/.." && pwd)
  EMBED_DESC=$(kicad_probe_env "$EMBED_KICAD_CLI" api-server --describe 2>/dev/null || printf '{}')
  if ! python3 -c 'import json,sys; sys.exit(0 if "sim-frame" in json.load(open(sys.argv[1])).get("capabilities", []) else 1)' "$EMBED_RT/manifest.json" 2>/dev/null; then
    mkdir -p "$RUN/rt"
    for e in "$EMBED_RT"/*; do
      [ "$(basename "$e")" = manifest.json ] || ln -sfn "$e" "$RUN/rt/$(basename "$e")"
    done
    printf '%s\n' "$EMBED_DESC" >"$RUN/rt/manifest.json"
    EMBED_KICAD_CLI="$NEUTRAL_RUN/rt/bin/kicad-cli"
  fi
  printf '%s\n' "$EMBED_DESC" | python3 -c 'import json,sys; d=json.load(sys.stdin); print(json.dumps({"embed_mode": True, "runtime_root": sys.argv[1], "capabilities": sorted(d.get("capabilities", []))}, indent=2))' "$EMBED_RT" >"$RUNTIME_CAPS_JSON"
  echo "rail-shots: --embed runtime -> $EMBED_KICAD_CLI"

  # Seed isolated KiCad XDG configs so embedded KiCad skips first-run wizard and enables API server.
  python3 - "$RUN/fakehome/.config/kicad" "$NEUTRAL_RUN/proj-full" "$EMBED_KICAD_CLI" <<'PY'
import json, os, sys
cfg_base, proj, kcli = sys.argv[1:4]
tpl = os.path.join(os.path.dirname(os.path.dirname(kcli)), "share", "kicad", "template")
def read_tpl(name, fallback):
    for p in (os.path.join(tpl, name), f"/usr/share/kicad/template/{name}"):
        if os.path.isfile(p):
            return open(p, encoding="utf-8").read()
    return fallback
sym_text = read_tpl("sym-lib-table", "(sym_lib_table\n  (version 7)\n)\n")
fp_text = read_tpl("fp-lib-table", "(fp_lib_table\n  (version 7)\n)\n")
common_cfg = {
    "meta": {"filename": "kicad_common.json", "version": 6},
    "api": {"enable_server": True, "interpreter_path": ""},
    "appearance": {"toolbar_icon_size": 24},
    "do_not_show_again": {"data_collection_prompt": True, "update_check_prompt": True, "zone_fill_warning": True},
    "environment": {"vars": None},
    "system": {"check_for_updates": False, "WorkingDir": proj},
}
editor_cfg = lambda fn, v: {
    "meta": {"filename": fn, "version": v},
    "appearance": {"canvas_scale": 1.0, "show_page_limits": True},
    "aui": {"show_properties": False, "show_layer_manager": False, "show_hierarchy": False, "show_schematic_hierarchy": False, "show_search": False, "show_net_inspector": False, "design_blocks_show": False},
    "graphics": {"canvas_type": 1},
    "window": {"maximized": False, "size_x": 900, "size_y": 720, "pos_x": 40, "pos_y": 40},
}
for ver in ("10.0", "10.99"):
    cdir = os.path.join(cfg_base, ver)
    os.makedirs(cdir, exist_ok=True)
    for fn, obj in (
        ("kicad_common.json", common_cfg),
        ("pcbnew.json", editor_cfg("pcbnew.json", 5)),
        ("fpedit.json", editor_cfg("fpedit.json", 5)),
        ("eeschema.json", editor_cfg("eeschema.json", 3)),
        ("symbol_editor.json", editor_cfg("symbol_editor.json", 1)),
        ("3d_viewer.json", editor_cfg("3d_viewer.json", 4)),
    ):
        with open(os.path.join(cdir, fn), "w", encoding="utf-8") as f:
            json.dump(obj, f, indent=2)
            f.write("\n")
    with open(os.path.join(cdir, "sym-lib-table"), "w", encoding="utf-8") as f: f.write(sym_text)
    with open(os.path.join(cdir, "fp-lib-table"), "w", encoding="utf-8") as f: f.write(fp_text)
    with open(os.path.join(cdir, "design-block-lib-table"), "w", encoding="utf-8") as f: f.write("(design_block_lib_table\n  (version 7)\n)\n")
PY
else
  printf '{"embed_mode": false, "runtime_root": "", "capabilities": []}\n' >"$RUNTIME_CAPS_JSON"
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

screen_width=1440
[[ ${RAIL_B2_SIZE:-} != wide ]] || screen_width=2200
# Folded sidebar 44 + thread 360 + minimum viewer 320 + rail 72.
[[ ${RAIL_B2_SIZE:-} != minimum ]] || screen_width=796
own env LD_LIBRARY_PATH="$XVFB_LD" "$XVFB_BIN" "$DISP" -screen 0 "${screen_width}x900x24" -nolisten tcp -xkbdir /usr/share/X11/xkb >"$RUN/xvfb.log" 2>&1
for _ in $(seq 40); do [ -e "/tmp/.X11-unix/X$d" ] && break; sleep 0.1; done
[ -e "/tmp/.X11-unix/X$d" ] || { echo "rail-shots: Xvfb failed to start on $DISP" >&2; cat "$RUN/xvfb.log" >&2; exit 1; }

cat >"$RUN/i3.conf" <<EOF
font pango:monospace 8
new_window none
for_window [class=".*"] border none
for_window [class="(?i)kicad|pcbnew|eeschema"] floating enable
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
  mkdir -p "$dir"
  printf '# %s fixture\n\nInitial text for rail-shots.\n' "$kind" >"$dir/README.md"

  case "$kind" in
    full)
      cp "$PLOT_FIX/ecc83-pp.kicad_pcb" "$PLOT_FIX/ecc83-pp.kicad_sch" "$PLOT_FIX/ecc83-pp.kicad_prl" "$dir/"
      printf '{}\n' >"$dir/ecc83-pp.kicad_pro"
      mkdir -p "$dir/mech" "$dir/mfg/gerbers"
      cp "$MESH_FIX/bracket.step" "$dir/mech/bracket.step"
      if [ -n "$KICAD_CLI" ] && [ -x "$KICAD_CLI" ]; then
        kicad_probe_env "$KICAD_CLI" pcb export gerbers --layers "F.Cu,B.Cu,F.SilkS,F.Mask,B.Mask,Edge.Cuts" -o "$dir/mfg/gerbers/" "$dir/ecc83-pp.kicad_pcb" >/dev/null 2>&1 || true
        kicad_probe_env "$KICAD_CLI" pcb export drill -o "$dir/mfg/gerbers/" "$dir/ecc83-pp.kicad_pcb" >/dev/null 2>&1 || true
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
    board-only)
      cp "$PLOT_FIX/ecc83-pp.kicad_pcb" "$PLOT_FIX/ecc83-pp.kicad_prl" "$dir/"
      printf '{}\n' >"$dir/ecc83-pp.kicad_pro"
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
    printf '*.lck\n~*.lck\n' >.gitignore
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

for kind in "${KINDS[@]}"; do
  echo "rail-shots: === kind=$kind ==="
  KIND_OUT="$OUTDIR/$kind"
  mkdir -p "$KIND_OUT"
  # Create directories strictly inside our fresh $RUN under $BP_TMP; pass neutral alias paths to hub/client.
  mkdir -p "$RUN/proj-$kind" "$RUN/home-$kind" "$RUN/shots-$kind"
  PROJ="$NEUTRAL_RUN/proj-$kind"
  HHOME="$NEUTRAL_RUN/home-$kind"
  SHOT_DIR="$RUN/shots-$kind"
  STOP_FLAG="$RUN/stop-$kind"

  seed_project "$kind" "$RUN/proj-$kind"

  USE_EMBED=0
  if [ "$EMBED_MODE_FLAG" = "1" ]; then
    USE_EMBED=1
  fi

  python3 - "$HHOME" "$PROJ" "$kind" "$USE_EMBED" "${EMBED_KICAD_CLI:-}" <<'PY'
import json, sys, time
hhome, proj, kind, use_embed, kcli = sys.argv[1:6]
now = int(time.time())
ev = [
    {"$": "ProjectCreated", "id": "p1", "title": kind, "root": proj, "at": now},
    {"$": "ThreadCreated", "id": "t1", "project": "p1", "title": f"project {kind}", "env": "local", "branch": "", "worktree": "", "provider": "claude", "model": "", "at": now},
]
if use_embed == "1" and kcli:
    ev.append({"$": "SettingSet", "key": "kicad.path", "value": kcli, "at": now})
with open(f"{hhome}/events.jsonl", "w", encoding="utf-8") as f:
    for e in ev:
        f.write(json.dumps(e, separators=(",", ":")) + "\n")
PY

  # Make the fixture's git change AFTER the thread exists so Diff compares against turn/0.
  seed_thread_change "$PROJ" "t1"

  PORT=$(free_port)
  if [ "$USE_EMBED" = "1" ]; then
    # Embedded KiCad mode: hub spawns fork KiCad on our private Xvfb display ($DISP) with software GL.
    own env -u WAYLAND_DISPLAY \
      HOME="$NEUTRAL_RUN/fakehome" \
      XDG_CONFIG_HOME="$NEUTRAL_RUN/fakehome/.config" \
      XDG_CACHE_HOME="$NEUTRAL_RUN/fakehome/.cache" \
      XDG_DATA_HOME="$NEUTRAL_RUN/fakehome/.local/share" \
      XDG_RUNTIME_DIR="$RUN/run" \
      TMPDIR="$RUN/tmp" \
      LC_ALL=C \
      DISPLAY="$DISP" \
      GDK_BACKEND=x11 \
      GDK_SCALE=1 \
      LIBGL_ALWAYS_SOFTWARE=1 \
      GALLIUM_DRIVER=llvmpipe \
      LP_NUM_THREADS=2 \
      VK_ICD_FILENAMES="$ICD" \
      BACKPLANE_AGENTS="$RUN/bin" \
      BACKPLANE_NO_UPDATE=1 \
      BP_STANDIN_MODE=stream \
      BACKPLANE_KICAD_EMBED_MODE=fork \
      BACKPLANE_KICAD_CLI="$EMBED_KICAD_CLI" \
      "$HUB_BIN" --home "$HHOME" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground >"$KIND_OUT/hub.log" 2>&1
  else
    own env -u DISPLAY -u WAYLAND_DISPLAY \
      HOME="$NEUTRAL_RUN/fakehome" \
      XDG_CONFIG_HOME="$NEUTRAL_RUN/fakehome/.config" \
      XDG_CACHE_HOME="$NEUTRAL_RUN/fakehome/.cache" \
      XDG_DATA_HOME="$NEUTRAL_RUN/fakehome/.local/share" \
      XDG_RUNTIME_DIR="$RUN/run" \
      TMPDIR="$RUN/tmp" \
      LC_ALL=C \
      BACKPLANE_AGENTS="$RUN/bin" \
      BACKPLANE_NO_UPDATE=1 \
      BP_STANDIN_MODE=stream \
      BACKPLANE_KICAD_CLI="${KICAD_CLI:-}" \
      "$HUB_BIN" --home "$HHOME" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground >"$KIND_OUT/hub.log" 2>&1
  fi
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
  DRV_EXTRA=()
  [ "$USE_EMBED" = "1" ] && DRV_EXTRA+=(--embed)
  if [[ -n ${RAIL_B2_SIZE:-} ]]; then
    RAIL_B2_KIND="$kind" python3 "$SCRIPT_DIR/rail_b2.py" --script >"$SCRIPT_JSON"
  else
    python3 "$SCRIPT_DIR/rail_shots_driver.py" gen-script "$kind" "$SCRIPT_JSON" "${RAIL_EARLY_MS:-300}" "${RAIL_SETTLE_TIMEOUT_MS:-15000}" "${DRV_EXTRA[@]}"
  fi

  # Start external capture watcher (only watcher's `import` and `xwininfo` receive DISPLAY=$DISP)
  own env RAIL_HUB_PORT="$PORT" RAIL_HUB_TOKEN_FILE="$HHOME/token" RAIL_BUN_BIN="$BUN_BIN" \
    python3 "$SCRIPT_DIR/rail_shots_driver.py" watch "$DISP" "$KIND_OUT/client.log" "$SHOT_DIR" "$KIND_OUT" "$STOP_FLAG" >"$KIND_OUT/watch.log" 2>&1
  WATCH_PG=$!

  # Launch client in its own process group and check its real exit status
  own env \
    HOME="$NEUTRAL_RUN/fakehome" \
    XDG_CONFIG_HOME="$NEUTRAL_RUN/fakehome/.config" \
    XDG_CACHE_HOME="$NEUTRAL_RUN/fakehome/.cache" \
    XDG_DATA_HOME="$NEUTRAL_RUN/fakehome/.local/share" \
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
    "$CLIENT_BIN" --url "http://127.0.0.1:$PORT" --token-file "$HHOME/token" --script "$SCRIPT_JSON" --exit-after 420 >"$KIND_OUT/client.log" 2>&1
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

if [[ -n ${RAIL_B2_SIZE:-} ]]; then
  exit "$CLIENT_RC"
fi
POST_EXTRA=()
[ "$EMBED_MODE_FLAG" = "1" ] && POST_EXTRA+=(--embed)
[ "$SHARE_FLAG" = "1" ] && POST_EXTRA+=(--share)
set +e
python3 "$SCRIPT_DIR/rail_shots_post.py" "${POST_EXTRA[@]}" "$OUTDIR" "${KINDS[@]}"
POST_RC=$?
set -e

echo "rail-shots: done (post_rc=$POST_RC) -> $OUTDIR"
for kind in "${KINDS[@]}"; do
  echo "  contact: $OUTDIR/contact-$kind.png"
done
echo "  headers: $OUTDIR/headers.png"
echo "  rails:   $OUTDIR/rails.png"
echo "  steps:   $OUTDIR/steps.json"
exit "$POST_RC"
