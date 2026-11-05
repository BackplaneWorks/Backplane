#!/bin/bash
# Run the Rust hub + GPUI desktop beside today's Backplane, on a private COPY of a store, on its own port and home.
#
#   scripts/gpui/switch.sh try [--no-client]     seeded synthetic home: a first thing to click on, nothing real in it
#   scripts/gpui/switch.sh start [--no-client]   fresh copy, hub (+ policy helper), then the GPUI client
#   scripts/gpui/switch.sh stop                  stop what start made; the copy stays (to look at, or to start again on)
#   scripts/gpui/switch.sh status                what runs, memory of each process, counts only
#   scripts/gpui/switch.sh rollback              stop and delete the copy. There is nothing to undo: today's app never stopped.
#
# What it never does: touch the live hub (3787/3788), ~/.backplane-bend, ~/.local/share/backplane, or any tailnet. The copy is
# made by reading the source file; nothing is written to the source. Agents are stand-ins only (no model, no credentials: the hub
# runs with a fake HOME and BACKPLANE_COPY=1, which makes every outside-reaching action a no-op, docs/gpui/COPY-GUARD.md).
#
# Environment (all optional):
#   SWITCH_SOURCE   the store to copy: a folder holding events.jsonl. Default ~/.backplane-gpui-copy (the verified copy).
#                   `live` reads ~/.backplane-bend/events.jsonl, read-only, and only with SWITCH_ALLOW_LIVE=person (the person's call).
#   `try` (no copy at all): a seeded synthetic home (3 projects, 12 threads), stand-in agents, nothing real in it.
#   SWITCH_PORT     default 3790 (3787, 3788, 3799, 3821 are refused: the live hub, its dev hub, the shared test hub)
#   SWITCH_DIR      state: home, logs, pids. Default ~/.backplane-gpui-switch (mode 700; never under the repo)
#   SWITCH_MEM      the scope's MemoryMax, default 3700M (swap is always 0). Never raise it to make a run fit; report instead.
#   SWITCH_DISPLAY  where the client opens. Required (no default): your own screen's, or :61 (build/gpui/xvfb) for tests.
#   HUB_BIN POLICY_BIN CLIENT_BIN  by absolute path; default the main checkout's build/gpui/{hub/prov/backplane-hub-full,
#                   slim/bp-policy-bp,linux/backplane-gpui}
#
# SWITCHING BACK. Starting this changes nothing of today's app, so going back is `rollback` and nothing else. What is lost is
# whatever was done on the copy: threads, messages, settings and project edits made in the GPUI app live in the copy's own log,
# not in today's. Nothing from the copy flows back into ~/.backplane-bend, and nothing done in today's app after the copy was
# made shows up here (a start makes a fresh copy every time).
set -eu
ROOT=${GPUI_MAIN:-/home/h/Documents/i2cjak/backplane-gpui}
HERE=$(cd "$(dirname "$0")/../.." && pwd)
SW=${SWITCH_DIR:-$HOME/.backplane-gpui-switch}
PORT=${SWITCH_PORT:-3790}
MEM=${SWITCH_MEM:-3700M}
SRC=${SWITCH_SOURCE:-$HOME/.backplane-gpui-copy}
HUB_BIN=${HUB_BIN:-$ROOT/build/gpui/hub/prov/backplane-hub-full}
POLICY_BIN=${POLICY_BIN:-$ROOT/build/gpui/slim/bp-policy-bp}
CLIENT_BIN=${CLIENT_BIN:-$ROOT/build/gpui/linux/backplane-gpui}
DISP=${SWITCH_DISPLAY:-}   # required for the client: never defaulted from $DISPLAY (an agent window must not land on a screen by accident)
H=$SW/home
MODE=copy

die() { echo "switch: $*" >&2; exit 1; }
alive() { [ -n "${1:-}" ] && kill -0 "$1" 2>/dev/null; }
pidof_file() { [ -f "$SW/$1.pid" ] && cat "$SW/$1.pid" || true; }
# a pid file is trusted only while /proc/<pid>/cmdline still names our binary and our home
ours() { local c; c=$(tr '\0' ' ' < "/proc/$1/cmdline" 2>/dev/null) || return 1
  case "$2" in hub) case "$c" in *"$HUB_BIN"*"$H"*) return 0;; esac;; client) case "$c" in *"$CLIENT_BIN"*"$H"*) return 0;; esac;; esac; return 1; }
owned() { [ -f "$SW/.switch-owned" ]; }
rss_mb() { awk '/^VmRSS/{printf "%d", $2/1024}' "/proc/$1/status" 2>/dev/null || echo "?"; }
hwm_mb() { awk '/^VmHWM/{printf "%d", $2/1024}' "/proc/$1/status" 2>/dev/null || echo "?"; }
helper_pid() { local h; h=$(pidof_file hub); alive "$h" && ours "$h" hub && pgrep -P "$h" 2>/dev/null | head -1 || true; }

guards() {
  case "$PORT" in 3787|3788|3799|3821) die "port $PORT belongs to another hub";; esac
  case "$PORT" in ''|*[!0-9]*) die "bad port";; esac
  case "$SW" in "$HOME/.backplane-bend"*|"$HOME/.local/share/backplane"*|"$ROOT"*|"$HERE"*) die "state dir $SW is not allowed there";; esac
  case "$SW" in ""|"/"|"$HOME"|"$HOME/") die "state dir $SW is not allowed";; esac
  [ "$SRC" != live ] || [ "${SWITCH_ALLOW_LIVE:-}" = person ] || die "SWITCH_SOURCE=live reads ~/.backplane-bend: only with SWITCH_ALLOW_LIVE=person, set by the person"
  [ -x "$HUB_BIN" ] || die "no hub binary $HUB_BIN"
  [ -x "$POLICY_BIN" ] || die "no policy helper $POLICY_BIN"
}

source_file() {
  if [ "$SRC" = live ]; then echo "$HOME/.backplane-bend/events.jsonl"; else echo "$SRC/events.jsonl"; fi
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
  touch "$SW/sample.stop"; sleep 1; keeplogs
  echo "stopped. The copy is kept in $H (rollback deletes it)."
}

cmd_start() {
  guards
  if [ -e "$SW" ] && ! owned && [ -n "$(ls -A "$SW" 2>/dev/null)" ]; then die "$SW exists and this script did not make it (no .switch-owned): refusing to use or delete it"; fi
  [ -d "$SW" ] && keeplogs
  mkdir -p "$SW"; chmod 700 "$SW"; echo "backplane switch.sh state; rollback may delete $H" > "$SW/.switch-owned"
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
  # stand-in agents: nothing may call a model
  printf '#!/bin/sh\nexec %s %s "$@"\n' "$(command -v bun)" "$HERE/test/tools/claude_standin.ts" > "$SW/bin/claude"; chmod +x "$SW/bin/claude"
  local n; for n in codex grok muse; do printf '#!/bin/sh\nexit 1\n' > "$SW/bin/$n"; chmod +x "$SW/bin/$n"; done
  # the hub in its own capped scope, a fake HOME (no real credentials), copy guard on, loopback only
  local envx; if [ "$MODE" = side ]; then
    # real HOME and PATH (the providers' logins), a real agent only on a person's turn (the hub decides); no BACKPLANE_AGENTS
    envx=(HOME="$HOME" BACKPLANE_COPY=1 BACKPLANE_SIDE_BY_SIDE=1 BACKPLANE_NO_UPDATE=1)
  else envx=(HOME="$SW/fakehome" XDG_CACHE_HOME="$SW/fakehome/.cache" BACKPLANE_COPY=1 BACKPLANE_AGENTS="$SW/bin" BACKPLANE_NO_UPDATE=1 BP_STANDIN_MODE=stream); fi
  ( cd "$SW/proj" && exec env -i PATH="$PATH" LC_ALL=C XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}" DBUS_SESSION_BUS_ADDRESS="${DBUS_SESSION_BUS_ADDRESS:-unix:path=/run/user/$(id -u)/bus}" "${envx[@]}" \
      systemd-run --user --scope -q -p "MemoryMax=$MEM" -p MemorySwapMax=0 -- \
      env -u DBUS_SESSION_BUS_ADDRESS -u XDG_RUNTIME_DIR "$HUB_BIN" --home "$H" --port "$PORT" --host 127.0.0.1 --no-tailscale --foreground --policy-bin "$POLICY_BIN" \
      > "$SW/hub.log" 2>&1 < /dev/null ) &
  echo $! > "$SW/hub.pid"
  local hp; hp=$(cat "$SW/hub.pid") i=0
  # before anything else: the stand-ins and the copy guard are in the hub's own environment
  sleep 1
  alive "$hp" || { tail -3 "$SW/hub.log"; die "the hub exited at once"; }
  local ev; ev=$(tr '\0' '\n' < "/proc/$hp/environ")
  local okenv=1; printf '%s\n' "$ev" | grep -qx "BACKPLANE_COPY=1" || okenv=0
  printf '%s\n' "$ev" | grep -q '^DBUS_SESSION_BUS_ADDRESS=\|^XDG_RUNTIME_DIR=' && okenv=0
  if [ "$MODE" = side ]; then printf '%s\n' "$ev" | grep -qx "BACKPLANE_SIDE_BY_SIDE=1" || okenv=0; printf '%s\n' "$ev" | grep -q '^BACKPLANE_AGENTS=' && okenv=0
  else printf '%s\n' "$ev" | grep -qx "BACKPLANE_AGENTS=$SW/bin" || okenv=0; fi
  [ $okenv = 1 ] || { kill "$hp" 2>/dev/null; rm -f "$SW/hub.pid"; die "hub $hp's environment is not the one required for mode $MODE (copy guard, stand-ins or side-by-side flag, no session bus): stopped"; }
  rm -f "$SW/sample.stop"; "$HERE/test/gpui/switch/sample.sh" "$SW" "$SW/rss.tsv" 5 < /dev/null > /dev/null 2>&1 &
  # a big log takes minutes to fold; wait for /hello, bounded
  echo "hub pid $hp, environment checked (stand-ins, copy guard). Waiting for it to open the log..."
  while [ $i -lt "${SWITCH_WAIT:-1800}" ]; do
    alive "$hp" || { tail -5 "$SW/hub.log"; journalctl --user --since "-3min" 2>/dev/null | grep "run-p$hp" | cut -c1-200 | tail -4; die "the hub exited after ${i}s while opening (see $SW/hub.log; the journal lines above say how: signal, exit code, memory peak)"; }
    curl -s -m2 -o /dev/null "http://127.0.0.1:$PORT/hello" && break
    sleep 1; i=$((i+1))
  done
  curl -s -m2 -o /dev/null "http://127.0.0.1:$PORT/hello" || die "no /hello after ${i}s; hub still running as pid $hp, see $SW/hub.log"
  echo "hub answers on http://127.0.0.1:$PORT after ${i}s"
  if [ "$MODE" = side ]; then  # the first-run check: nothing may act by itself; 30 s idle may add only a few settings, by count
    local l0 l1; l0=$(wc -l < "$H/events.jsonl"); sleep 30; l1=$(wc -l < "$H/events.jsonl")
    echo "idle 30 s: the copy's log grew by $((l1 - l0)) events"
    [ $((l1 - l0)) -le 50 ] || { cmd_stop >/dev/null; die "the copy's log grew by $((l1 - l0)) events while idle: something acts by itself; stopped (logs kept)"; }
  fi
  if [ "${1:-}" != "--no-client" ]; then
    [ -x "$CLIENT_BIN" ] || die "no client $CLIENT_BIN (use --no-client to run the hub alone)"
    [ -n "$DISP" ] || die "no display: set SWITCH_DISPLAY (:61 is the isolated one) or pass --no-client"
    ( exec env DISPLAY="$DISP" ${VK_ICD_FILENAMES:+VK_ICD_FILENAMES="$VK_ICD_FILENAMES"} HOME="$HOME" \
        systemd-run --user --scope -q -p "MemoryMax=$MEM" -p MemorySwapMax=0 -- \
        "$CLIENT_BIN" --url "http://127.0.0.1:$PORT" --token-file "$H/token" > "$SW/client.log" 2>&1 < /dev/null ) &
    echo $! > "$SW/client.pid"; echo "client pid $(cat "$SW/client.pid") on $DISP"
  fi
  echo "Running beside today's Backplane (untouched on 3787). To go back: scripts/gpui/switch.sh rollback"
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
  echo "Lost by going back: everything done on the copy (threads, messages, settings, edits), since it lived only in the copy."
}

case "${1:-}" in
  start) shift; MODE=copy; cmd_start "$@";;
  try) shift; MODE=try; cmd_start "$@";;
  side) shift; MODE=side; SRC=live; cmd_start "$@";;
  stop) cmd_stop;;
  status) cmd_status;;
  rollback) cmd_rollback;;
  *) sed -n '2,10p' "$0"; exit 2;;
esac
