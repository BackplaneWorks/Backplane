#!/bin/bash
# Move a user's data from the old home (~/.backplane-bend) to a Backplane home. The old home is only read; nothing in it is changed or deleted.
#   scripts/migrate-from-bend.sh [--check] [SRC [DEST]]
#   SRC   the old home (default ~/.backplane-bend)
#   DEST  the new home (default ~/.backplane/hub); must be empty or missing
#   --check  say what would be copied and stop
# Copies the event log (threads, messages, settings, projects), attachments, bots, drafts, devices and secrets. Leaves behind what the new hub rebuilds or must not inherit:
# the old hub's store.db and checkpoints, browser/CEF/Electron profiles, caches, locks and pid files. Stop the old hub first (the script refuses while it holds hub.lock).
# Then start the new one: backplane-hub --home DEST --port N. Paired phones keep their pairing (devices-v1.json comes along).
set -eu
CHECK=0; [ "${1:-}" = --check ] && { CHECK=1; shift; }
SRC=${1:-$HOME/.backplane-bend}; DEST=${2:-$HOME/.backplane/hub}
die() { echo "migrate: $*" >&2; exit 1; }
[ -f "$SRC/events.jsonl" ] || die "no events.jsonl in $SRC: nothing to migrate"
if [ -e "$SRC/hub.lock" ] && command -v flock >/dev/null 2>&1; then
  ( flock -n 9 ) 9< "$SRC/hub.lock" || die "the old hub still runs ($(cut -d' ' -f1,2 "$SRC/hub.lock")): stop it first"
fi
[ "$(realpath -m "$SRC")" != "$(realpath -m "$DEST")" ] || die "SRC and DEST are the same"
if [ -d "$DEST" ] && [ -n "$(ls -A "$DEST" 2>/dev/null)" ]; then die "$DEST is not empty"; fi
need=$(du -sk --exclude=store.db* --exclude='*-profile' --exclude=browser.rgb "$SRC" | cut -f1); mkdir -p "$(dirname "$DEST")"
free=$(df -k --output=avail "$(dirname "$DEST")" | tail -1)
[ "$free" -gt $((need * 2)) ] || die "not enough room: need about $((need / 1024)) MB, ${free}K free"
EX=(--exclude='store.db*' --exclude='live-checkpoints' --exclude='hub.lock' --exclude='*.pids' --exclude='*.pid' --exclude='*.lock'
    --exclude='browser-profile' --exclude='cef-profile' --exclude='electron-profile' --exclude='browser.rgb' --exclude='*.cache' --exclude='caches')
if [ "$CHECK" = 1 ]; then echo "would copy $((need / 1024)) MB from $SRC to $DEST:"; rsync -a --dry-run "${EX[@]}" --out-format='%n' "$SRC/" "$DEST/" | awk -F/ '{print $1}' | sort -u; exit 0; fi
mkdir -p "$DEST"; chmod 700 "$DEST"
rsync -a "${EX[@]}" "$SRC/" "$DEST/"
cmp -s "$SRC/events.jsonl" "$DEST/events.jsonl" || die "the copied event log differs from the source; $DEST is incomplete"
echo "migrated $(wc -l < "$DEST/events.jsonl") events into $DEST"
echo "start: backplane-hub --home $DEST --port <port>   (the old home is untouched; delete it yourself when you are satisfied)"
