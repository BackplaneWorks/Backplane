# Sourced by journey_project.sh after the libraries row: the ways a person's KiCad session ends or doubles, on the clean box.
#   R1 warm and Edit together: showing the thread warms the project's KiCad and Edit is pressed under a second later: one KiCad
#   R2 KiCad killed (as by a crash), then Edit: a new KiCad, painted in the pane
#   R3 Backplane killed mid-edit (its window and its hub), then started again as a person does, then Edit: KiCad painted, and one KiCad
# The client's script presses (Edit waits for its button, "within"); the harness kills between the script's marks.
phase "restart rows"
kinfo() { grep -h 'GPUI_LOG {"kicad_info"' "$HH/win.log" 2>/dev/null | grep "\"project\":\"$PROJ/" | tail -1; }
kfield() { kinfo | python3 -c 'import json,sys; l=sys.stdin.read(); print(json.loads(l[l.index("{"):]).get(sys.argv[1], "") if "{" in l else "")' "$1" 2>/dev/null; }
# the embedded KiCads running on this project (its board or project path on the command line)
kcount() { in_box sh -c "pgrep -af -- '--embedded' 2>/dev/null" | grep -v pgrep | grep -c "pic_programmer"; }
WP=$(cat "$HH/.client.pid" 2>/dev/null); [ -n "$WP" ] && in_box kill "$WP" 2>/dev/null; sleep 2
# KiCad from the sessions before is closed first (R1 is about a cold project): the hub's sessions end with their client
in_box sh -c "pkill -f -- '--embedded' 2>/dev/null"; sleep 3
cat >"$HH/restart.json" <<J
[{"wait":3000},{"click":"project:$pid_"},{"wait":1500},{"click":"page-thread:$tid"},{"wait":300},{"click":"rail:Board"},{"wait":600},{"click":"ui:Edit"},
 {"kicad":{"wait_painted":120000,"kind":"PCB","tag":"r1"}},{"kicad":{"report":true,"tag":"r1"}},{"mark":"r1"},{"wait":20000},
 {"click":"ui:Edit","within":60000},{"kicad":{"wait_painted":120000,"kind":"PCB","tag":"r2"}},{"kicad":{"report":true,"tag":"r2"}},{"mark":"r2"},{"wait":1800000}]
J
: >"$HH/win.log"
launch --url http://127.0.0.1:$(hub_port) --token-file $PH/.backplane/hub/token --script $PH/restart.json
rmark() { for _ in $(seq ${2:-600}); do grep -q "\"name\":\"$1\"" "$HH/win.log" 2>/dev/null && return 0; sleep 0.25; done; return 1; }
if rmark r1 720; then sleep 3; n=$(kcount); P1=$(kfield pid); shot 40-r1
  [ "$n" = 1 ] && ok "R1 warm and Edit together: one KiCad for the project (pid $P1)" || { bad "R1 warm and Edit together: $n KiCads for the project"; cut_ 1 restart "the warm-up and Edit together started $n KiCads"; }
  # R2: KiCad dies; the script presses Edit again when its button is back
  in_box kill -9 "$P1" 2>/dev/null; t=$(date +%s)
  if rmark r2 720; then sleep 2; P2=$(kfield pid); S2=$(kfield state); F2=$(kfield frames); shot 41-r2
    [ -n "$P2" ] && [ "$P2" != "$P1" ] && [ "$S2" = ready ] && [ "${F2:-0}" -ge 1 ] && ok "R2 after KiCad was killed, Edit shows a new KiCad (pid $P2, in $(( $(date +%s) - t )) s)" \
      || { bad "R2 after KiCad was killed, Edit did not show a new KiCad (pid $P1 -> ${P2:-none}, $S2, frames $F2)"; cut_ 1 restart "after KiCad crashed, Edit did not bring it back"; }
  else bad "R2 after KiCad was killed, Edit never painted KiCad again ($(kinfo | cut -c1-200))"; cut_ 1 restart "after KiCad crashed, Edit did not bring it back"; fi
else bad "R1 Edit never showed KiCad ($(kinfo | cut -c1-200))"; fi
cp "$HH/win.log" "$OUT/logs/win-restart-1.log" 2>/dev/null
# R3: Backplane killed mid-edit, its window and its hub, with KiCad open in the pane
HPID=$(cut -d' ' -f1 "$HUBH/hub.lock" 2>/dev/null); WP=$(cat "$HH/.client.pid" 2>/dev/null)
[ -n "$WP" ] && in_box kill -9 "$WP" 2>/dev/null; [ -n "$HPID" ] && in_box kill -9 "$HPID" 2>/dev/null; sleep 3
# started again as a person does (no arguments: the app starts its hub), then the board and Edit
: >"$HH/win.log"; launch; for _ in $(seq 240); do hub_up && break; sleep 0.5; done
hub_up && ok "R3 Backplane started again after it was killed (hub on port $(hub_port))" || { bad "R3 Backplane did not start again after it was killed"; cut_ 1 restart "after Backplane was killed it did not start again"; }
WP=$(cat "$HH/.client.pid" 2>/dev/null); [ -n "$WP" ] && in_box kill "$WP" 2>/dev/null; sleep 2
cat >"$HH/restart3.json" <<J
[{"wait":3000},{"click":"project:$pid_"},{"wait":2000},{"click":"page-thread:$tid"},{"wait":2500},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},
 {"kicad":{"wait_painted":120000,"kind":"PCB","tag":"r3"}},{"kicad":{"report":true,"tag":"r3"}},{"mark":"r3"},{"wait":600000}]
J
: >"$HH/win.log"; t=$(date +%s)
launch --url http://127.0.0.1:$(hub_port) --token-file $PH/.backplane/hub/token --script $PH/restart3.json
if rmark r3 720; then sleep 3; S3=$(kfield state); F3=$(kfield frames); n=$(kcount); shot 42-r3
  [ "$S3" = ready ] && [ "${F3:-0}" -ge 1 ] && ok "R3 after Backplane was killed mid-edit and started again, Edit shows KiCad (in $(( $(date +%s) - t )) s)" \
    || { bad "R3 after Backplane was killed and started again, Edit did not show KiCad ($S3, frames $F3)"; cut_ 1 restart "after Backplane was killed mid-edit, Edit did not show KiCad again"; }
  [ "$n" = 1 ] && ok "R3 one KiCad for the project after the restart (the killed hub's is gone or reused)" || { bad "R3 $n KiCads for the project after the restart"; cut_ 1 restart "a KiCad left by the killed Backplane runs beside the new one ($n)"; }
else bad "R3 after Backplane was killed and started again, Edit never painted KiCad ($(kinfo | cut -c1-200))"; cut_ 1 restart "after Backplane was killed mid-edit, Edit did not show KiCad again"; fi
cp "$HH/win.log" "$OUT/logs/win-restart-3.log" 2>/dev/null; WP=$(cat "$HH/.client.pid" 2>/dev/null); [ -n "$WP" ] && in_box kill "$WP" 2>/dev/null; sleep 1
