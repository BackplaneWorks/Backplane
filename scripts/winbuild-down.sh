#!/usr/bin/env bash
# Deallocate bp-winbuild now (the compute meter stops; its disks stay, $52.48/month). Only "VM
# deallocated" stops the bill: a VM shut down from inside Windows is "stopped" and still billed.
set -euo pipefail
vm=${WINBUILD_VM:-bp-winbuild}; rg=${WINVM_RG:-BP-WIN-RG}
az vm deallocate -g "$rg" -n "$vm" -o none
az vm get-instance-view -g "$rg" -n "$vm" --query "instanceView.statuses[?starts_with(code,'PowerState/')].displayStatus|[0]" -o tsv
