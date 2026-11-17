#!/usr/bin/env bash
# Install the desktop watcher (winvm-watch.sh) as ~/.local/bin/bp-winvm-watch.sh, run by cron every 10 minutes. Re-run after changing winvm-watch.sh. Its log: ~/.cache/bp-winvm/watch.log.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
install -D -m 755 "$here/winvm-watch.sh" "$HOME/.local/bin/bp-winvm-watch.sh"
line="*/10 * * * * $HOME/.local/bin/bp-winvm-watch.sh >/dev/null 2>&1 # bp-winvm-watch"
{ crontab -l 2>/dev/null | grep -v '# bp-winvm-watch$' || true; echo "$line"; } | crontab -
crontab -l | grep bp-winvm-watch
