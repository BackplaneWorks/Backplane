#!/bin/sh
# A stand-in for `omp --mode rpc-ui` (Oh My Pi 18.12's RPC). Never a real model. Scenes by a word in the prompt:
#   zqask   omp's own approval select ("Allow tool: bash"); the chosen label decides
#   zqfail  prompt_result says the run failed
#   (none)  a reply with a bash call and a Backplane call through xd://, then the stats
# FAKE_LOG, when set, gets the argv (the launch folder's files are checked, not shown), the environment and every line read.
sid=; sessions=; prev=; sp=; ext=
for a in "$@"; do case "$prev" in --session-dir) sessions=$a;; --append-system-prompt) sp=$a;; -e) ext=$a;; esac; prev=$a; done
sid="omp-$(basename "$sessions")"
if [ -n "$FAKE_LOG" ]; then
  { printf 'ARGV'; for a in "$@"; do case "$a" in */launch-*) printf ' [LAUNCH/%s]' "$(basename "$a")";; *) printf ' [%s]' "$a";; esac; done; printf '\n'
    [ -s "$sp" ] && echo "FILE system-prompt non-empty"; [ -s "$ext" ] && echo "FILE extension non-empty"; case "$BACKPLANE_OMP_MCP" in http://*) echo "ENV BACKPLANE_OMP_MCP=url";; esac; } >> "$FAKE_LOG"
fi
say() { printf '%s\n' "$1"; }
upd() { say '{"type":"message_update","assistantMessageEvent":{"type":"'"$1"'"'"$2"'}}'; }
say '{"type":"ready"}'
while IFS= read -r line; do
  [ -n "$FAKE_LOG" ] && echo "IN $line" >> "$FAKE_LOG"
  case "$line" in
    *'"type":"set_event_filter"'*) say '{"id":"0","type":"response","command":"set_event_filter","success":true}' ;;
    *'"type":"get_state"'*) say '{"id":"1","type":"response","command":"get_state","success":true,"data":{"sessionId":"'"$sid"'","messageCount":0}}' ;;
    *'"type":"prompt"'*)
      say '{"id":"2","type":"response","command":"prompt","success":true,"data":{"agentInvoked":true}}'
      say '{"type":"available_commands_update","commands":[]}'
      case "$line" in
        *zqfail*) say '{"type":"prompt_result","status":"error","error":{"message":"No API key for provider openai"}}' ;;
        *zqask*)
          say '{"type":"extension_ui_request","id":"a1","method":"select","title":"Allow tool: bash\nrm -rf build","options":["Approve","Approve bash for this session","Deny"]}'
          IFS= read -r resp; [ -n "$FAKE_LOG" ] && echo "IN $resp" >> "$FAKE_LOG"
          case "$resp" in
            *'"value":"Deny"'*) upd text_delta ',"delta":"Denied."' ;;
            *) say '{"type":"tool_execution_start","toolCallId":"c9","toolName":"bash","args":{"command":"rm -rf build"}}'; say '{"type":"tool_execution_end","toolCallId":"c9","toolName":"bash"}'; upd text_delta ',"delta":"Removed it."' ;;
          esac
          say '{"type":"prompt_result","status":"completed"}' ;;
        *)
          upd text_start ''; upd text_delta ',"delta":"Looking."'
          say '{"type":"tool_execution_start","toolCallId":"c1","toolName":"bash","args":{"command":"ls"}}'
          say '{"type":"tool_execution_end","toolCallId":"c1","toolName":"bash"}'
          say '{"type":"tool_execution_start","toolCallId":"c2","toolName":"write","args":{"path":"xd://mcp__backplane__thread_list"}}'
          say '{"type":"tool_execution_end","toolCallId":"c2","toolName":"write"}'
          upd text_start ''; upd text_delta ',"delta":"Done."'
          say '{"type":"message_end","message":{"role":"assistant","stopReason":"stop"}}'
          say '{"type":"prompt_result","status":"completed"}' ;;
      esac ;;
    *'"type":"get_session_stats"'*)
      say '{"id":"3","type":"response","command":"get_session_stats","success":true,"data":{"tokens":{"input":120,"cacheRead":30,"cacheWrite":4,"output":25,"reasoning":6},"cost":0.0042,"contextUsage":{"tokens":1500,"contextWindow":200000}}}'
      exit 0 ;;
  esac
done
