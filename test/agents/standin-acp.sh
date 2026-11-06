#!/bin/sh
# A stand-in for `grok agent stdio` and `opencode acp` (ACP over stdio), installed under either name. No model, no network.
# Words in the prompt choose the scenario: zqask (a permission request, the answer is logged), zqnomodel never. FAKE_LOST=1: session/resume answers
# "session not found". FAKE_NOMODEL=1: session/set_config_option answers an error. Everything it reads is logged to $FAKE_LOG as "IN <line>".
# fake standin: no model, no network
name=$(basename "$0"); log=${FAKE_LOG:-/dev/null}; sid="S-$$"
echo "START $name $$ ARGS $*" >> "$log"
[ "$name" = opencode ] && printf "ENV %s\n" "$OPENCODE_CONFIG_CONTENT" >> "$log"
out() { printf '%s\n' "$1"; }
upd() { out '{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"'"$sid"'","update":'"$1"'}}'; }
while IFS= read -r line; do
  printf "IN %s\n" "$line" >> "$log"
  id=$(printf '%s' "$line" | sed -n 's/^{"jsonrpc":"2.0","id":\([0-9]*\),.*/\1/p')
  case "$line" in
    *'"method":"initialize"'*) out '{"jsonrpc":"2.0","id":'"$id"',"result":{"protocolVersion":1,"agentCapabilities":{"loadSession":true,"sessionCapabilities":{"resume":{}}}}}' ;;
    *'"method":"session/new"'*) out '{"jsonrpc":"2.0","id":'"$id"',"result":{"sessionId":"'"$sid"'"}}' ;;
    *'"method":"session/load"'*) upd '{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"(replayed old text)"}}'; out '{"jsonrpc":"2.0","id":'"$id"',"result":{}}' ;;
    *'"method":"session/resume"'*)
      if [ -n "$FAKE_LOST" ]; then out '{"jsonrpc":"2.0","id":'"$id"',"error":{"code":-32602,"message":"session not found: old"}}'
      else out '{"jsonrpc":"2.0","id":'"$id"',"result":{"configOptions":[]}}'; fi ;;
    *'"method":"session/set_config_option"'*)
      if [ -n "$FAKE_NOMODEL" ]; then out '{"jsonrpc":"2.0","id":'"$id"',"error":{"code":-32602,"message":"Invalid params: model not found: nope/x"}}'
      else out '{"jsonrpc":"2.0","id":'"$id"',"result":{"configOptions":[]}}'; fi ;;
    *'"method":"session/prompt"'*)
      case "$line" in *zqask*)
        out '{"jsonrpc":"2.0","id":"perm1","method":"session/request_permission","params":{"sessionId":"'"$sid"'","toolCall":{"toolCallId":"pc1","title":"List files","kind":"execute","rawInput":{"command":"ls -la"}},"options":[{"optionId":"once","kind":"allow_once","name":"Allow once"},{"optionId":"always","kind":"allow_always","name":"Always allow"},{"optionId":"reject","kind":"reject_once","name":"Reject"}]}}'
        read -r ans; printf "IN %s\n" "$ans" >> "$log"
        case "$ans" in *'"optionId":"reject"'*) upd '{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"denied. "}}' ;; *) upd '{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"allowed. "}}' ;; esac ;;
      esac
      if [ "$name" = opencode ]; then
        upd '{"sessionUpdate":"tool_call","toolCallId":"c1","name":"bash","title":"bash","kind":"execute","status":"pending","locations":[],"rawInput":{"cwd":"/tmp"}}'
        upd '{"sessionUpdate":"tool_call_update","toolCallId":"c1","status":"in_progress","kind":"execute","title":"ls -la","locations":[],"rawInput":{"command":"ls -la","cwd":"/tmp"}}'
        upd '{"sessionUpdate":"tool_call_update","toolCallId":"c1","status":"completed","locations":[],"content":[],"rawOutput":{}}'
        upd '{"sessionUpdate":"tool_call_update","toolCallId":"c2","status":"in_progress","kind":"read","title":"read","locations":[{"path":"'"$(pwd)"'/src/a.kicad_pcb"}],"rawInput":{"filePath":"'"$(pwd)"'/src/a.kicad_pcb"}}'
        upd '{"sessionUpdate":"tool_call_update","toolCallId":"c2","status":"failed","locations":[]}'
      else
        upd '{"sessionUpdate":"tool_call","toolCallId":"c1","title":"List files","kind":"execute","status":"pending"}'
        upd '{"sessionUpdate":"tool_call_update","toolCallId":"c1","status":"in_progress"}'
        upd '{"sessionUpdate":"tool_call_update","toolCallId":"c1","status":"completed"}'
        upd '{"sessionUpdate":"tool_call","toolCallId":"c2","kind":"read","status":"pending"}'
        upd '{"sessionUpdate":"tool_call_update","toolCallId":"c2","status":"failed"}'
      fi
      upd '{"sessionUpdate":"plan","entries":[{"content":"look","priority":"high","status":"completed"},{"content":"fix","priority":"high","status":"in_progress"},{"content":"test"}]}'
      upd '{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Hello "}}'
      upd '{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"world."}}'
      out '{"jsonrpc":"2.0","id":'"$id"',"result":{"stopReason":"end_turn"}}'
      # the hub ends the process when the turn is over; wait for it (and for stdin to close)
      ;;
  esac
done
