#!/usr/bin/env bash
# Start bp-winbuild (the 32-core Windows KiCad build VM, $3.41/h while running) and wait for its ssh.
# Hold the winbuild-vm lock for as long as you use it, or the desktop watcher deallocates it after 30
# minutes; the VM's own watchdog deallocates it 20 minutes after its last build process or session:
#
#   scripts/bp-lock.sh winbuild-vm -- sh -c 'scripts/winbuild-up.sh && ssh bp-winbuild "..."; scripts/winbuild-down.sh'
#
# Prints the ssh host to use: bp-winbuild (Tailscale), else bp-winbuild-vnet (through bp-win).
set -euo pipefail
vm=${WINBUILD_VM:-bp-winbuild}; rg=${WINVM_RG:-BP-WIN-RG}
power=$(az vm get-instance-view -g "$rg" -n "$vm" --query "instanceView.statuses[?starts_with(code,'PowerState/')].code|[0]" -o tsv)
if [ "$power" != PowerState/running ]; then
  echo "$vm: ${power#PowerState/}, starting" >&2
  az vm start -g "$rg" -n "$vm" -o none
fi
for _ in $(seq 60); do
  for host in bp-winbuild bp-winbuild-vnet; do
    if ssh -o BatchMode=yes -o ConnectTimeout=10 "$host" 'hostname' >/dev/null 2>&1; then echo "$host"; exit 0; fi
  done
  sleep 5
done
echo "$vm: running but ssh does not answer" >&2; exit 1
