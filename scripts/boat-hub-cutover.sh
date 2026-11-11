#!/bin/bash
# The Boat hub cutover, one step at a time (docs/BOAT-HUB.md). Each step says what it did and what it needs next; none runs without its name.
# Only with the person's go. Run from the desktop.
#
#   scripts/boat-hub-cutover.sh plan            print the steps (default)
#   scripts/boat-hub-cutover.sh backup          desktop home + apns config to ~/.backplane-gpui-fresh/backup-<date>/, and a Boat snapshot
#   scripts/boat-hub-cutover.sh link | link-join CODE | approve   pair the machines: an invite on the box, the desktop joins, approved on the box
#   scripts/boat-hub-cutover.sh join            the box joins the uberbot the desktop offers (bots.uber_join)
#   scripts/boat-hub-cutover.sh home            the box takes Bub's lease (bots.uber_home); the desktop steps down at the next exchange
#   scripts/boat-hub-cutover.sh carry           the other bots, rooms and hooks (ids kept) with their files, the vault, the ring and APNs config
#                                               to the box (box hub stopped for the append, started again); writes the retire list
#   scripts/boat-hub-cutover.sh retire          switch the same off on the desktop (bots.delete, bots.room_delete, bots.hook_revoke) and set
#                                               ring.home there; the ring's URL then has to be changed in the Pebble app at once
#   scripts/boat-hub-cutover.sh check           what each hub holds now
#   scripts/boat-hub-cutover.sh rollback        the lease back to the desktop, the retired things restored there, the box hub stopped
set -euo pipefail
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd); ROOT=$(cd "$HERE/.." && pwd)
DH=${DESK_HOME:-$HOME/.backplane-gpui-fresh/home}; DURL=${DESK_URL:-ws://127.0.0.1:3797}
BOX=boat-hub.tail9e0cf2.ts.net; R=/home/user/bp; BH=$R/home
ST=$HOME/.config/backplane/boat-hub/cutover; mkdir -p "$ST"
ID=$(cat "$HOME/.config/backplane/boat-hub/id")
say() { echo "cutover: $*" >&2; }
on() { boat --no-update ssh "$ID" -- "$@"; }
drpc() { bun "$HERE/hub-rpc.mjs" "$DH/token" "$DURL" "$@"; }
# an RPC to the box hub, made on the box (its loopback; the client files are copied there first)
brpc() { on "cd ~/bp-rpc && node scripts/hub-rpc.mjs $BH/token ws://127.0.0.1:3797 $1 '${2:-{\}}'"; }
rpc_files() { on "mkdir -p ~/bp-rpc" > /dev/null; (cd "$ROOT" && rsync -rlpR -e "$HERE/boat-ssh.sh" test/wire.mjs test/wire-codec.js native/host/src/wire_dict.rs scripts/hub-rpc.mjs "$ID:/home/user/bp-rpc/" > /dev/null); }
done_mark() { date -Is > "$ST/$1"; }
need() { [ -f "$ST/$1" ] || { say "do '$1' first"; exit 1; }; }

case "${1:-plan}" in
plan) sed -n '2,15p' "$0" | sed 's/^# \{0,1\}//'; ls "$ST" 2>/dev/null | sed 's/^/done: /';;

backup)
  B=$HOME/.backplane-gpui-fresh/backup-$(date +%Y%m%d-%H%M); mkdir -p "$B"
  say "copying the desktop home to $B (the hub keeps running; the log is append-only, so a copy is consistent up to its last whole line)"
  rsync -a --exclude=/voice --exclude=/browser-profile --exclude=/tmp "$DH/" "$B/home/"
  mkdir -p "$B/config"; cp -a "$HOME/.config/backplane/apns.env" "$HOME/.config/backplane/apns" "$B/config/" 2>/dev/null || true
  [ -d "$HOME/.backplane/bots" ] && rsync -a "$HOME/.backplane/bots/" "$B/bots/"
  boat --no-update snapshot "$ID" "bp-hub-before-$(date +%Y%m%d)" > /dev/null && say "Boat snapshot bp-hub-before-$(date +%Y%m%d)"
  echo "$B" > "$ST/backup"; say "backup: $B ($(du -sh "$B" | cut -f1))";;

link)
  need backup
  say "box invite (the code is entered on the desktop with: scripts/boat-hub-cutover.sh link-join CODE):"
  on "BACKPLANE_HOME=$BH $R/backplane pair --machine";;
link-join)
  need backup; [ -n "${2:-}" ] || { say "link-join CODE"; exit 2; }
  "$HOME/.backplane-gpui-dev/builds/$(head -1 "$HOME/.backplane-gpui-dev/CURRENT")/backplane" pair --home "$DH" --join "https://$BOX" "$2"
  say "box requests (compare the check code with the desktop's, then: scripts/boat-hub-cutover.sh approve):"
  on "BACKPLANE_HOME=$BH $R/backplane pair --requests";;
approve)
  req=$(on "BACKPLANE_HOME=$BH $R/backplane pair --requests" | grep -o -E 'id [^ ]+' | head -1 | cut -d' ' -f2)
  [ -n "$req" ] || { say "no request waiting on the box"; exit 1; }
  on "BACKPLANE_HOME=$BH $R/backplane pair --approve $req" && done_mark link;;

join)
  need link; rpc_files
  uid=$(on "grep -h '\"uber.offer\"' $BH/events.jsonl | tail -1" | python3 -I -c 'import sys,json
l=sys.stdin.read().strip(); print(json.loads(json.loads(l)["value"])["uid"] if l else "")')
  [ -n "$uid" ] || { say "the box has no offer yet (the first exchange makes it, within a minute of the link)"; exit 1; }
  brpc bots.uber_join "{\"uid\":\"$uid\"}" && done_mark join
  say "joined $uid; Bub's memory, space and routines arrive by sync (scripts/boat-hub-cutover.sh check)";;

home)
  need join; rpc_files; brpc bots.uber_home && done_mark home
  say "the box holds the lease; the desktop steps down at its next exchange (within a minute)";;

carry)
  need home; rpc_files
  W=$ST/carry; mkdir -p "$W"; cp "$DH/events.jsonl" "$W/desk.jsonl"
  rep=$(on "grep -h '\"UberSet\"' $BH/events.jsonl | tail -1" | python3 -I -c 'import sys,json; l=sys.stdin.read().strip(); print(json.loads(l)["bot"] if l else "")')
  [ -n "$rep" ] || { say "no uberbot replica on the box"; exit 1; }
  python3 -I "$HERE/boat-hub-carry.py" "$W/desk.jsonl" "$W" --uber-to "$rep"
  say "stopping the box hub for the append"; on "sudo -n systemctl stop bp-hub"
  on "cp $BH/events.jsonl $BH/events.jsonl.before-carry"
  on "cat >> $BH/events.jsonl" < "$W/carry.jsonl"
  # their files: bots' folders (the box keeps bots under ~/.backplane/bots), hook secrets, the ring, the vault, APNs
  while read -r f; do [ -n "$f" ] || continue
    case "$f" in /*) src=$f; dst=/home/user/.backplane/bots/$(basename "$f");; *) src=$DH/$f; dst=$BH/$f;; esac
    [ -e "$src" ] || continue; on "mkdir -p $(dirname "$dst")"
    rsync -a -e "$HERE/boat-ssh.sh" "$src" "$ID:$(dirname "$dst")/" > /dev/null; done < "$W/files.txt"
  on "mkdir -p $BH/secrets ~/.config/backplane && cp $BH/secrets/vault.json $BH/secrets/vault.json.box 2>/dev/null; cp $BH/secrets/vault.key $BH/secrets/vault.key.box 2>/dev/null; true"
  rsync -a -e "$HERE/boat-ssh.sh" "$DH/secrets/vault.json" "$DH/secrets/vault.key" "$ID:$BH/secrets/" > /dev/null
  rsync -a -e "$HERE/boat-ssh.sh" "$HOME/.config/backplane/apns.env" "$HOME/.config/backplane/apns" "$ID:/home/user/.config/backplane/" > /dev/null
  on "chmod -R go-rwx $BH/secrets ~/.config/backplane; sudo -n systemctl start bp-hub"
  done_mark carry; say "carried: $(head -c 2000 "$W/summary.txt" | tail -1); next: retire (then the Pebble app's URL)";;

retire)
  need carry; W=$ST/carry
  python3 -I -c 'import json,sys; [print(json.dumps(x)) for x in json.load(open(sys.argv[1]))]' "$W/retire.json" | while read -r x; do
    m=$(printf %s "$x" | python3 -I -c 'import sys,json; print(json.load(sys.stdin)["m"])'); p=$(printf %s "$x" | python3 -I -c 'import sys,json; print(json.dumps(json.load(sys.stdin)["p"]))')
    drpc "$m" "$p" > /dev/null && say "desktop: $m $p" || say "desktop: $m $p FAILED"; done
  hook=$(grep -o -E '^hook [^ ]+' "$W/summary.txt" | head -1 | cut -d' ' -f2 || true)
  if [ -n "$hook" ]; then drpc setting.set "{\"key\":\"ring.home\",\"value\":\"{\\\"url\\\":\\\"https://$BOX/hook/$hook\\\",\\\"machine\\\":\\\"boat-hub\\\"}\"}" > /dev/null
    say "ring.home set on the desktop: https://$BOX/hook/$hook. NOW: Pebble app, Index 01 Settings, Webhook, URL = that, Save, Send test event"; fi
  [ -d "$DH/ring" ] && mv "$DH/ring" "$DH/ring.moved-$(date +%Y%m%d)"
  done_mark retire;;

check)
  say "desktop:"; drpc update.status "{}" | head -c 300; echo; grep -h -o '"key":"uber.r.[^"]*lease"[^}]*' "$DH/events.jsonl" | tail -1
  say "box:"; on "systemctl is-active bp-hub; curl -s -m5 http://127.0.0.1:3797/healthz; echo; grep -c . $BH/events.jsonl; grep -h -o '\"key\":\"uber.r.[^\"]*lease\"' $BH/events.jsonl | tail -1";;

rollback)
  # Bub, the bots and rooms back on the desktop: the box hub stopped first (so nothing runs twice), then the desktop's own last
  # BotSet/RoomSet/UberSet lines appended to its log while ITS hub is stopped (no RPC commits raw lines), then the person starts the app again.
  on "sudo -n systemctl disable --now bp-hub" && say "box hub stopped and disabled (its home kept: $BH)"
  python3 -I -c 'import json,sys; [print(json.dumps(x, separators=(",",":"))) for x in json.load(open(sys.argv[1]))]' "$ST/carry/restore.json" > "$ST/carry/restore.jsonl"
  if [ -e "$DH/hub.lock" ] && kill -0 "$(awk '{print $1}' "$DH/hub.lock")" 2>/dev/null; then
    say "the desktop hub is running: stop it (backplane hub stop, at the desktop), then run rollback again; the box is already stopped"; exit 1; fi
  cp "$DH/events.jsonl" "$DH/events.jsonl.before-rollback"; cat "$ST/carry/restore.jsonl" >> "$DH/events.jsonl"
  say "restored $(wc -l < "$ST/carry/restore.jsonl") lines (Bub as uberbot, the bots and rooms); start the app again. Backup of everything: $(cat "$ST/backup")"
  rm -f "$ST"/{carry-done,retire-done};;
*) "$0" plan; exit 2;;
esac
