#!/bin/sh
# A stand-in for `codex exec --json` (one process per turn) and for its one-shot writing calls (`--ephemeral`). Never a real model.
# A turn reads its text from the last argument; "slow" waits 2 s, "die" exits 3, "hang" waits a minute; every turn says thread.started, a delta, a command
# and a reply that echoes the text. The argv goes to argv.log beside the script and to $FAKE_LOG; a one-shot answers one agent_message naming its model.
case "$1" in --version) echo "codex-cli 0.0.0"; exit 0;; esac
for a in "$@"; do last="$a"; done
D=$(dirname "$0"); echo "$@" >> "$D/argv.log"; echo $$ > "$D/pid"
[ -n "$FAKE_LOG" ] && echo "CODEX $*" >> "$FAKE_LOG"
case " $* " in *" --ephemeral "*)
  m=default; p=; for a in "$@"; do [ "$p" = -m ] && m=$a; p=$a; done
  printf '{"type":"item.completed","item":{"type":"agent_message","text":"Codex title from %s"}}\n' "$m"; exit 0;; esac
echo '{"type":"thread.started","thread_id":"cs-1"}'
echo '{"type":"item.delta","text":"par"}'
case "$last" in *slow*) sleep 2;; *die*) exit 3;; *hang*) sleep 60;; esac
echo '{"type":"item.completed","item":{"id":"it1","type":"command_execution","command":"echo ok"}}'
printf '{"type":"item.completed","item":{"id":"it2","type":"agent_message","text":"re: %s"}}\n' "$(echo "$last" | tr '\n' ' ' | tr -d '"')"
echo '{"type":"turn.completed","usage":{"input_tokens":100,"cached_input_tokens":40,"output_tokens":10}}'
