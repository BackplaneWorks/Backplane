#!/bin/bash
# Shared Xvfb + i3 + BP_TMP + process-group environment for test/kicad_embed/*.sh.
# Obeys AGENTS.md: temp space in $BP_TMP (never /tmp), stand-in agents only, one KiCad at a time,
# and cleanup strictly via captured PIDs / own-group.sh.

KEMBED_LIB=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd "$KEMBED_LIB/../../.." && pwd)

# shellcheck source=../../../scripts/lib-bptmp.sh
. "$REPO_ROOT/scripts/lib-bptmp.sh"
# shellcheck source=../../e2e/lib/own-group.sh
. "$REPO_ROOT/test/e2e/lib/own-group.sh"
# shellcheck source=gpu_display.sh
. "$KEMBED_LIB/gpu_display.sh"

REAL_HOME="$HOME"
EMBED_MODE=${EMBED_MODE:-spike}
OUT=${OUT:-$(mktemp -d "$BP_TMP/bp-kembed.XXXXXX")}
mkdir -p "$OUT"
OUT=$(cd "$OUT" && pwd)

# Discover Backplane client, hub, and assets when not explicitly passed.
# Prefer the worktree's own built target/fast binaries first so local edits are tested.
discover_bins() {
  local cur_name cur_dir main_git
  [ -z "${CLIENT_BIN:-}" ] && [ -x "$REPO_ROOT/native/gpui/target/fast/backplane-gpui" ] && CLIENT_BIN="$REPO_ROOT/native/gpui/target/fast/backplane-gpui"
  [ -z "${HUB_BIN:-}" ] && [ -x "$REPO_ROOT/native/host/target/fast/backplane-hub" ] && HUB_BIN="$REPO_ROOT/native/host/target/fast/backplane-hub"
  if [ -z "${CLIENT_BIN:-}" ] || [ -z "${HUB_BIN:-}" ]; then
    if [ -f "$REAL_HOME/.backplane-gpui-dev/CURRENT" ]; then
      cur_name=$(cat "$REAL_HOME/.backplane-gpui-dev/CURRENT" 2>/dev/null || true)
      cur_dir="$REAL_HOME/.backplane-gpui-dev/builds/$cur_name"
      [ -z "${CLIENT_BIN:-}" ] && [ -x "$cur_dir/backplane-gpui" ] && CLIENT_BIN="$cur_dir/backplane-gpui"
      [ -z "${HUB_BIN:-}" ] && [ -x "$cur_dir/backplane-hub" ] && HUB_BIN="$cur_dir/backplane-hub"
      [ -z "${ASSETS_DIR:-}" ] && [ -d "$cur_dir/assets" ] && ASSETS_DIR="$cur_dir/assets"
    fi
  fi
  main_git=$(dirname "$(git -C "$REPO_ROOT" rev-parse --path-format=absolute --git-common-dir 2>/dev/null || echo "$REPO_ROOT/.git")")
  [ -z "${ASSETS_DIR:-}" ] && [ -d "$REPO_ROOT/assets" ] && ASSETS_DIR="$REPO_ROOT/assets"
  [ -z "${ASSETS_DIR:-}" ] && [ -d "$main_git/assets" ] && ASSETS_DIR="$main_git/assets"
  export CLIENT_BIN HUB_BIN ASSETS_DIR
}

discover_kicad() {
  if [ "$EMBED_MODE" = "stock" ]; then
    KICAD_BIN=${KICAD_BIN:-${BACKPLANE_KICAD_BIN:-/usr/bin/kicad}}
    [ -x "$KICAD_BIN" ] || KICAD_BIN=$(command -v kicad || true)
    KICAD_CLI=${BACKPLANE_KICAD_CLI:-/usr/bin/kicad-cli}
    [ -x "$KICAD_CLI" ] || KICAD_CLI=$(command -v kicad-cli || true)
  elif [ "$EMBED_MODE" = "fork" ]; then
    if [ -z "${KICAD_BIN:-}" ]; then
      if [ -n "${BACKPLANE_KICAD_BIN:-}" ]; then
        KICAD_BIN="$BACKPLANE_KICAD_BIN"
      else
        local cand
        for cand in $(ls -1dr "$REAL_HOME"/.local/share/backplane/kicad/10.99.0-bp.*/bin/kicad 2>/dev/null || true); do
          if [ -x "$cand" ] && grep -qa 'kiapi.backplane.embed' "$(dirname "$cand")/../share/backplane/api.desc" 2>/dev/null; then
            KICAD_BIN="$cand"
            break
          fi
        done
      fi
    fi
    if [ -z "${KICAD_BIN:-}" ] || [ ! -x "$KICAD_BIN" ]; then
      echo "SKIP/FAIL EMBED_MODE=fork requires Backplane KiCad >= 10.99.0-bp.8 (--embedded); pass BACKPLANE_KICAD_BIN=/path/to/kicad" >&2
      exit 2
    fi
    # the cli beside the KiCad asked for: the hub takes the GUI kicad from beside its cli, and an agent's shell exports BACKPLANE_KICAD_CLI
    # for the runtime the person runs (a "bp.12" run was bp.11 through it)
    if [ -x "$(dirname "$KICAD_BIN")/kicad-cli" ]; then KICAD_CLI="$(dirname "$KICAD_BIN")/kicad-cli"; else KICAD_CLI=${BACKPLANE_KICAD_CLI:-$(dirname "$KICAD_BIN")/kicad-cli}; fi
  else
    KICAD_CLI=${BACKPLANE_KICAD_CLI:-$REAL_HOME/.local/bin/kicad-cli}
    [ -x "$KICAD_CLI" ] || KICAD_CLI=$(command -v kicad-cli || true)
    if [ -z "${KICAD_BIN:-}" ]; then
      if [ -n "${BACKPLANE_KICAD_BIN:-}" ]; then
        KICAD_BIN="$BACKPLANE_KICAD_BIN"
      elif [ -x /usr/bin/kicad ]; then
        KICAD_BIN=/usr/bin/kicad
      else
        KICAD_BIN=$(command -v kicad || true)
      fi
    fi
  fi
  KICAD_DIR=$(dirname "$(readlink -f "$KICAD_BIN" 2>/dev/null || echo "$KICAD_BIN")")
  PCBNEW_BIN=${PCBNEW_BIN:-$KICAD_DIR/pcbnew}
  [ -x "$PCBNEW_BIN" ] || PCBNEW_BIN=$(command -v pcbnew || true)
  EESCHEMA_BIN=${EESCHEMA_BIN:-$KICAD_DIR/eeschema}
  [ -x "$EESCHEMA_BIN" ] || EESCHEMA_BIN=$(command -v eeschema || true)
  export KICAD_CLI KICAD_BIN PCBNEW_BIN EESCHEMA_BIN
}

free_port() {
  python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()'
}

KEMBED_PIDS=()
track_pid() { KEMBED_PIDS+=("$1"); }

stop_pid() {
  local p=$1
  [ -n "$p" ] || return 0
  kill -TERM "$p" 2>/dev/null || true
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    kill -0 "$p" 2>/dev/null || return 0
    sleep 0.15
  done
  kill -KILL "$p" 2>/dev/null || true
}

kembed_cleanup() {
  local rc=$?
  set +e
  local p
  for p in "${KEMBED_PIDS[@]}"; do
    kill -CONT "$p" 2>/dev/null || true
    kill -TERM "$p" 2>/dev/null || true
  done
  sleep 0.4
  for p in "${KEMBED_PIDS[@]}"; do
    kill -KILL "$p" 2>/dev/null || true
  done
  own_stop || true
  exit "$rc"
}
trap kembed_cleanup EXIT INT TERM

# Isolate HOME, XDG_*, and TMPDIR under $OUT (short path in $BP_TMP so unix sockets stay <108 chars).
init_isolated_dirs() {
  rm -rf "$OUT/home" "$OUT/fh" "$OUT/proj" "$OUT/origin.git" "$OUT/bin" "$OUT/run" "$OUT/tmp" "$OUT/shots"
  mkdir -p "$OUT/home" "$OUT/fh/.config" "$OUT/fh/.cache" "$OUT/fh/.local/share" "$OUT/proj" "$OUT/bin" "$OUT/shots"
  mkdir -p -m 700 "$OUT/run" "$OUT/tmp"
  export HOME="$OUT/fh"
  export XDG_CONFIG_HOME="$OUT/fh/.config"
  export XDG_CACHE_HOME="$OUT/fh/.cache"
  export XDG_DATA_HOME="$OUT/fh/.local/share"
  export XDG_RUNTIME_DIR="$OUT/run"
  export TMPDIR="$OUT/tmp"
  export LC_ALL=C
}

start_xvfb_and_wm() {
  local xv="" xvb="" xld="" d icd
  for cand in "${XVFB_ROOT:-}" "/home/h/.local/xvfb-root" "/home/h/.cache/xvfb-root"; do
    if [ -n "$cand" ] && [ -x "$cand/usr/bin/Xvfb" ]; then xv="$cand"; break; fi
  done
  if [ -n "$xv" ]; then
    xvb="$xv/usr/bin/Xvfb"
    xld="$xv/usr/lib/x86_64-linux-gnu"
  else
    xvb=$(command -v Xvfb)
  fi
  # REAL_HARDWARE=1: a private display the GPU draws (lib/gpu_display.sh); Xvfb has none. KEMBED_XVFB=1 keeps Xvfb.
  if [ "${REAL_HARDWARE:-0}" = "1" ] && [ "${KEMBED_XVFB:-0}" != "1" ] && gpu_display_available; then
    start_gpu_display || return 1
  else
  d=$((520 + $$ % 200))
  while [ -e "/tmp/.X11-unix/X$d" ] || [ -e "/tmp/.X$d-lock" ]; do d=$((d + 1)); done
  DISP=":$d"

  own env LD_LIBRARY_PATH="$xld" "$xvb" "$DISP" -screen 0 1800x1050x24 -nolisten tcp -xkbdir /usr/share/X11/xkb >"$OUT/xvfb.log" 2>&1
  for _ in $(seq 30); do [ -e "/tmp/.X11-unix/X$d" ] && break; sleep 0.1; done
  export DISPLAY="$DISP"
  fi

  cat >"$OUT/i3.conf" <<EOF
font pango:monospace 8
new_window none
for_window [class=".*"] border none
for_window [class="(?i)kicad|pcbnew|eeschema"] floating enable
default_border none
hide_edge_borders both
ipc-socket $OUT/i3.sock
EOF
  own env DISPLAY="$DISP" XDG_RUNTIME_DIR="$OUT/run" i3 -c "$OUT/i3.conf" >"$OUT/i3.log" 2>&1
  for _ in $(seq 30); do [ -S "$OUT/i3.sock" ] && break; sleep 0.1; done

  export GDK_BACKEND=x11
  export GDK_SCALE=1
  if [ "${REAL_HARDWARE:-0}" != "1" ]; then
    icd=$(ls /usr/share/vulkan/icd.d/lvp_icd*.json 2>/dev/null | head -1 || true)
    export LIBGL_ALWAYS_SOFTWARE=1
    export GALLIUM_DRIVER=llvmpipe
    export LP_NUM_THREADS=2
    [ -n "$icd" ] && export VK_ICD_FILENAMES="$icd"
  fi
}

start_hub() {
  # software GL for the hub's KiCad unless REAL_HARDWARE=1 (then KiCad draws with the display's GPU, as for the person)
  KEMBED_SW_ENV=()
  [ "${REAL_HARDWARE:-0}" = "1" ] || KEMBED_SW_ENV=(LIBGL_ALWAYS_SOFTWARE="${LIBGL_ALWAYS_SOFTWARE:-1}" GALLIUM_DRIVER="${GALLIUM_DRIVER:-llvmpipe}" LP_NUM_THREADS="${LP_NUM_THREADS:-2}" VK_ICD_FILENAMES="${VK_ICD_FILENAMES:-}")
  HPORT=$(free_port)
  export HPORT
  local mode_env=""
  if [ "$EMBED_MODE" = "stock" ] || [ "$EMBED_MODE" = "fork" ]; then
    mode_env="$EMBED_MODE"
  fi
  own env \
    HOME="$OUT/fh" \
    XDG_CONFIG_HOME="$OUT/fh/.config" \
    XDG_CACHE_HOME="$OUT/fh/.cache" \
    XDG_DATA_HOME="$OUT/fh/.local/share" \
    XDG_RUNTIME_DIR="$OUT/run" \
    TMPDIR="$OUT/tmp" \
    DISPLAY="$DISP" \
    GDK_BACKEND=x11 \
    GDK_SCALE=1 \
    ${KEMBED_SW_ENV[@]+"${KEMBED_SW_ENV[@]}"} \
    BACKPLANE_KICAD_CLI="${KICAD_CLI:-}" \
    BACKPLANE_KICAD_EMBED_MODE="$mode_env" \
    BACKPLANE_AGENTS="$OUT/bin" \
    BACKPLANE_NO_UPDATE=1 \
    FAKE_LOG="$OUT/standin.log" \
    BP_STANDIN_MODE=stream \
    BP_STANDIN_MCPCALL_MAX="${BP_STANDIN_MCPCALL_MAX:-600}" \
    "$HUB_BIN" --home "$OUT/home" --port "$HPORT" --host 127.0.0.1 --no-tailscale --foreground >"$OUT/hub.log" 2>&1
  for _ in $(seq 80); do
    curl -s -m 1 -o /dev/null "http://127.0.0.1:$HPORT/hello" && return 0
    sleep 0.15
  done
  echo "FAIL hub did not start on port $HPORT"
  tail -n 20 "$OUT/hub.log" || true
  return 1
}

PASS_COUNT=0
FAIL_COUNT=0
check() {
  local what=$1 good=$2 detail=${3:-}
  if [ "$good" = "1" ] || [ "$good" = "true" ]; then
    echo "PASS $what"
    PASS_COUNT=$((PASS_COUNT + 1))
  else
    echo "FAIL $what${detail:+ ($detail)}"
    FAIL_COUNT=$((FAIL_COUNT + 1))
  fi
}

shot() {
  local name=$1
  import -window root "$OUT/shots/$name.png" 2>/dev/null || true
  xwininfo -root -tree >"$OUT/shots/$name.tree" 2>&1 || true
}

# The KiCad the hub really ran (its "kicad embed: running <version> (<argv0>)" line) against the one asked for: a mismatch fails the run.
check_runtime() {
  local ran want bin
  [ -f "$OUT/hub.log" ] || return 0
  ran=$(grep -o "kicad embed: running .*" "$OUT/hub.log" | head -1 | sed 's/^kicad embed: running //' || true) # (a suite that opens no editor has no such line)
  [ -n "$ran" ] || return 0
  want=$("${KICAD_CLI:-kicad-cli}" --version 2>/dev/null | tail -1)
  bin=$(echo "$ran" | sed -n 's/.*(\(.*\))$/\1/p')
  echo "RUNTIME asked ${KICAD_BIN:-?} ($want) ran $ran"
  local v=${ran%% *} same=1
  if [ -n "$want" ] && [ -n "$v" ] && [[ "$want" != *"$v"* ]] && [[ "$v" != *"$want"* ]]; then same=0; fi
  if [ -n "${KICAD_BIN:-}" ] && [ -n "$bin" ] && [ "$(dirname "$(readlink -f "$bin")")" != "$(dirname "$(readlink -f "$KICAD_BIN")")" ]; then same=0; fi
  check "the KiCad run is the one asked for" "$same" "asked $KICAD_BIN $want, ran $ran"
}

finish_suite() {
  local suite_name=$1
  check_runtime
  echo "$suite_name: passed=$PASS_COUNT failed=$FAIL_COUNT artifacts=$OUT"
  [ "$FAIL_COUNT" -eq 0 ]
}

DRIVER_PY="$KEMBED_LIB/x11_embed_driver.py"
IPC_PY="$KEMBED_LIB/gui_ipc_probe.py"
SEED_PY="$KEMBED_LIB/seed_project.py"
HUB_RPC_MJS="$KEMBED_LIB/hub_rpc.mjs"
XINPUT_PY="$REPO_ROOT/test/cef/xinput.py"

hub_rpc() {
  local steps=${1:-[]}
  bun "$HUB_RPC_MJS" "$HPORT" "$OUT/home/token" "$steps"
}

last_kicad_report() {
  local log=$1 tag=${2:-}
  python3 - "$log" "$tag" <<'PY'
import json, sys
log_path, want_tag = sys.argv[1], sys.argv[2]
found = None
for line in open(log_path, encoding="utf-8", errors="replace"):
    if "GPUI_METRIC " not in line:
        continue
    try:
        obj = json.loads(line.split("GPUI_METRIC ", 1)[1])
    except Exception:
        continue
    if obj.get("event") != "kicad":
        continue
    if want_tag and obj.get("tag") != want_tag:
        continue
    found = obj
print(json.dumps(found or {}))
PY
}

wait_for_log() {
  local log=$1 needle=$2 tries=${3:-160}
  for _ in $(seq "$tries"); do
    if grep -Fq "$needle" "$log" 2>/dev/null; then
      return 0
    fi
    sleep 0.25
  done
  return 1
}

wait_for_ipc() {
  local sock=$1 tries=${2:-80}
  for _ in $(seq "$tries"); do
    if [ -S "$sock" ] && python3 "$IPC_PY" ping "$sock" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.2
  done
  return 1
}

wait_for_kicad_win() {
  local pid=$1 sub=$2 tries=${3:-80} xid=""
  for _ in $(seq "$tries"); do
    xid=$(python3 "$DRIVER_PY" list-windows --match "$sub" 2>/dev/null | python3 -c '
import json, sys
pid = int(sys.argv[1])
ws = json.load(sys.stdin)
good = [
    w for w in ws
    if w.get("shown") and not w.get("override_redirect")
    and w.get("w", 0) >= 180 and w.get("h", 0) >= 100
    and (w.get("pid") in (pid, 0) or "kicad" in w.get("wm_class", "").lower())
]
print(good[0]["xid"] if good else "")
' "$pid" 2>/dev/null || true)
    if [ -n "$xid" ]; then
      echo "$xid"
      return 0
    fi
    sleep 0.25
  done
  echo "DIAG timeout waiting for KiCad window '$sub' (pid=$pid); current X11 windows:" >&2
  python3 "$DRIVER_PY" list-windows >&2 || true
  return 1
}

start_container() {
  local gpui_xid=$1 x=$2 y=$3 w=$4 h=$5 tag=${6:-c1}
  local ready="$OUT/container_${tag}.ready"
  rm -f "$ready"
  python3 "$DRIVER_PY" container-daemon "$gpui_xid" "$x" "$y" "$w" "$h" "$ready" >"$OUT/container_${tag}.log" 2>&1 &
  CONTAINER_LAST_PID=$!
  track_pid "$CONTAINER_LAST_PID"
  for _ in $(seq 50); do
    if [ -s "$ready" ]; then
      CONTAINER_LAST_XID=$(head -1 "$ready" | tr -d '[:space:]')
      return 0
    fi
    sleep 0.1
  done
  return 1
}

click_win_center() {
  local target_xid=$1
  local xy
  xy=$(python3 "$DRIVER_PY" list-windows | python3 -c '
import json, sys
want = int(sys.argv[1], 0)
for w in json.load(sys.stdin):
    if w["xid_int"] == want:
        print(w["root_x"] + w["w"] // 2, w["root_y"] + w["h"] // 2)
        break
' "$target_xid")
  if [ -n "$xy" ]; then
    # shellcheck disable=SC2086
    python3 "$XINPUT_PY" click $xy
  fi
  python3 "$DRIVER_PY" focus "$target_xid" >/dev/null
}

# Click a Backplane GPUI control (e.g. "rail:Schematic", "rail:Board", "rail:3D", "ui:View", "ui:Edit")
# using its latest logged GPUI control rect + GPUI top-level window's root coordinates.
click_gpui_control() {
  local log=$1 gpui_xid=$2 ctl_id=$3
  python3 "$XINPUT_PY" chord ctrl grave 2>/dev/null || true
  sleep 0.1
  local xy
  xy=$(python3 - "$log" "$DRIVER_PY" "$gpui_xid" "$ctl_id" <<'PY'
import json, subprocess, sys
log_path, drv, gpui_xid_s, ctl_id = sys.argv[1:5]
gpui_xid = int(gpui_xid_s, 0)
rect = None
for line in open(log_path, encoding="utf-8", errors="replace"):
    if "GPUI_METRIC " not in line or '"controls"' not in line:
        continue
    try:
        obj = json.loads(line.split("GPUI_METRIC ", 1)[1])
    except Exception:
        continue
    if obj.get("event") != "controls":
        continue
    for c in obj.get("controls", []):
        if c.get("id") == ctl_id and c.get("rect"):
            rect = c["rect"]
if not rect:
    sys.exit(0)
wins = json.loads(subprocess.check_output([sys.executable, drv, "list-windows"], text=True))
gx, gy = 0, 0
for w in wins:
    if w["xid_int"] == gpui_xid:
        gx, gy = w["root_x"], w["root_y"]
        break
cx = int(round(gx + rect["x"] + rect["w"] / 2.0))
cy = int(round(gy + rect["y"] + rect["h"] / 2.0))
print(f"{cx} {cy}")
PY
)
  if [ -n "$xy" ]; then
    # shellcheck disable=SC2086
    python3 "$XINPUT_PY" click $xy
    return 0
  fi
  echo "DIAG click_gpui_control: control '$ctl_id' not found in $log" >&2
  return 1
}

# Read-only poll until Backplane's client has adopted/mapped a frame matching `sub` inside `cont_xid` and it painted.
wait_for_embedded_frame() {
  local gpui_xid=$1 cont_xid=$2 pid=$3 sub=$4 tries=${5:-160}
  local xid="" rpt="" ok=""
  for _ in $(seq "$tries"); do
    xid=$(python3 "$DRIVER_PY" list-windows --match "$sub" 2>/dev/null | python3 -c '
import json, sys
pid = int(sys.argv[1])
ws = json.load(sys.stdin)
good = [
    w for w in ws
    if w.get("shown") and not w.get("override_redirect")
    and w.get("w", 0) >= 180 and w.get("h", 0) >= 100
    and (pid == 0 or w.get("pid") in (pid, 0) or "kicad" in w.get("wm_class", "").lower())
]
print(good[0]["xid"] if good else "")
' "$pid" 2>/dev/null || true)
    if [ -n "$xid" ]; then
      rpt=$(python3 "$DRIVER_PY" report "$gpui_xid" "$cont_xid" "$xid" 2>/dev/null || echo "{}")
      ok=$(python3 -c 'import json,sys; r=json.loads(sys.argv[1]); print(int(bool(r.get("in_gpui_tree")) and bool(r.get("in_container")) and (r.get("native",{}).get("painted") or 0) > 20))' "$rpt" 2>/dev/null || echo 0)
      if [ "$ok" = "1" ]; then
        echo "$xid"
        return 0
      fi
    fi
    sleep 0.25
  done
  echo "DIAG timeout waiting for embedded frame '$sub' (pid=$pid cont=$cont_xid); current X11 windows:" >&2
  python3 "$DRIVER_PY" list-windows >&2 || true
  return 1
}

# Stock KiCad 10.0.7 returns AS_UNHANDLED for GetDocumentModifiedState (added in 10.99-bp),
# but prefixes _NET_WM_NAME with '*' when dirty. Support both seamlessly.
is_doc_modified() {
  local sock=$1 kind=$2 win_xid=$3
  local ipc_res
  ipc_res=$(python3 "$IPC_PY" modified "$sock" "$kind" 2>/dev/null || echo '{"unhandled":true}')
  python3 - "$ipc_res" "$DRIVER_PY" "$win_xid" <<'PY'
import json, subprocess, sys
res = json.loads(sys.argv[1])
if not res.get("unhandled"):
    print(1 if res.get("modified") else 0)
    sys.exit(0)
drv, xid = sys.argv[2], int(sys.argv[3], 0)
wins = json.loads(subprocess.check_output([sys.executable, drv, "list-windows"], text=True))
for w in wins:
    if w["xid_int"] == xid:
        print(1 if w.get("title", "").lstrip().startswith("*") else 0)
        sys.exit(0)
print(0)
PY
}
