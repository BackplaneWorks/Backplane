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

EMBED_MODE=${EMBED_MODE:-spike}
OUT=${OUT:-$(mktemp -d "$BP_TMP/bp-kembed.XXXXXX")}
mkdir -p "$OUT"
OUT=$(cd "$OUT" && pwd)

# Discover Backplane client, hub, and assets when not explicitly passed.
discover_bins() {
  local cur_name cur_dir main_git
  if [ -z "${CLIENT_BIN:-}" ] || [ -z "${HUB_BIN:-}" ]; then
    if [ -f "$HOME/.backplane-gpui-dev/CURRENT" ]; then
      cur_name=$(cat "$HOME/.backplane-gpui-dev/CURRENT" 2>/dev/null || true)
      cur_dir="$HOME/.backplane-gpui-dev/builds/$cur_name"
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
  KICAD_CLI=${BACKPLANE_KICAD_CLI:-/home/h/.local/bin/kicad-cli}
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
  d=$((520 + $$ % 200))
  while [ -e "/tmp/.X11-unix/X$d" ] || [ -e "/tmp/.X$d-lock" ]; do d=$((d + 1)); done
  DISP=":$d"

  own env LD_LIBRARY_PATH="$xld" "$xvb" "$DISP" -screen 0 1800x1050x24 -nolisten tcp -xkbdir /usr/share/X11/xkb >"$OUT/xvfb.log" 2>&1
  for _ in $(seq 30); do [ -e "/tmp/.X11-unix/X$d" ] && break; sleep 0.1; done
  export DISPLAY="$DISP"

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
  HPORT=$(free_port)
  export HPORT
  own env \
    HOME="$OUT/fh" \
    XDG_CONFIG_HOME="$OUT/fh/.config" \
    XDG_CACHE_HOME="$OUT/fh/.cache" \
    XDG_RUNTIME_DIR="$OUT/run" \
    TMPDIR="$OUT/tmp" \
    BACKPLANE_AGENTS="$OUT/bin" \
    BACKPLANE_NO_UPDATE=1 \
    FAKE_LOG="$OUT/standin.log" \
    BP_STANDIN_MODE=stream \
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

finish_suite() {
  local suite_name=$1
  echo "$suite_name: passed=$PASS_COUNT failed=$FAIL_COUNT artifacts=$OUT"
  [ "$FAIL_COUNT" -eq 0 ]
}

DRIVER_PY="$KEMBED_LIB/x11_embed_driver.py"
IPC_PY="$KEMBED_LIB/gui_ipc_probe.py"
SEED_PY="$KEMBED_LIB/seed_project.py"
XINPUT_PY="$REPO_ROOT/test/cef/xinput.py"

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
