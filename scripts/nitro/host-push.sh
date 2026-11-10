#!/bin/bash
# host-push.sh DEST: put Backplane's side of the Nitro parent on it (DEST: ec2-user@host, the same as the setting sandbox.nitro.ssh) and build the guest images.
# Run from a checkout, after the parent's first boot (scripts/nitro/host-setup.sh). Copies only scripts/nitro and scripts/boat-prune.py: nothing of the person's.
set -eu; cd "$(dirname "$0")/../.."; D=${1:?usage: host-push.sh ec2-user@host}
ssh -o BatchMode=yes "$D" 'mkdir -p ~/bp-nitro-src/scripts/nitro'
rsync -rlp --checksum scripts/nitro/ "$D:bp-nitro-src/scripts/nitro/"; rsync -p scripts/boat-prune.py "$D:bp-nitro-src/scripts/"
ssh -o BatchMode=yes "$D" 'sudo install -m 755 ~/bp-nitro-src/scripts/nitro/bp-nitro ~/bp-nitro-src/scripts/nitro/bp-vsock.py /usr/local/bin/ && uname -r && sudo dmesg | grep "nitro_enclaves: CMA pool" | tail -1; sudo bash ~/bp-nitro-src/scripts/nitro/guest-build.sh ~/bp-nitro-src && sudo bp-nitro status'
