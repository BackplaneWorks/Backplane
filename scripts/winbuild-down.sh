#!/usr/bin/env bash
# Deallocate bp-winbuild now (the compute meter stops; its disks stay, $52.48/month). Only "VM
# deallocated" stops the bill: a VM shut down from inside Windows is "stopped" and still billed.
set -euo pipefail
vm=${WINBUILD_VM:-bp-winbuild}; rg=${WINVM_RG:-BP-WIN-RG}
# the direct path's port 22 (winbuild-up.sh), closed; a failed delete is said, and makes this exit 1 once the VM is deallocated
rc=0; az network nsg rule delete -g "$rg" --nsg-name "$vm-gate-nsg" -n winbuild-ssh -o none 2>/dev/null || { echo "$vm: could not delete the NSG rule winbuild-ssh: delete it (az network nsg rule delete -g $rg --nsg-name $vm-gate-nsg -n winbuild-ssh); the desktop's watcher also does once the VM is deallocated" >&2; rc=1; }
az vm deallocate -g "$rg" -n "$vm" -o none
az vm get-instance-view -g "$rg" -n "$vm" --query "instanceView.statuses[?starts_with(code,'PowerState/')].displayStatus|[0]" -o tsv

exit $rc
