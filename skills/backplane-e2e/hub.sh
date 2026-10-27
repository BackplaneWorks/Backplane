#!/bin/sh
# An isolated Backplane hub for the user stories (SKILL.md): its own home,
# port and fixture projects, stand-in agents only, no tailnet, no update,
# no keyring. Never the person's ~/.backplane-bend.
#
#   hub.sh start NAME      start one; prints shell lines: PORT= TOKEN= URL=
#                          PAIR= HOME_DIR= PROJ= MULTI= THREAD= PID=
#   hub.sh long NAME [N]   a thread of N (80) exchanges in the fixture
#                          project (the history-paging stories); prints LONG=
#   hub.sh trial NAME      a workspace that looks used, to try things by hand
#                          or for stories that want one: 5 projects, ~20
#                          threads (3 long), some pinned, settled, snoozed,
#                          archived (~3 min; starts the hub if needed)
#   hub.sh stop NAME       stop it (the PID it started, nothing else)
#   hub.sh restart NAME    stop and start it again on the same home and port
#
# BP_E2E_BIN      backplane-serve to test (default: dist/backplane-serve, else
#                 build/dev/backplane-serve, from the checkout this is in)
# BP_E2E_WEB      its web client (default: the web/ folder beside the binary)
# BP_E2E_FIXTURE  a KiCad project folder (default /usr/share/kicad/demos/ecc83);
#                 its top-level KiCad files are copied, nothing else
# BP_E2E_ROOT     scratch root (default ${TMPDIR:-/tmp}/bp-e2e)
# BP_E2E_STREAM   stand-in reply: "N [MS]", deltas per turn (60) and ms apart (50)
set -eu
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
cmd=${1:-}; name=${2:-default}
root=${BP_E2E_ROOT:-${TMPDIR:-/tmp}/bp-e2e}/$name
bin=${BP_E2E_BIN:-}
[ -n "$bin" ] || { [ -x "$repo/dist/backplane-serve" ] && bin=$repo/dist/backplane-serve; } || true
[ -n "$bin" ] || bin=$repo/build/dev/backplane-serve
web=${BP_E2E_WEB:-$(dirname "$bin")/web}
fixture=${BP_E2E_FIXTURE:-/usr/share/kicad/demos/ecc83}
rpc() { node "$here/rpc.mjs" "$(cat "$root/port")" "$(cat "$root/home/token")" "$@"; }

copy_kicad() { # FROM TO [STEM]: the top-level KiCad files (the project's
  # own board, project and root sheet renamed to STEM; sub-sheets kept)
  mkdir -p "$2"
  own=$(for p in "$1"/*.kicad_pro; do [ -f "$p" ] && basename "$p" .kicad_pro; done | head -n 1)
  for f in "$1"/*.kicad_pcb "$1"/*.kicad_sch "$1"/*.kicad_pro "$1"/*.kicad_dru "$1"/*lib-table; do
    [ -f "$f" ] || continue
    b=$(basename "$f")
    if [ -n "${3:-}" ] && [ "${b%.*}" = "$own" ]; then b="$3.${b##*.}"; fi
    cp "$f" "$2/$b"
  done
}

seed() {
  [ -d "$fixture" ] || { echo "no fixture project at $fixture (set BP_E2E_FIXTURE)" >&2; exit 1; }
  copy_kicad "$fixture" "$root/proj"
  # several boards: two real ones, an archived copy and a build snapshot
  # (the board picker offers the two)
  copy_kicad "$fixture" "$root/multi/boards/alpha" alpha
  copy_kicad "$fixture" "$root/multi/boards/beta" beta
  copy_kicad "$fixture" "$root/multi/archive/alpha_revA" alpha
  mkdir -p "$root/multi/boards/alpha/build"
  cp "$root/multi/boards/alpha/alpha.kicad_pcb" "$root/multi/boards/alpha/build/before-reroute.kicad_pcb"
  printf '{"project": "boards/alpha/alpha.kicad_pro"}\n' > "$root/multi/.backplane.json"
}

start() {
  [ -x "$bin" ] || { echo "no hub binary at $bin (set BP_E2E_BIN)" >&2; exit 1; }
  if [ -f "$root/pid" ] && kill -0 "$(cat "$root/pid")" 2>/dev/null; then echo "already running: $root" >&2; exit 1; fi
  if [ ! -d "$root/home" ]; then
    mkdir -p "$root/home" "$root/fake" "$root/userhome"
    seed
    printf '#!/bin/sh\nexec bun "%s/test/tools/claude_standin.ts" "$@"\n' "$repo" > "$root/fake/claude"
    chmod +x "$root/fake/claude"
    python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1])' > "$root/port"
    fresh=1
  else fresh=0; fi
  port=$(cat "$root/port")
  set -- "$bin" --home "$root/home" --port "$port" --no-tailscale --foreground
  [ -d "$web" ] && set -- "$@" --web "$web"
  n=${BP_E2E_STREAM:-60}; ms=50
  case $n in *" "*) ms=${n#* }; n=${n%% *};; esac
  env HOME="$root/userhome" PATH="$root/fake:$PATH" DISPLAY= WAYLAND_DISPLAY= BACKPLANE_NO_UPDATE=1 BACKPLANE_PEERS= \
    BACKPLANE_AGENTS="$root/fake" BACKPLANE_VAULT_KEYRING=file BP_STANDIN_USAGE=1 BP_STANDIN_MODE=stream BP_STREAM_N="$n" \
    BP_STREAM_MS="$ms" BP_STREAM_SEP="
" "$@" > "$root/hub.log" 2>&1 &
  echo $! > "$root/pid"
  i=0; until curl -s -o /dev/null "http://127.0.0.1:$port/hello"; do i=$((i+1)); [ $i -lt 100 ] || { echo "hub did not start; see $root/hub.log" >&2; exit 1; }; sleep 0.2; done
  token=$(cat "$root/home/token")
  if [ $fresh = 1 ]; then
    rpc project.add "{\"path\":\"$root/proj\"}" | python3 -c 'import json,sys;print(json.load(sys.stdin).get("thread",""))' > "$root/thread"
    rpc project.add "{\"path\":\"$root/multi\"}" > /dev/null
  fi
  start_lines
}

start_lines() {
  port=$(cat "$root/port"); token=$(cat "$root/home/token")
  echo "PORT=$port"
  echo "TOKEN=$token"
  echo "URL=http://127.0.0.1:$port/#token=$token"
  echo "PAIR=backplane://pair?url=http://10.0.2.2:$port/%23token=$token"
  echo "HOME_DIR=$root/home"
  echo "PROJ=$root/proj"
  echo "MULTI=$root/multi"
  echo "THREAD=$(cat "$root/thread")"
  echo "PID=$(cat "$root/pid")"
}

stop() {
  [ -f "$root/pid" ] || return 0
  p=$(cat "$root/pid")
  case "$(tr '\0' ' ' < "/proc/$p/cmdline" 2>/dev/null)" in *"--home $root/home "*) kill "$p" ;; esac
  i=0; while kill -0 "$p" 2>/dev/null && [ $i -lt 50 ]; do sleep 0.1; i=$((i+1)); done
  rm -f "$root/pid"
}

long() { # N exchanges queued; the stand-in answers each in ~3 s (~4 min for 80)
  n=${3:-80}
  t=$(rpc thread.create "{\"project\":\"$(rpc_project)\",\"title\":\"Long history\"}" | python3 -c 'import json,sys;print(json.load(sys.stdin).get("thread",""))')
  i=1
  while [ $i -le "$n" ]; do
    rpc turn.start "{\"thread\":\"$t\",\"text\":\"mode:plain Question $i\",\"msg\":\"e2e-$t-$i\",\"mode\":\"\"}" > /dev/null || true
    sleep 0.3; i=$((i+1))
  done
  echo "LONG=$t"
}
trial() {
  [ -f "$root/pid" ] && kill -0 "$(cat "$root/pid")" 2>/dev/null || start > /dev/null
  for d in amp sensor-node psu; do copy_kicad "$fixture" "$root/$d" "$d"; done
  node "$here/trial.mjs" "$(cat "$root/port")" "$(cat "$root/home/token")" "$root/home" "$root" > "$root/trial"
  start_lines
}
rpc_project() { grep -o '"ProjectCreated","id":"[^"]*","title":"proj"' "$root/home/events.jsonl" | head -1 | sed 's/.*"id":"\([^"]*\)".*/\1/'; }

case $cmd in
  start) start ;;
  stop) stop ;;
  restart) stop; start ;;
  long) long "$@" ;;
  trial) trial ;;
  *) sed -n '2,24p' "$0"; exit 2 ;;
esac
