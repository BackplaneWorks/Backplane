#!/bin/bash
# What embedded KiCad would have put in a message box nobody could see is said to the person (embed-notice, ET_NOTICE): OpenGL made to
# fail (MESA_GL_VERSION_OVERRIDE=1.4, as the fork's scripts/backplane-embed-notice-check.py) gives KiCad's "Could not use OpenGL, falling
# back to software rendering" as a notice, not a modal: in the hub log, in the pane (under "Starting KiCad…" when it comes during the start,
# and as one line in the editor's strip once it is up, dismissed with its ×), and in what an agent reads of the session (kicad_ipc_status).
# No dialog is open meanwhile. A runtime without embed-notice: said as KNOWN and skipped. fork only.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/lib/env_xvfb.sh"
[ "$EMBED_MODE" = "fork" ] || { echo "embed_notice_e2e: fork only"; exit 0; }
discover_bins; [ -x "$REAL_HOME/.bun/bin/bun" ] && PATH="$REAL_HOME/.bun/bin:$PATH"
discover_kicad
if ! grep -q '"embed-notice"' "$(dirname "$(dirname "$KICAD_BIN")")/manifest.json" 2>/dev/null; then
  echo "KNOWN this runtime has no embed-notice: nothing to check"; exit 0
fi
init_isolated_dirs
python3 "$SEED_PY" "$OUT" "$REPO_ROOT"
PRO=$(find "$OUT/proj" -maxdepth 3 -name '*.kicad_pro' -not -path '*/.backplane/*' -not -name '*mic_flex*' | sort | head -1); PCB=${PRO%.kicad_pro}.kicad_pcb
start_xvfb_and_wm
# KiCad (started by the hub) gets an OpenGL it cannot use; the client draws as usual
BP_STANDIN_MCPCALL_MAX=6000 MESA_GL_VERSION_OVERRIDE=1.4 start_hub
cat > "$OUT/script.json" <<'JSON'
[{"select":0},{"wait":2000},{"click":"rail:Board"},{"wait":2500},{"click":"ui:Edit"},
 {"kicad":{"wait_painted":120000,"kind":"PCB","tag":"up"}},{"wait":3000},{"kicad":{"report":true,"tag":"shown"}},{"mark":"shown"},
 {"wait":4000},{"click":"kicad:notice-dismiss"},{"wait":2500},{"kicad":{"report":true,"tag":"dismissed"}},{"mark":"done"},{"wait":1500}]
JSON
HOME="$OUT/fh" BACKPLANE_GPUI_RECTS=1 BACKPLANE_GPUI_ASSETS="$ASSETS_DIR" "$CLIENT_BIN" --url "http://127.0.0.1:$HPORT" --token-file "$OUT/home/token" --script "$OUT/script.json" --exit-after 240 >"$OUT/client.log" 2>&1 &
CP=$!; track_pid "$CP"
for _ in $(seq 1800); do grep -q '"name":"shown","event":"mark"' "$OUT/client.log" 2>/dev/null && break; kill -0 "$CP" 2>/dev/null || break; sleep 0.1; done
grep "kicad embed: KiCad notice" "$OUT/hub.log" | head -3 | sed 's/^/INFO hub: /' || true
check "the hub log has KiCad's notice" "$(grep -q 'kicad embed: KiCad notice (graphics.*OpenGL' "$OUT/hub.log" && echo 1 || echo 0)"
R=$(last_kicad_report "$OUT/client.log" shown)
echo "INFO pane: $(echo "$R" | python3 -c 'import json,sys; r=json.loads(sys.stdin.read() or "{}"); print({k: r.get(k) for k in ("state","say","dialogs","notice")})' 2>/dev/null)"
check "the editor is up" "$(python3 -c 'import json,sys; print(int(json.loads(sys.argv[1] or "{}").get("state")=="ready"))' "$R")"
check "the pane says KiCad's notice once the editor is up (one line, in its strip)" "$(echo "$R" | grep -q 'Could not use OpenGL' && grep -q '"kicad:notice"' "$OUT/client.log" && echo 1 || echo 0)"
check "no dialog is open over KiCad (the notice is not a modal)" "$(python3 -c 'import json,sys; print(int(not json.loads(sys.argv[1] or "{}").get("dialogs")))' "$R")"
if grep -q '"kicad:notice-start"' "$OUT/client.log"; then
  check "the notice during the start is said under Starting KiCad…" 1
else
  echo "KNOWN the notice came after the editor was on show (nothing to say under Starting KiCad…): $(grep -o 'kicad embed: KiCad notice' "$OUT/hub.log" | head -1)"
fi
# an agent asking the session's state
before=$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null || true)
text=$(python3 -c 'import json,sys; print(json.dumps("mcpcall:" + json.dumps({"name": "kicad_ipc_status", "arguments": {"path": sys.argv[1]}})))' "$PCB")
hub_rpc "[{\"method\":\"turn.start\",\"params\":{\"thread\":\"t1\",\"text\":$text,\"msg\":\"mnotice$(date +%s%N)\"}}]" >>"$OUT/rpc.log" 2>&1 || true
for _ in $(seq 240); do n=$(grep -c "^MCPCALL" "$OUT/standin.log" 2>/dev/null || true); [ "${n:-0}" -gt "${before:-0}" ] && break; sleep 0.25; done
ST=$(grep "^MCPCALL kicad_ipc_status" "$OUT/standin.log" | tail -1 || true)
echo "INFO agent: $(echo "$ST" | grep -o 'notices[^]]*' | head -1 | cut -c1-300)"
check "an agent asking the session's state reads the notice" "$(echo "$ST" | grep -q 'notices.*Could not use OpenGL' && echo 1 || echo 0)" "$(echo "$ST" | cut -c1-300)"
for _ in $(seq 400); do grep -q '"name":"done","event":"mark"' "$OUT/client.log" 2>/dev/null && break; kill -0 "$CP" 2>/dev/null || break; sleep 0.1; done
D=$(last_kicad_report "$OUT/client.log" dismissed)
check "the person's × dismisses it in the pane" "$(python3 -c 'import json,sys; print(int(json.loads(sys.argv[1] or "{}").get("notice") is None))' "$D")" "$(echo "$D" | cut -c1-200)"
kill "$CP" 2>/dev/null || true
finish_suite "embed_notice_e2e ($EMBED_MODE)"
