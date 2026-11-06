#!/bin/sh
# One heavy job at a time across every lane, at low priority (the hub's scope ran out of memory once from parallel builds): scripts/mobile/heavy.sh CMD ARGS...
# (scripts/heavy-command.sh is the stricter guard; this is the flock fallback docs/heavy-local-work.md allows when memory is short.)
exec flock -w 3600 /tmp/bp-heavy.lock nice -n 10 "$@"
