#!/bin/sh
# A stand-in for `pi --mode rpc` (Pi 1.4's JSONL RPC). Never a real model. The prompt picks the scene by a word:
#   zqask   Backplane's extension asks to run a tool (confirm "backplane:approval"); the answer decides the tool
#   zqquest another extension's select dialog; the answer is echoed in the reply
#   zqfail  the assistant message ends with an error
#   zqabort the run settles aborted
#   (none)  a reply in two paragraphs, a thought, a bash call, a Backplane call
# FAKE_LOG, when set, gets the argv, the environment the extension reads, and every line read.
sid=; prev=
for a in "$@"; do [ "$prev" = "--session-id" ] && sid=$a; prev=$a; done
if [ -n "$FAKE_LOG" ]; then
  { printf 'ARGV'; for a in "$@"; do printf ' [%s]' "$a"; done; printf '\n'; echo "ENV BACKPLANE_PI_MODE=$BACKPLANE_PI_MODE"; case "$BACKPLANE_PI_MCP" in http://*) echo "ENV BACKPLANE_PI_MCP=url";; esac; } >> "$FAKE_LOG"
fi
say() { printf '%s\n' "$1"; }
upd() { say '{"type":"message_update","assistantMessageEvent":{"type":"'"$1"'"'"$2"'}}'; }
while IFS= read -r line; do
  [ -n "$FAKE_LOG" ] && echo "IN $line" >> "$FAKE_LOG"
  case "$line" in
    *'"type":"get_state"'*) say '{"id":"1","type":"response","command":"get_state","success":true,"data":{"sessionId":"'"$sid"'","messageCount":0}}' ;;
    *'"type":"prompt"'*)
      say '{"id":"2","type":"response","command":"prompt","success":true,"data":{"disposition":"started"}}'
      say '{"type":"agent_start"}'
      say '{"type":"message_start","message":{"role":"assistant","content":[]}}'
      case "$line" in
        *zqfail*)
          say '{"type":"message_end","message":{"role":"assistant","stopReason":"error","errorMessage":"No API key for provider anthropic"}}'
          say '{"type":"agent_settled"}'; exit 0 ;;
        *zqabort*) say '{"type":"agent_settled","aborted":true}'; exit 0 ;;
        *zqquest*)
          say '{"type":"extension_ui_request","id":"x1","method":"select","title":"Which branch?","options":["main","dev"]}'
          IFS= read -r resp; [ -n "$FAKE_LOG" ] && echo "IN $resp" >> "$FAKE_LOG"
          upd text_start ''; upd text_delta ',"delta":"You said: '"$(printf '%s' "$resp" | sed -n 's/.*"\(value\|cancelled\)":\("\?[^",}]*\)"\?.*/\1=\2/p')"'"'
          ;;
        *zqask*)
          say '{"type":"extension_ui_request","id":"x2","method":"confirm","title":"backplane:approval","message":"{\"tool\":\"bash\",\"input\":{\"command\":\"rm -rf build\"}}"}'
          IFS= read -r resp; [ -n "$FAKE_LOG" ] && echo "IN $resp" >> "$FAKE_LOG"
          case "$resp" in
            *'"confirmed":true'*) say '{"type":"tool_execution_start","toolCallId":"c9","toolName":"bash","args":{"command":"rm -rf build"}}'; say '{"type":"tool_execution_end","toolCallId":"c9","toolName":"bash","isError":false}'; upd text_start ''; upd text_delta ',"delta":"Removed it."' ;;
            *) upd text_start ''; upd text_delta ',"delta":"Left it alone."' ;;
          esac ;;
        *)
          upd text_start ''; upd text_delta ',"delta":"Looking at the board."'
          upd thinking_end ',"content":"check the outline"'
          say '{"type":"tool_execution_start","toolCallId":"c1","toolName":"bash","args":{"command":"ls -la"}}'
          say '{"type":"tool_execution_end","toolCallId":"c1","toolName":"bash","isError":false}'
          say '{"type":"tool_execution_start","toolCallId":"c2","toolName":"mcp__backplane__thread_list","args":{}}'
          say '{"type":"tool_execution_end","toolCallId":"c2","toolName":"mcp__backplane__thread_list","isError":false}'
          say '{"type":"tool_execution_start","toolCallId":"c3","toolName":"read","args":{"path":"board.kicad_pcb"}}'
          say '{"type":"tool_execution_end","toolCallId":"c3","toolName":"read","isError":false}'
          upd text_start ''; upd text_delta ',"delta":"All good."' ;;
      esac
      say '{"type":"message_end","message":{"role":"assistant","stopReason":"stop"}}'
      say '{"type":"turn_end","message":{"role":"assistant"}}'
      say '{"type":"agent_end","messages":[]}'
      say '{"type":"agent_settled"}'
      exit 0 ;;
  esac
done
