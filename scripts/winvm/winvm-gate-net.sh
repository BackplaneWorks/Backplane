#!/usr/bin/env bash
# One-time setup of the Windows gate's direct path to bp-winbuild (scripts/land-windows.sh), so it needs no tailnet:
#  - a Standard static public IP on the VM's NIC (Standard is closed by default: nothing reaches it without an NSG rule);
#  - an NSG on the NIC with no rule of its own: Azure's defaults allow the VNet (bp-win's path, bp-winbuild-vnet) and deny the internet.
# land-windows.sh adds the rule land-gate-ssh (TCP 22 from this machine's public address only) for each run and deletes it at the end.
#   scripts/winvm/winvm-gate-net.sh            # make or check them; prints the address
#   scripts/winvm/winvm-gate-net.sh --remove   # take both off and delete them
set -euo pipefail
rg=${WINVM_RG:-BP-WIN-RG}; vm=${WINBUILD_VM:-bp-winbuild}; ip=$vm-gate-ip; nsg=$vm-gate-nsg
loc=$(az vm show -g "$rg" -n "$vm" --query location -o tsv)   # the VM's region (the group's default differs)
nic=$(az vm show -g "$rg" -n "$vm" --query 'networkProfile.networkInterfaces[0].id' -o tsv)
cfg=$(az network nic show --ids "$nic" --query 'ipConfigurations[0].name' -o tsv)
if [ "${1:-}" = --remove ]; then
  az network nic ip-config update -g "$rg" --nic-name "${nic##*/}" -n "$cfg" --remove publicIpAddress -o none
  az network nic update --ids "$nic" --remove networkSecurityGroup -o none
  az network public-ip delete -g "$rg" -n "$ip"; az network nsg delete -g "$rg" -n "$nsg"; echo removed; exit 0
fi
az network nsg show -g "$rg" -n "$nsg" -o none 2>/dev/null || az network nsg create -g "$rg" -n "$nsg" -l "$loc" -o none
az network public-ip show -g "$rg" -n "$ip" -o none 2>/dev/null || az network public-ip create -g "$rg" -n "$ip" -l "$loc" --sku Standard --allocation-method Static --version IPv4 -o none
az network nic update --ids "$nic" --network-security-group "$nsg" -o none
az network nic ip-config update -g "$rg" --nic-name "${nic##*/}" -n "$cfg" --public-ip-address "$ip" -o none
az network public-ip show -g "$rg" -n "$ip" --query ipAddress -o tsv
