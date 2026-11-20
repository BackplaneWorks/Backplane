#!/bin/bash
# The hub waits for KiCad's real exit after a close and records it (the close-wait PR); a hang past the bound (15 s) is ended only when
# KiCad cannot be saving or asking. Each row starts its own KiCad through the hub (editor.kicad.attach, no pane) and ends it one way:
#   a   the hub's close (an agent's: discard), KiCad stopped (SIGSTOP) right after it answered `exiting`, never let go: the session reads
#       closing (not stopped), KiCad is still there 3 s later, an attach meanwhile is answered closing, and at the bound it is ended as a
#       hang (logged); the next attach starts a fresh KiCad
#   c1  the hub's close, then KiCad aborts at its exit (SIGABRT once it answered: the bp.13.x teardown abort): logged as an abort at exit,
#       recorded as the close (cold), not as a crash
#   c2  KiCad's own Quit (its menu's, nothing modified), then an abort at its exit (SIGABRT once its socket went with no frame left):
#       closed by the person (closedByPerson), not "closed unexpectedly"
#   e   KiCad's own Quit, stopped once its socket went and never let go: closing, ended at the bound as a hang (logged)
#   d   the hub's close with a save (save: "save", the board modified) whose save is slow (KiCad stopped from just before the Close for
#       20 s: no answer within the hub's 15 s): its caller hears "not answered yet", not "refused to close"; KiCad is not killed (a
#       missing answer may be KiCad asking); once it
#       goes on it ends by itself; the board is whole (kicad-cli loads it, the track in it) and the local history repository is sound
# On #1470's base (a made-up exit and an 800 ms kill after the hub's close; nothing after KiCad's own Quit) a, c1, c2, d and e fail.
# Copies only (seed_project.py), fork mode.
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_close_wait_e2e: fork only"; exit 0; }
discover_bins; [ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"; discover_kicad; init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT" >/dev/null 2>&1
PROJ="$OUT/proj"; PRO="$PROJ/ecc83-pp.kicad_pro"; PCB="$PROJ/ecc83-pp.kicad_pcb"
export BACKPLANE_KICAD_DEBUG=1 BACKPLANE_KICAD_WARM_HEADROOM_MB=0
start_xvfb_and_wm; start_hub
for _ in $(seq 60); do hub_rpc "[]" >/dev/null 2>&1 && break; sleep 0.5; done
info() { hub_rpc "[]" 2>/dev/null | grep '^INFO ' | grep '"key":"editor.kicad.' | tail -1 | cut -c6- | python3 -c 'import json,sys
try: v=json.load(sys.stdin).get("value") or {}
except Exception: v={}
if isinstance(v,str): v=json.loads(v)
r=v.get("runtime") or {}
print(json.dumps({"state": v.get("state"), "pid": v.get("pid"), "closing": v.get("closing"), "sock": v.get("sock"), "session": r.get("session"), "stop": r.get("stopReason")}))' 2>/dev/null || echo '{}'; }
f() { python3 -c 'import json,sys; v=json.loads(sys.argv[1] or "{}"); x=v.get(sys.argv[2]); print("" if x is None else x)' "$1" "$2"; }
start_kicad() { # a KiCad for the project, no pane; echoes its info once ready
  hub_rpc "[{\"method\":\"editor.kicad.attach\",\"timeout_ms\":120000,\"params\":{\"project\":\"$PRO\",\"frame\":\"PCB\",\"wait\":true}}]" >"$OUT/attach-$1.out" 2>&1 || true
  local i; for _ in $(seq 240); do i=$(info); [ "$(f "$i" state)" = ready ] && [ -n "$(f "$i" pid)" ] && break; sleep 0.5; done; sleep 6; info; }
close_hub() { # the hub's close (an agent's), asked again while KiCad's automatic check runs
  for _ in $(seq 60); do
    hub_rpc "[{\"method\":\"editor.kicad.close\",\"timeout_ms\":90000,\"params\":{\"project\":\"$PRO\",\"save\":\"$1\"}}]" >"$OUT/close.out" 2>&1 || true
    grep -q '"closed":true' "$OUT/close.out" && return 0; grep -q 'a task is running' "$OUT/close.out" || return 1; sleep 1; done; return 1; }
gone() { for _ in $(seq "${2:-120}"); do kill -0 "$1" 2>/dev/null || return 0; sleep 0.25; done; return 1; }
hub_since() { tail -n +"$(($1 + 1))" "$OUT/hub.log"; }
wait_hub() { for _ in $(seq "${3:-400}"); do hub_since "$1" | grep -q "$2" && return 0; sleep 0.02; done; return 1; }

# ---- a
I=$(start_kicad a); P=$(f "$I" pid); echo "INFO a: KiCad $P"; H=$(wc -l <"$OUT/hub.log")
close_hub discard; kill -STOP "$P" 2>/dev/null; T0=$(date +%s)
sleep 1; I=$(info); sleep 2; ALIVE3=$(kill -0 "$P" 2>/dev/null && echo 1 || echo 0)
hub_rpc "[{\"method\":\"editor.kicad.attach\",\"params\":{\"project\":\"$PRO\",\"frame\":\"PCB\",\"wait\":false}}]" >"$OUT/attach-a2.out" 2>&1 || true
CLOSING=$(grep -c '"closing":true' "$OUT/attach-a2.out" || true)
gone "$P" 120; KILLED=$(hub_since "$H" | grep -o "a hang after its agentClosed close: pid $P still running [0-9]* ms after it, killed" | head -1)
echo "INFO a: after the close $I; alive 3 s later $ALIVE3; attach closing $CLOSING; ${KILLED:-not ended as a hang} ($(( $(date +%s) - T0 )) s)"
check "a the hub's close, KiCad never ends: closing (not stopped), still there 3 s later, an attach answered closing, ended at the bound as a hang" "$([ "$(f "$I" closing)" = True ] && [ "$ALIVE3" = 1 ] && [ "${CLOSING:-0}" -ge 1 ] && [ -n "$KILLED" ] && echo 1 || echo 0)" "$I alive3 $ALIVE3 closing $CLOSING $KILLED"
for _ in $(seq 40); do [ "$(f "$(info)" state)" = stopped ] && break; sleep 0.25; done

# ---- c1
I=$(start_kicad c1); P=$(f "$I" pid); echo "INFO c1: KiCad $P"; H=$(wc -l <"$OUT/hub.log")
close_hub discard; kill -ABRT "$P" 2>/dev/null; gone "$P" 60
for _ in $(seq 40); do I=$(info); [ "$(f "$I" state)" = stopped ] && break; sleep 0.25; done
ABORT=$(hub_since "$H" | grep -c "ended after its close (.*): an abort at exit, recorded as the close (agentClosed)" || true)
echo "INFO c1: $I; abort logged $ABORT"
check "c1 the hub's close, then an abort at exit: logged, recorded as the close (cold), not a crash" "$([ "$(f "$I" session)" = cold ] && [ "${ABORT:-0}" -ge 1 ] && echo 1 || echo 0)" "$I abort $ABORT"

# ---- c2
I=$(start_kicad c2); P=$(f "$I" pid); S=$(f "$I" sock); echo "INFO c2: KiCad $P"; H=$(wc -l <"$OUT/hub.log")
python3 "$IPC_PY" run-action "$S" common.Control.quit >"$OUT/quit-c2.out" 2>&1 &
wait_hub "$H" "NextEvents failed (1/6)" 500 && kill -ABRT "$P" 2>/dev/null; gone "$P" 60
for _ in $(seq 40); do I=$(info); [ "$(f "$I" state)" = stopped ] || [ "$(f "$I" state)" = closed_unexpectedly ] && break; sleep 0.25; done
echo "INFO c2: $I; $(hub_since "$H" | grep -E "socket went|ended after its close|ended \(" | head -2 | cut -c1-160 | tr '\n' ' ')"
check "c2 KiCad's own Quit, then an abort at exit: closed by the person, not closed unexpectedly" "$([ "$(f "$I" session)" = closedByPerson ] && echo 1 || echo 0)" "$I"

# ---- e
I=$(start_kicad e); P=$(f "$I" pid); S=$(f "$I" sock); echo "INFO e: KiCad $P"; H=$(wc -l <"$OUT/hub.log")
python3 "$IPC_PY" run-action "$S" common.Control.quit >"$OUT/quit-e.out" 2>&1 &
wait_hub "$H" "NextEvents failed (1/6)" 500 && kill -STOP "$P" 2>/dev/null; T0=$(date +%s)
gone "$P" 160; KILLED=$(hub_since "$H" | grep -o "a hang after its kicadQuit quit: pid $P still running [0-9]* ms after it, killed" | head -1)
echo "INFO e: ${KILLED:-not ended as a hang} ($(( $(date +%s) - T0 )) s); $(hub_since "$H" | grep -E "socket went" | head -1 | cut -c1-160)"
check "e KiCad's own Quit that never ends after its socket went: ended at the bound as a hang (logged)" "$([ -n "$KILLED" ] && ! kill -0 "$P" 2>/dev/null && echo 1 || echo 0)" "$KILLED"
kill -CONT "$P" 2>/dev/null
for _ in $(seq 40); do [ "$(f "$(info)" state)" = stopped ] && break; sleep 0.25; done

# ---- d
I=$(start_kicad d); P=$(f "$I" pid); S=$(f "$I" sock); echo "INFO d: KiCad $P"; H=$(wc -l <"$OUT/hub.log")
SEG0=$(grep -c "(segment" "$PCB" || true); HIST0=$(git -C "$PROJ/.history" rev-list --all --count 2>/dev/null || echo 0)
python3 "$IPC_PY" agent-add-track "$S" 100 120 104 120 0.25 3 >"$OUT/track-d.out" 2>&1 || true
kill -STOP "$P"; ( sleep 20; kill -CONT "$P" ) &   # KiCad busy (a long save, as bp.13.1's took 35 s): the Close waits on it
close_hub save; RC=$?; REFUSED=$(grep '"closed":false' "$OUT/close.out" | grep -vc '"pending":true' || true); PENDING=$(grep -c '"pending":true' "$OUT/close.out" || true)
echo "INFO d: the close answered (rc $RC): $(cut -c1-200 "$OUT/close.out" | tr '\n' ' ')"
wait; gone "$P" 200; KILLED=$(hub_since "$H" | grep -c "a hang after .*pid $P" || true); WAITED=$(hub_since "$H" | grep -c "pid $P still running .* not killed" || true)
SEG1=$(grep -c "(segment" "$PCB" || true); "${KICAD_CLI:-kicad-cli}" pcb export svg --output "$OUT/pcb-d.svg" --layers F.Cu "$PCB" >"$OUT/cli-d.out" 2>&1; CLI=$?
if [ -d "$PROJ/.history" ]; then FSCK=$(git -C "$PROJ/.history" fsck --no-progress >/dev/null 2>&1 && echo ok || echo bad); else FSCK=none; fi; HIST1=$(git -C "$PROJ/.history" rev-list --all --count 2>/dev/null || echo 0)
echo "INFO d: pending (no answer within 15 s) $PENDING, refused $REFUSED, killed $KILLED, kills skipped $WAITED; segments $SEG0 -> $SEG1, kicad-cli $CLI, history $HIST0 -> $HIST1 commits ($FSCK)"
check "d a slow save-close: not refused, not killed while it may be writing, the board whole with the track, the history sound" "$([ "${REFUSED:-0}" = 0 ] && [ "${KILLED:-0}" = 0 ] && [ "$CLI" = 0 ] && [ "$SEG1" -gt "$SEG0" ] && { [ "$FSCK" = ok ] || [ "$FSCK" = none ]; } && echo 1 || echo 0)" "refused $REFUSED killed $KILLED seg $SEG0->$SEG1 cli $CLI fsck $FSCK"

grep -E "kicad embed: .*(close answered|had no answer|a hang|not killed|abort at exit|socket went|refused)" "$OUT/hub.log" | cut -c1-200 | sed 's/^/INFO   /' || true
finish_suite "embed_close_wait_e2e ($EMBED_MODE)"
