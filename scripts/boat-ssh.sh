#!/bin/bash
# rsync -e wrapper: `rsync -e scripts/boat-ssh.sh SRC bx_id:DEST` reaches the sandbox through `boat ssh` (rsync passes [-l user] host command...).
while [ "${1:-}" = -l ]; do shift 2; done
h=$1; shift
exec boat --no-update ssh "$h" -- "$@"
