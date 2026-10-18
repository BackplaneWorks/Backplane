// Deterministic protocol fixture. No model or external provider runs.
export const standinScript = `#!/bin/sh
secs=\${FAKE_SECS:-600}; per=\${FAKE_PER:-3}; cycle=\${FAKE_CYCLE:-30}
sid="fake-$$"; n=0
echo "START $$ $(pwd)" >> "$FAKE_LOG"
while IFS= read -r line; do
  case "$line" in
    *'"type":"user"'*)
      n=$((n + 1)); ticks=$((secs * 10)); sub=
      case "$line" in *zqshort*) ticks=80 ;; esac
      case "$line" in *zqsub*) sub=1 ;; esac
      echo "TURN $$ sub=$sub ticks=$ticks" >> "$FAKE_LOG"
      printf '%s\\n' '{"type":"system","subtype":"init","session_id":"'"$sid"'","cwd":"/tmp","tools":["Bash","Read","Agent"],"model":"fake","permissionMode":"default"}'
      if [ -n "$sub" ]; then
        printf '%s\\n' '{"type":"assistant","message":{"id":"ma'"$$-$n"'","role":"assistant","content":[{"type":"tool_use","id":"ag'"$$-$n"'","name":"Agent","input":{"description":"Survey the footprints","prompt":"Look through the footprints and report","subagent_type":"general-purpose"}}],"usage":{"input_tokens":10}},"session_id":"'"$sid"'"}'
        printf '%s\\n' '{"type":"system","subtype":"task_started","task_id":"a'"$$-$n"'","tool_use_id":"ag'"$$-$n"'","description":"Survey the footprints","task_type":"local_agent","is_backgrounded":true,"session_id":"'"$sid"'"}'
        printf '%s\\n' '{"type":"system","subtype":"task_started","task_id":"b'"$$-$n"'","tool_use_id":"tb'"$$-$n"'","description":"npm run watch","task_type":"local_bash","is_backgrounded":true,"session_id":"'"$sid"'"}'
      fi
      t=0; k=0
      while [ $t -lt $ticks ]; do
        kill -0 $PPID 2>/dev/null || exit 0
        t=$((t + 1)); k=$((k + 1))
        if [ $k -eq 1 ]; then
          printf '%s\\n' '{"type":"stream_event","event":{"type":"message_start","message":{"id":"m'"$$-$n-$t"'","role":"assistant","content":[],"usage":{"input_tokens":10}}},"session_id":"'"$sid"'"}'
          printf '%s\\n' '{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}},"session_id":"'"$sid"'"}'
        fi
        if [ -n "$sub" ]; then
          printf '%s\\n' '{"type":"system","subtype":"task_progress","task_id":"a'"$$-$n"'","description":"Reading footprint '"$t"'","session_id":"'"$sid"'"}'
          printf '%s\\n' '{"type":"system","subtype":"task_progress","task_id":"b'"$$-$n"'","description":"rebuilt '"$t"'","session_id":"'"$sid"'"}'
          if [ $((t % 10)) -eq 5 ]; then
            printf '%s\\n' '{"type":"assistant","parent_tool_use_id":"ag'"$$-$n"'","message":{"id":"ms'"$$-$n-$t"'","role":"assistant","content":[{"type":"text","text":"Checked footprint '"$t"', pads match the datasheet."},{"type":"tool_use","id":"st'"$$-$n-$t"'","name":"Read","input":{"file_path":"/tmp/f'"$t"'.kicad_mod"}}]},"session_id":"'"$sid"'"}'
          fi
        fi
        now=$(date +%s%3N); i=0
        while [ $i -lt $per ]; do
          i=$((i + 1))
          printf '%s\\n' '{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"@'"$now"'@ some words of an answer about the board, "}},"session_id":"'"$sid"'"}'
        done
        if [ $k -ge $cycle ]; then
          k=0
          printf '%s\\n' '{"type":"stream_event","event":{"type":"content_block_stop","index":0},"session_id":"'"$sid"'"}'
          printf '%s\\n' '{"type":"assistant","message":{"id":"m'"$$-$n-$t"'","role":"assistant","content":[{"type":"text","text":"Some words of an answer about the board, the whole paragraph the deltas built, as Claude sends it once the block ends. It names U3 and the 3V3 rail, and says what changed in the layout and why."}],"usage":{"input_tokens":1000,"cache_read_input_tokens":'"$((20000 + t))"',"output_tokens":'"$t"'}},"session_id":"'"$sid"'"}'
          printf '%s\\n' '{"type":"assistant","message":{"id":"m'"$$-$n-$t"'b","role":"assistant","content":[{"type":"tool_use","id":"tu'"$$-$n-$t"'","name":"Bash","input":{"command":"ls -la src/core | head -'"$t"'","description":"List files"}}],"usage":{"input_tokens":1000,"cache_read_input_tokens":'"$((20000 + t))"',"output_tokens":'"$((t + 1))"'}},"session_id":"'"$sid"'"}'
          printf '%s\\n' '{"type":"user","message":{"role":"user","content":[{"tool_use_id":"tu'"$$-$n-$t"'","type":"tool_result","content":"total 42\\\\ndrwxr-xr-x  2 h h 4096 model.bend\\\\n-rw-r--r--  1 h h 1234 hub.bend","is_error":false}]},"session_id":"'"$sid"'"}'
        fi
        sleep 0.1
      done
      if [ -n "$sub" ]; then
        printf '%s\\n' '{"type":"system","subtype":"task_notification","task_id":"a'"$$-$n"'","tool_use_id":"ag'"$$-$n"'","status":"completed","session_id":"'"$sid"'"}'
        printf '%s\\n' '{"type":"system","subtype":"task_notification","task_id":"b'"$$-$n"'","tool_use_id":"tb'"$$-$n"'","status":"completed","session_id":"'"$sid"'"}'
      fi
      printf '%s\\n' '{"type":"result","subtype":"success","is_error":false,"result":"done","session_id":"'"$sid"'","modelUsage":{"fake":{"contextWindow":200000}}}' ;;
  esac
done
`;
