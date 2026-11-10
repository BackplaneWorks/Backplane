#!/bin/bash
# rsync -e wrapper: `rsync -e scripts/nitro-ssh.sh SRC NAME:DEST` reaches Nitro Sandbox NAME through its parent (BP_NITRO_SSH, e.g. ec2-user@host) and
# `bp-nitro pipe` there (scripts/nitro/bp-nitro), as scripts/boat-ssh.sh does for a Boat box. rsync passes [-l user] host command...
while [ "${1:-}" = -l ]; do shift 2; done
n=$1; shift
exec ssh -o BatchMode=yes -o ConnectTimeout=15 -o ServerAliveInterval=15 "${BP_NITRO_SSH:?set BP_NITRO_SSH to the parent instance}" sudo /usr/local/bin/bp-nitro pipe "$n" "$@"
