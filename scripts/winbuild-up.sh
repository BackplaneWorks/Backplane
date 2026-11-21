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
# Two ways in: Tailscale (bp-winbuild), else the vnet through bp-win (bp-winbuild-vnet, ProxyJump bp-win).
# When neither answers once the guest has had time to boot, and bp-win is down, the jump path cannot come:
# say so instead of waiting out the loop (2026-11-21 UTC: Tailscale on bp-winbuild logged out, bp-win
# deallocated by its idle watcher, two build attempts lost). Keep bp-win up for a build by holding its
# `winbuild` lock (scripts/bp-lock.sh winbuild -- ...), which the desktop watcher keys on.
jump_power() { az vm get-instance-view -g "$rg" -n bp-win --query "instanceView.statuses[?starts_with(code,'PowerState/')].code|[0]" -o tsv 2>/dev/null || true; }
# a VM that was already running has no boot to wait for; a cold start gets ~4 minutes first
[ "$power" = PowerState/running ] && boot_tries=3 || boot_tries=24
for i in $(seq 60); do
  for host in bp-winbuild bp-winbuild-vnet; do
    if ssh -o BatchMode=yes -o ConnectTimeout=10 "$host" 'hostname' >/dev/null 2>&1; then echo "$host"; exit 0; fi
  done
  if [ "$i" -ge "$boot_tries" ]; then
    jp=$(jump_power)
    case $jp in PowerState/deallocated|PowerState/deallocating|PowerState/stopped|PowerState/stopping)
      echo "$vm: neither Tailscale nor the jump path answers, and bp-win is ${jp#PowerState/}: the jump path needs it (hold its lock for the build: scripts/bp-lock.sh winbuild -- ...)" >&2; exit 1 ;;
    esac # running, starting or unknown (an az error): keep waiting
  fi
  sleep 5
done
echo "$vm: running but ssh does not answer, by Tailscale or through bp-win" >&2; exit 1
