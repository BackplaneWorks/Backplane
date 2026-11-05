#!/bin/bash
# Run the Rust hub + GPUI desktop beside today's Backplane, on a private COPY of a store, on its own port and home.
#
#   scripts/gpui/switch.sh try [--no-client]     seeded synthetic home: a first thing to click on, nothing real in it
#   scripts/gpui/switch.sh start [--no-client]   fresh copy, hub (+ policy helper), then the GPUI client
#   scripts/gpui/switch.sh stop                  stop what start made; the copy stays (to look at, or to start again on)
#   scripts/gpui/switch.sh reload [hub|client|both]  the binaries in HUB_BIN/CLIENT_BIN on the SAME copy: nothing copied, nothing deleted (dev-reload.sh runs it)
#   scripts/gpui/switch.sh status                what runs, memory of each process, counts only
#   scripts/gpui/switch.sh rollback              stop and delete the copy. There is nothing to undo: today's app never stopped.
#
# What it never does: touch the live hub (3787/3788), ~/.backplane-bend, ~/.local/share/backplane, or any tailnet. The copy is
# made by reading the source file; nothing is written to the source. Agents are stand-ins only (no model, no credentials: the hub
# runs with a fake HOME and BACKPLANE_COPY=1, which makes every outside-reaching action a no-op, docs/gpui/COPY-GUARD.md).
#
# Environment (all optional):
#   SWITCH_SOURCE   `start`: the store to copy, a folder holding events.jsonl, REQUIRED and synthetic (real stores are refused: the hold).
#                   `side` reads the live store only through side-by-side.sh (typed yes), and not while HOLD=1.
#   `try` (no copy at all): a seeded synthetic home (3 projects, 12 threads), stand-in agents, nothing real in it.
#   SWITCH_PORT     default 3790 (3787, 3788, 3799, 3821 are refused: the live hub, its dev hub, the shared test hub)
#   SWITCH_DIR      state: home, logs, pids. Default ~/.backplane-gpui-switch (mode 700; never under the repo)
#   SWITCH_MEM      the scope's MemoryMax, default 3700M (swap is always 0). Never raise it to make a run fit; report instead.
#   SWITCH_DISPLAY  where the client opens. Required (no default): your own screen's, or :61 (build/gpui/xvfb) for tests.
#   HUB_BIN POLICY_BIN CLIENT_BIN  by absolute path; default the main checkout's build/gpui/{hub/prov/backplane-hub-full,
#                   slim/bp-policy-bp,linux/backplane-gpui}
#
# SWITCHING BACK. Starting this changes nothing of today's app, so going back is `rollback` and nothing else. Lost: the threads,
# messages and settings made on the copy (they live only in its own log). NOT undone: files an agent changed in a real folder (in the
# side-by-side mode agents run in real folders; with stand-ins only, none). Nothing flows back into ~/.backplane-bend, and nothing done
# in today's app after the copy was made shows up here (a start makes a fresh copy every time).
set -eu
ROOT=${GPUI_MAIN:-/home/h/Documents/i2cjak/backplane-gpui}
HERE=$(cd "$(dirname "$0")/../.." && pwd)
SW=${SWITCH_DIR:-$HOME/.backplane-gpui-switch}
PORT=${SWITCH_PORT:-3790}
MEM=${SWITCH_MEM:-3700M}
# `start` needs an explicit source and it must be synthetic: no default store (the hold, below)
SRC=${SWITCH_SOURCE:-}
HUB_BIN=${HUB_BIN:-$ROOT/build/gpui/hub/prov/backplane-hub-full}
POLICY_BIN=${POLICY_BIN:-$ROOT/build/gpui/slim/bp-policy-bp}
# the "live" store: ~/.backplane-bend, or SWITCH_LIVE=<dir> (a synthetic one, for tests; the real one still needs SWITCH_ALLOW_LIVE=person)
LIVE=${SWITCH_LIVE:-$HOME/.backplane-bend}
# the newest commit-named client build (client-<10 hex>), with its assets beside it; CLIENT_BIN overrides
DB=$ROOT/build/gpui/desktop-bin
# build/gpui/desktop-bin/CURRENT names the client to use (one line: a file name beside it), set by whoever lands a newer build
CLIENT_BIN=${CLIENT_BIN:-$( [ -f "$DB/CURRENT" ] && echo "$DB/$(head -1 "$DB/CURRENT")" || ls -t "$DB"/client-* 2>/dev/null | grep -E '/client-[0-9a-f]{9,10}$' | head -1)}
[ -d "$DB/assets" ] && export BACKPLANE_GPUI_ASSETS=${BACKPLANE_GPUI_ASSETS:-$DB/assets}
DISP=${SWITCH_DISPLAY:-}   # required for the client: never defaulted from $DISPLAY (an agent window must not land on a screen by accident)
H=$SW/home
MODE=copy

die() { echo "switch: $*" >&2; exit 1; }
alive() { [ -n "${1:-}" ] && kill -0 "$1" 2>/dev/null; }
pidof_file() { [ -f "$SW/$1.pid" ] && cat "$SW/$1.pid" || true; }
# a pid file is trusted only while /proc/<pid>/cmdline still names our binary and our home
# The binary is the one RECORDED at launch ($SW/hub.bin, $SW/client.bin), never re-derived from CURRENT or the environment at stop time.
ours() { local c b; c=$(tr '\0' ' ' < "/proc/$1/cmdline" 2>/dev/null) || return 1
  b=$(cat "$SW/$2.bin" 2>/dev/null) || b=
  # a session an older script started has no .bin: fall back to the identity check on the binary this script would use
  if [ -z "$b" ]; then case "$2" in hub) b=$HUB_BIN;; client) b=$CLIENT_BIN;; esac; fi
  [ -n "$b" ] || return 1
  case "$c" in *"$b"*"$H"*) return 0;; esac; return 1; }
owned() { [ -f "$SW/.switch-owned" ]; }
rss_mb() { awk '/^VmRSS/{printf "%d", $2/1024}' "/proc/$1/status" 2>/dev/null || echo "?"; }
hwm_mb() { awk '/^VmHWM/{printf "%d", $2/1024}' "/proc/$1/status" 2>/dev/null || echo "?"; }
helper_pid() { local h; h=$(pidof_file hub); alive "$h" && ours "$h" hub && pgrep -P "$h" 2>/dev/null | head -1 || true; }

# THE HOLD (coordinator's, 2026-11-05): no hub on a copy of any real log. Lifted only by a reviewed commit that sets HOLD=0; even then a real store is
# reached only by `side` with SWITCH_ALLOW_LIVE=person, which side-by-side.sh sets after the person's typed yes. Test variables (SWITCH_SOURCE,
# SWITCH_LIVE, SWITCH_STANDIN) may point only at synthetic stores.
HOLD=1
real_store() {   # is this path (after realpath) a real store?
  local p r; p=$(realpath -m "$1") || return 0
  for r in "$HOME/.backplane-bend" "$HOME/.backplane-gpui-copy" "$HOME/.backplane-gpui" "$HOME/.local/share/backplane" "$HOME/.backplane-gpui-scratch/store-copy-backup"* "$HOME/.backplane-gpui-scratch/person-"*; do
    r=$(realpath -m "$r"); case "$p" in "$r"|"$r"/*) return 0;; esac
  done
  return 1
}
guard_source() {
  local d
  if [ "$MODE" = try ]; then return 0; fi
  if [ "$MODE" = side ]; then
    d=$LIVE
    if [ "${SWITCH_STANDIN:-}" = 1 ]; then   # stand-ins: synthetic only, whatever else is set
      [ -n "${SWITCH_LIVE:-}" ] || die "SWITCH_STANDIN needs SWITCH_LIVE=<a synthetic store>"
      ! real_store "$d" || die "SWITCH_STANDIN with a real store ($d) is refused: test variables may point only at synthetic logs"
    else
      [ "${SWITCH_ALLOW_LIVE:-}" = person ] || die "side mode needs the person's typed yes (SWITCH_ALLOW_LIVE=person, set by side-by-side.sh): refused"
      if real_store "$d"; then
        [ "$(realpath -m "$d")" = "$(realpath -m "$HOME/.backplane-bend")" ] || die "side mode reads only ~/.backplane-bend as a real store; $d is another real store: refused"
        # the env var alone is not consent: a real store also needs a terminal here, not only in side-by-side.sh
        { [ -t 0 ] && [ -t 1 ]; } || die "side on a real store needs a terminal (the person's own): refused"
        [ "$HOLD" = 0 ] || die "the hold: no hub on a copy of a real log yet (HOLD=1 in switch.sh, lifted by a reviewed commit)"
      fi
    fi
  else   # start: an explicit synthetic source
    [ -n "$SRC" ] || die "start needs SWITCH_SOURCE=<a synthetic store folder>: no default store (the hold)"
    if [ "$SRC" = live ]; then die "start does not read the live store; use side-by-side.sh"; fi
    ! real_store "$SRC" || die "the hold: $SRC is a real store; start takes only a synthetic source"
  fi
}

guards() {   # `guards source` (start only) adds the source/hold check; rollback and stop never read a source
  [ "${1:-}" != source ] || guard_source
  case "$PORT" in 3787|3788|3799|3821) die "port $PORT belongs to another hub";; esac
  case "$PORT" in ''|*[!0-9]*) die "bad port";; esac
  case "$SW" in "$HOME/.backplane-bend"*|"$HOME/.local/share/backplane"*|"$ROOT"*|"$HERE"*) die "state dir $SW is not allowed there";; esac
  case "$SW" in ""|"/"|"$HOME"|"$HOME/") die "state dir $SW is not allowed";; esac
  [ -x "$HUB_BIN" ] || die "no hub binary $HUB_BIN"
  # a hub without --policy-bin (Bend off the runtime) needs no helper; one with it does
  if "$HUB_BIN" --help 2>&1 | grep -q -- --policy-bin; then USE_POLICY=1; [ -x "$POLICY_BIN" ] || die "no policy helper $POLICY_BIN"; else USE_POLICY=0; fi
}

source_file() {
  if [ "$SRC" = live ]; then echo "$LIVE/events.jsonl"; else echo "$SRC/events.jsonl"; fi
}

# a small synthetic home: 3 projects (folders under $SW/proj), 12 threads with a few messages each; nothing from any real store
seed() {
  python3 - "$H" "$SW/proj" <<'PY'
import json,sys,os,time
H,P=sys.argv[1],sys.argv[2]; N=int(time.time()); ev=[]
for i,name in enumerate(["amp-board","sensor-node","mech-bracket"]):
    os.makedirs(f"{P}/{name}",exist_ok=True); ev.append({"$":"ProjectCreated","id":f"p{i}","title":name,"root":f"{P}/{name}","at":N-86400*5+i})
titles=["Route the USB pair","Pick a 3V3 regulator","Review the BOM","Fix DRC clearance errors","Add test points","Place decoupling caps","Bracket fit check","Why does ERC flag U3?","Panelize for 5 boards","Silk cleanup","Datasheet summary","Stackup question"]
for k,t in enumerate(titles):
    tid=f"t-seed-{k}"; at=N-86400*4+k*3600
    ev.append({"$":"ThreadCreated","id":tid,"project":f"p{k%3}","title":t,"env":"local","branch":"","worktree":"","provider":"claude","model":"","at":at})
    for j in range(4):
        ev.append({"$":"MessagePosted","thread":tid,"msg":f"m-seed-{k}-{j}","role":"user" if j%2==0 else "assistant","text":(f"{t}: question {j//2+1}" if j%2==0 else f"Here is a worked answer for \"{t}\".\n\n- first point\n- second point\n\nSay the word and I will apply it."),"at":at+60*(j+1)})
ev.append({"$":"SettingSet","key":"restart.continue","value":"off","at":N})
open(f"{H}/events.jsonl","w").write("".join(json.dumps(e,separators=(",",":"))+"\n" for e in ev))
PY
}

# the memory bar is HUB RSS only (not the helper, not agents): the peak while starting and the last reading, from the sampler's column 2
rss_summary() {
  [ -f "$SW/rss.tsv" ] || return 0
  awk 'NF>=2 && $2!="" {n++; if ($2>m) m=$2; l=$2} END {if (n) printf "hub RSS: peak %d MB, last %d MB (%d samples; the bar: startup under 1000 MB, at rest under 500 MB)\n", m, l, n}' "$SW/rss.tsv"
}
# every run's logs outlive its cleanup: copied (never the store) to ~/.backplane-gpui-switch-logs/<time>-<mode>/ at each stop and rollback
keeplogs() {
  local f d; [ -d "$SW" ] || return 0; ls "$SW"/*.log "$SW"/*.tsv >/dev/null 2>&1 || return 0
  d=${SWITCH_LOGS:-$HOME/.backplane-gpui-switch-logs}/$(date +%Y%m%d-%H%M%S)-$MODE; mkdir -p "$d"; chmod 700 "$d"
  for f in "$SW"/*.log "$SW"/*.tsv; do [ -f "$f" ] && mv "$f" "$d/"; done; echo "logs kept in $d"
}

cmd_stop() {
  local who p
  for who in client hub; do
    p=$(pidof_file "$who")
    if alive "$p"; then ours "$p" "$who" && kill "$p" 2>/dev/null || echo "pid $p is not our $who (stale pid file): left alone"; fi
  done
  # the helper is the hub's child and ends with it (--parent-pid); wait for both to go, then leave no pid files
  local i=0
  while [ $i -lt 20 ]; do
    alive "$(pidof_file hub)" || alive "$(pidof_file client)" || break
    sleep 0.5; i=$((i+1))
  done
  for who in client hub; do p=$(pidof_file $who); alive "$p" && ours "$p" $who && die "$who ($(pidof_file $who)) did not stop; look at it, I kill nothing else"; rm -f "$SW/$who.pid"; done
  touch "$SW/sample.stop"; sleep 1; rss_summary; keeplogs
  echo "stopped. The copy is kept in $H (rollback deletes it)."
}

cmd_start() {
  guards source
  if [ -e "$SW" ] && ! owned && [ -n "$(ls -A "$SW" 2>/dev/null)" ]; then die "$SW exists and this script did not make it (no .switch-owned): refusing to use or delete it"; fi
  [ -d "$SW" ] && keeplogs
  mkdir -p "$SW"; chmod 700 "$SW"; echo "backplane switch.sh state; rollback may delete $H" > "$SW/.switch-owned"
  echo "$MODE" > "$SW/mode"
  alive "$(pidof_file hub)" && ours "$(pidof_file hub)" hub && die "already running (pid $(pidof_file hub)); stop first"
  local src=; if [ "$MODE" = try ]; then :; else src=$(source_file); [ -f "$src" ] || die "no $src"; fi
  [ -z "$(ss -ltn "sport = :$PORT" 2>/dev/null | tail -n +2)" ] || die "port $PORT is in use"
  # disk: the copy must leave 20 GB free
  local need=1 free; [ -z "$src" ] || need=$(( $(stat -c %s "$src") / 1048576 )); free=$(df -Pm / | awk 'NR==2{print $4}')
  [ $((free - need)) -ge 20480 ] || die "copy needs ${need} MB and / has ${free} MB free: under 20 GB would remain; tell the coordinator"
  # a fresh private copy: only the log (no tokens, no secrets, no vault); the source is only read
  [ "$H" = "$SW/home" ] && rm -rf "$H"; mkdir -p "$H" "$SW/bin" "$SW/fakehome/.cache" "$SW/proj"; chmod 700 "$H"
  if [ "$MODE" = try ]; then seed; echo "seeded $(wc -l < "$H/events.jsonl") events (synthetic)"; else
  if [ "$SRC" = live ]; then  # the live hub keeps appending: take the size now, cut to the last whole line
    head -c "$(stat -c %s "$src")" "$src" > "$H/events.jsonl"
    python3 - "$H/events.jsonl" <<'PY'
import sys,os
p=sys.argv[1]; f=open(p,'r+b'); n=os.path.getsize(p); back=min(n,1<<20); f.seek(n-back); tail=f.read(back); k=tail.rfind(b"\n")
if k>=0 and k!=len(tail)-1: f.truncate(n-back+k+1)
PY
  else cp --reflink=auto "$src" "$H/events.jsonl"; fi
  chmod 600 "$H/events.jsonl"
  # a copy must not resume the turns its log says were running (they would run in the real folders)
  printf '{"$":"SettingSet","key":"restart.continue","value":"off","at":%s}\n' "$(date +%s)" >> "$H/events.jsonl"
  echo "copied $(wc -l < "$H/events.jsonl") events ($need MB) from $([ "$SRC" = live ] && echo 'the live store (read only)' || echo "$SRC")"; fi
  launch_hub; idle_check; launch_client "$@"
}

# the hub, on what is in $H. The stand-ins and every environment check run at each launch, a reload's too.
launch_hub() {
  # stand-in agents: nothing may call a model
  printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun)" "$HERE/test/tools/claude_standin.ts" > "$SW/bin/claude"; chmod +x "$SW/bin/claude"
  local n; for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > "$SW/bin/$n"; chmod +x "$SW/bin/$n"; done
  # the hub in its own capped scope, a fake HOME (no real credentials), copy guard on, loopback only
  local pol=(); [ "$USE_POLICY" = 0 ] || pol=(--policy-bin "$POLICY_BIN")
  local envx; if [ "$MODE" = side ]; then
    # the hub itself has the FAKE home; the real one goes only to a person's-turn child (BACKPLANE_PERSON_HOME, which the hub strips from
    # that child with every other BACKPLANE_*); BACKPLANE_LIVE_LOG is read-only, for the same-folder check; no BACKPLANE_AGENTS
    # a test on a synthetic live store may use stand-ins (SWITCH_STANDIN=1); against the real store never
    if [ "${SWITCH_STANDIN:-}" = 1 ] && [ -n "${SWITCH_LIVE:-}" ] && [ "$(realpath -m "$LIVE")" != "$(realpath -m "$HOME/.backplane-bend")" ]; then SBS_AGENTS=1; else SBS_AGENTS=0; fi
    envx=(HOME="$SW/fakehome" XDG_CACHE_HOME="$SW/fakehome/.cache" BACKPLANE_PERSON_HOME="$HOME" BACKPLANE_LIVE_LOG="$LIVE/events.jsonl" BACKPLANE_COPY=1 BACKPLANE_SIDE_BY_SIDE=1 BACKPLANE_PASSIVE=1 BACKPLANE_NO_UPDATE=1)
    [ "$SBS_AGENTS" = 0 ] || envx+=(BACKPLANE_AGENTS="$SW/bin" BP_STANDIN_MODE=stream)
  else envx=(HOME="$SW/fakehome" XDG_CACHE_HOME="$SW/fakehome/.cache" BACKPLANE_COPY=1 BACKPLANE_AGENTS="$SW/bin" BACKPLANE_NO_UPDATE=1 BP_STANDIN_MODE=stream); fi
  ( cd "$SW/proj" && exec setsid env -i PATH="$PATH" LC_ALL=C XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}" DBUS_SESSION_BUS_ADDRESS="${DBUS_SESSION_BUS_ADDRESS:-unix:path=/run/user/$(id -u)/bus}" "${envx[@]}" \
      systemd-run --user --scope -q -p "MemoryMax=$MEM" -p MemorySwapMax=0 -- \
      env -u DBUS_SESSION_BUS_ADDRESS -u XDG_RUNTIME_DIR "$HUB_BIN" --home "$H" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground "${pol[@]}" \
      > "$SW/hub.log" 2>&1 < /dev/null ) &
  echo $! > "$SW/hub.pid"; echo "$HUB_BIN" > "$SW/hub.bin"
  local hp; hp=$(cat "$SW/hub.pid") i=0
  # before anything else: the stand-ins and the copy guard are in the hub's own environment
  sleep 1
  alive "$hp" || { tail -3 "$SW/hub.log"; die "the hub exited at once"; }
  local ev; ev=$(tr '\0' '\n' < "/proc/$hp/environ")
  local okenv=1; printf '%s\n' "$ev" | grep -qx "BACKPLANE_COPY=1" || okenv=0
  printf '%s\n' "$ev" | grep -q '^DBUS_SESSION_BUS_ADDRESS=\|^XDG_RUNTIME_DIR=' && okenv=0
  if [ "$MODE" = side ]; then printf '%s\n' "$ev" | grep -qx "BACKPLANE_SIDE_BY_SIDE=1" || okenv=0; printf '%s\n' "$ev" | grep -qx "BACKPLANE_PASSIVE=1" || okenv=0; printf '%s\n' "$ev" | grep -qx "HOME=$SW/fakehome" || okenv=0; [ "$SBS_AGENTS" = 1 ] || { printf '%s\n' "$ev" | grep -q '^BACKPLANE_AGENTS=' && okenv=0; }
  else printf '%s\n' "$ev" | grep -qx "BACKPLANE_AGENTS=$SW/bin" || okenv=0; fi
  [ $okenv = 1 ] || { kill "$hp" 2>/dev/null; rm -f "$SW/hub.pid"; die "hub $hp's environment is not the one required for mode $MODE (copy guard, stand-ins or side-by-side flag, no session bus): stopped"; }
  rm -f "$SW/sample.stop"; if [ "${RELOADING:-}" != 1 ]; then "$HERE/test/gpui/switch/sample.sh" "$SW" "$SW/rss.tsv" 5 < /dev/null > /dev/null 2>&1 & fi   # it reads hub.pid each round: a reload keeps it. One command: a list here would keep this script's stdout open
  # a big log takes minutes to fold; wait for /hello, bounded
  echo "hub pid $hp, environment checked (stand-ins, copy guard). Waiting for it to open the log..."
  while [ $i -lt "${SWITCH_WAIT:-1800}" ]; do
    alive "$hp" || { tail -5 "$SW/hub.log"; journalctl --user --since "-3min" 2>/dev/null | grep "run-p$hp" | cut -c1-200 | tail -4; die "the hub exited after ${i}s while opening (see $SW/hub.log; the journal lines above say how: signal, exit code, memory peak)"; }
    curl -s -m2 -o /dev/null "http://127.0.0.1:$PORT/hello" && break
    sleep 1; i=$((i+1))
  done
  curl -s -m2 -o /dev/null "http://127.0.0.1:$PORT/hello" || die "no /hello after ${i}s; hub still running as pid $hp, see $SW/hub.log"
  echo "hub answers on http://127.0.0.1:$PORT after ${i}s"
}

idle_check() {
  if [ "$MODE" = side ]; then  # the first-run check: nothing may act by itself; 30 s idle may add only a few settings, by count
    local l0 l1; l0=$(wc -l < "$H/events.jsonl"); sleep 30; l1=$(wc -l < "$H/events.jsonl")
    echo "idle 30 s: the copy's log grew by $((l1 - l0)) events"
    [ $((l1 - l0)) -le 50 ] || { cmd_stop >/dev/null; die "the copy's log grew by $((l1 - l0)) events while idle: something acts by itself; stopped (logs kept)"; }
  fi
}

launch_client() {
  if [ "${1:-}" != "--no-client" ]; then
    [ -x "$CLIENT_BIN" ] || die "no client $CLIENT_BIN (use --no-client to run the hub alone)"
    [ -n "$DISP" ] || die "no display: set SWITCH_DISPLAY (:61 is the isolated one) or pass --no-client"
    local sw=; case "${VK_ICD_FILENAMES:-}" in *lvp*) sw="BACKPLANE_GPUI_RECTS=1 LP_NUM_THREADS=2";; esac  # software Vulkan draws only in rects mode
    ( exec setsid env $sw DISPLAY="$DISP" ${VK_ICD_FILENAMES:+VK_ICD_FILENAMES="$VK_ICD_FILENAMES"} ${BACKPLANE_GPUI_ASSETS:+BACKPLANE_GPUI_ASSETS="$BACKPLANE_GPUI_ASSETS"} HOME="$HOME" \
        systemd-run --user --scope -q -p "MemoryMax=$MEM" -p MemorySwapMax=0 -- \
        "$CLIENT_BIN" --url "http://127.0.0.1:$PORT" --token-file "$H/token" ${SWITCH_CLIENT_ARGS:-} > "$SW/client.log" 2>&1 < /dev/null ) &
    echo $! > "$SW/client.pid"; echo "$CLIENT_BIN" > "$SW/client.bin"; echo "client pid $(cat "$SW/client.pid") on $DISP"
  fi
  echo "Running beside today's Backplane (untouched on 3787). To go back: scripts/gpui/switch.sh rollback"
}

# one process stopped and only that one. A TERM asks; a client saves where the person is and quits, and a second TERM ends one too stuck to.
stop_one() {
  local who=$1 p i=0; p=$(pidof_file "$who")
  if alive "$p"; then
    ours "$p" "$who" || die "pid $p is not our $who (stale pid file): left alone"
    kill "$p" 2>/dev/null || true
    while alive "$p" && [ $i -lt 60 ]; do sleep 0.25; i=$((i+1)); [ $i != 16 ] || kill "$p" 2>/dev/null || true; done
    ! alive "$p" || die "$who ($p) did not stop; look at it, I kill nothing else"
  fi
  rm -f "${SW:?}/$who.pid"; [ ! -f "$SW/$who.log" ] || mv -f "$SW/$who.log" "$SW/$who.log.prev"
}

# what a running or last session was: `mode` is written at start; a session an older script started says so in its hub's environment
mode_of() {
  local h; h=$(pidof_file hub)
  if [ -f "$SW/mode" ]; then cat "$SW/mode"
  elif alive "$h" && tr '\0' '\n' < "/proc/$h/environ" 2>/dev/null | grep -qx BACKPLANE_SIDE_BY_SIDE=1; then echo side
  elif alive "$h"; then echo copy
  else die "no mode in $SW and no hub running: start one first"; fi
}

# new binaries on the same copy. This never makes or deletes a copy, so what was done on it stays. Every check a start makes still holds: on a real store
# guard_source wants the person's yes and a terminal, and the hub's environment is checked again. The window opens as soon as the hub answers; the idle check runs after it.
cmd_reload() {
  local what=${1:-both}; case "$what" in hub|client|both) ;; *) die "reload [hub|client|both]";; esac
  owned || die "$SW has no .switch-owned: reload works only on a copy this script made"
  [ -f "$H/events.jsonl" ] || die "no copy in $H: reload never makes one; start first"
  MODE=$(mode_of); case "$MODE" in side) SRC=live; guards source;; *) guards;; esac
  RELOADING=1
  if [ "$what" != client ]; then stop_one hub; launch_hub; fi
  if [ "$what" != hub ]; then stop_one client; launch_client; fi
  [ "$what" = client ] || idle_check
}

cmd_status() {
  local hp cp pp; hp=$(pidof_file hub); cp=$(pidof_file client); pp=$(helper_pid)
  if alive "$hp"; then
    echo "hub    pid $hp  rss $(rss_mb "$hp") MB (peak $(hwm_mb "$hp"))"
    [ -n "$pp" ] && echo "helper pid $pp  rss $(rss_mb "$pp") MB (peak $(hwm_mb "$pp"))"
    alive "$cp" && echo "client pid $cp  rss $(rss_mb "$cp") MB"
    echo "port   $PORT: $(curl -s -m3 -o /dev/null -w '/hello %{http_code}' "http://127.0.0.1:$PORT/hello" || echo 'no answer')"
    [ -f "$H/events.jsonl" ] && echo "copy   $(wc -l < "$H/events.jsonl") events, $(du -sm "$H" | cut -f1) MB"
  else
    echo "not running"; [ -d "$H" ] && echo "a copy is kept in $H"
  fi
  echo "today's Backplane (3787): $(curl -s -m3 -o /dev/null -w '/hello %{http_code}' http://127.0.0.1:3787/hello || echo 'no answer') (read only probe)"
}

cmd_rollback() {
  guards
  owned || die "$SW has no .switch-owned: this script did not make it, so it deletes nothing there"
  cmd_stop
  [ "$H" = "$SW/home" ] && [ -d "$H" ] && rm -rf "$H" && echo "copy deleted."
  echo "Nothing to undo: today's app (3787, ~/.backplane-bend) was never stopped or changed."
  echo "Lost by going back: the threads, messages and settings made on the copy. Files an agent changed in a real folder stay changed (the copy keeps no history of them; use git)."
}

# selftest: try mode on a display, a scripted select+type+send, a real screenshot of the live window, FAILS on a flat or near-flat picture
# (fewer than 50 colours) or if the send did not reach the log. Starts its own Xvfb on SWITCH_DISPLAY if none; always rolls back; logs and shot kept.
cmd_selftest() {
  [ -n "$DISP" ] || die "selftest needs SWITCH_DISPLAY (an isolated one, e.g. :61)"
  # never a screen that exists: `import -window root` would save the whole screen of whoever is on it. The selftest makes its own Xvfb.
  [ "$DISP" != "${DISPLAY:-}" ] || die "SWITCH_DISPLAY is your own \$DISPLAY ($DISP): refused (the screenshot is of the whole screen)"
  local n=${DISP#:}; [ ! -e "/tmp/.X11-unix/X$n" ] || die "display $DISP already exists (someone's screen or another lane's): pick a free number, the selftest starts its own"
  XP=; IP=; RC=0; DONE=0
  local out=${SWITCH_LOGS:-$HOME/.backplane-gpui-switch-logs}/shots; mkdir -p "$out"
  # whatever fails below, the hub, the client and the Xvfb are stopped (only the pids this function started)
  trap 'selftest_cleanup' EXIT INT TERM
  local X=$ROOT/build/gpui/xvfb; [ -x "$X/usr/bin/Xvfb" ] || { echo "no Xvfb in $X"; RC=1; return 1; }
  ( LD_LIBRARY_PATH=$X/usr/lib/x86_64-linux-gnu exec "$X/usr/bin/Xvfb" "$DISP" -screen 0 1440x900x24 -nolisten tcp -xkbdir /usr/share/X11/xkb ) > /dev/null 2>&1 < /dev/null & XP=$!
  sleep 2
  if command -v i3 >/dev/null; then printf 'font pango:monospace 8\nnew_window none\nfor_window [class=".*"] border none\ndefault_border none\nhide_edge_borders both\n' > "$SW.i3.conf"
    ( DISPLAY=$DISP exec i3 -c "$SW.i3.conf" ) > /dev/null 2>&1 < /dev/null & IP=$!; sleep 1; fi
  mkdir -p "$SW"; echo '[{"select":0},{"wait":3000},{"type":"selftest message"},{"wait":300},{"send":true},{"wait":30000}]' > "$SW.selftest.json"
  SWITCH_CLIENT_ARGS="--script $SW.selftest.json" cmd_start || RC=1
  if [ $RC = 0 ]; then
    local f="$out/$(date +%Y%m%d-%H%M%S)-selftest.png" l0 l1 colours=0
    l0=$(wc -l < "$H/events.jsonl") || true; sleep 16
    DISPLAY=$DISP import -window root "$f" || { echo "selftest FAILED: no screenshot"; RC=1; }
    l1=$(wc -l < "$H/events.jsonl") || true
    if [ -s "$f" ]; then colours=$(python3 -I -W ignore - "$f" <<'PY' || echo 0
import sys
from PIL import Image
im=Image.open(sys.argv[1]).convert('RGB'); print(len(set(im.getdata())))
PY
); fi
    rss_summary
    echo "selftest: shot $f has ${colours:-0} colours; the log has ${l1:-?} events (${l0:-?} before the wait)"
    [ "${colours:-0}" -ge 50 ] 2>/dev/null || { echo "selftest FAILED: the window is flat (under 50 colours)"; RC=1; }
    [ "$(grep -c 'selftest message' "$H/events.jsonl" 2>/dev/null)" -ge 1 ] || { echo "selftest FAILED: the sent message never reached the log"; RC=1; }
  fi
  DONE=1; return $RC
}
selftest_cleanup() {
  trap - EXIT INT TERM
  cmd_rollback > /dev/null 2>&1 || true
  [ -z "${IP:-}" ] || kill "$IP" 2>/dev/null || true; [ -z "${XP:-}" ] || kill "$XP" 2>/dev/null || true
  rm -f "$SW.i3.conf" "$SW.selftest.json"
  [ "${RC:-1}" = 0 ] && [ "${DONE:-0}" = 1 ] && echo "selftest ok" || echo "selftest FAILED (logs in ${SWITCH_LOGS:-$HOME/.backplane-gpui-switch-logs})"
}

case "${1:-}" in
  selftest) MODE=${2:-try}; case "$MODE" in try) ;; side) [ -n "${SWITCH_LIVE:-}" ] || die "selftest side needs SWITCH_LIVE=<a synthetic live store>"; SRC=live; SWITCH_STANDIN=1;; *) die "selftest [try|side]";; esac; cmd_selftest && rc=0 || rc=$?; selftest_cleanup; exit $rc;;
  start) shift; MODE=copy; cmd_start "$@";;
  try) shift; MODE=try; cmd_start "$@";;
  side) shift; MODE=side; SRC=live; cmd_start "$@";;
  reload) shift; cmd_reload "$@";;
  stop) cmd_stop;;
  status) cmd_status;;
  rollback) cmd_rollback;;
  *) sed -n '2,10p' "$0"; exit 2;;
esac
