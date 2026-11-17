#!/usr/bin/env bash
# Keep a VM's idle watchdog from deallocating it until a time (at most 12 h ahead), e.g. bp-win for a
# release window whose Windows jobs its self-hosted runner takes. Starts the VM if it is deallocated.
#
#   scripts/winvm/winvm-hold.sh bp-win 6h      # hold for 6 hours
#   scripts/winvm/winvm-hold.sh bp-win off     # end the hold
# Azure's daily auto-shutdown still applies: suspend it for the window as the release playbook says.
set -euo pipefail
vm=${1:?usage: winvm-hold.sh VM DURATION|off}; dur=${2:?duration (e.g. 90m, 6h) or off}
rg=${WINVM_RG:-BP-WIN-RG}
if [ "$dur" = off ]; then script='Remove-Item -Force C:\ProgramData\bp-watchdog\hold-until.txt -ErrorAction SilentlyContinue; "hold off"'
else
  case $dur in *h) s=$(( ${dur%h} * 3600 )) ;; *m) s=$(( ${dur%m} * 60 )) ;; *) echo 'duration: Nm or Nh' >&2; exit 2 ;; esac
  [ "$s" -le 43200 ] || { echo 'at most 12h' >&2; exit 2; }
  until=$(date -u -d "+$s seconds" +%FT%TZ)
  script="Set-Content -LiteralPath C:\\ProgramData\\bp-watchdog\\hold-until.txt -Value '$until'; 'held until $until'"
fi
power=$(az vm get-instance-view -g "$rg" -n "$vm" --query "instanceView.statuses[?starts_with(code,'PowerState/')].code|[0]" -o tsv)
[ "$power" = PowerState/running ] || { [ "$dur" = off ] && { echo "$vm: ${power#PowerState/}, nothing to release"; exit 0; }; echo "$vm: starting" >&2; az vm start -g "$rg" -n "$vm" -o none; }
az vm run-command invoke -g "$rg" -n "$vm" --command-id RunPowerShellScript --scripts "$script" --query 'value[0].message' -o tsv
