#!/usr/bin/env bash
# Start bp-winbuild (the 32-core Windows KiCad build VM, $3.41/h while running) and wait for its ssh.
# Hold the winbuild-vm lock for as long as you use it, or the desktop watcher deallocates it after 30
# minutes; the VM's own watchdog deallocates it 20 minutes after its last build process or session:
#
#   scripts/bp-lock.sh winbuild-vm -- sh -c 'scripts/winbuild-up.sh && ssh bp-winbuild "..."; scripts/winbuild-down.sh'
#
# Prints the ssh host to use: first bpadmin@<its Azure public address>, which needs neither the tailnet nor bp-win (port 22 opened to this
# machine's public address alone by the NSG rule winbuild-ssh, closed again by scripts/winbuild-down.sh, or at once when this does not hand the
# direct host out; ssh with the VMs' key and bp-winbuild's host key as already known: scripts/winbuild-kicad.sh adds them); else bp-winbuild
# (Tailscale); else bp-winbuild-vnet (through bp-win). No Tailscale key is ever asked for (the person's rule).
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
# the direct path: the public address and NSG (scripts/winvm/winvm-gate-net.sh made them), and a rule for this machine alone
direct() {
  local me pub
  me=${WINBUILD_MYIP:-$(curl -s -4 --max-time 10 https://api.ipify.org 2>/dev/null)}; case "$me" in *[!0-9.]*|'') echo "$vm: no public address of this machine; the direct path is skipped" >&2; return 1;; esac
  pub=$(az network public-ip show -g "$rg" -n "$vm-gate-ip" --query ipAddress -o tsv 2>/dev/null) && [ -n "$pub" ] || { echo "$vm: no public address ($vm-gate-ip); the direct path is skipped" >&2; return 1; }
  az network nsg rule create -g "$rg" --nsg-name "$vm-gate-nsg" -n winbuild-ssh --priority 110 --direction Inbound --access Allow --protocol Tcp \
    --destination-port-ranges 22 --source-address-prefixes "$me/32" -o none 2>/dev/null || { echo "$vm: could not open ssh to $me in $vm-gate-nsg; the direct path is skipped" >&2; return 1; }
  echo "bpadmin@$pub"; }
# the direct host's ssh options: the VMs' key, and bp-winbuild's host key as already known here (never a first key from a public address)
sshx() { SX=(); case "$1" in *@*) SX=(-i "${WINBUILD_KEY:-$HOME/.ssh/bp-win}" -o IdentitiesOnly=yes -o StrictHostKeyChecking=yes -o HostKeyAlias="$vm");; esac; }
# the rule is this run's until it hands the direct host out: on any other end (a fallback, a failure, a signal) it is closed again
used=; d=
close_rule() { [ -n "$d" ] && [ -z "$used" ] && { az network nsg rule delete -g "$rg" --nsg-name "$vm-gate-nsg" -n winbuild-ssh -o none 2>/dev/null || echo "$vm: could not close the NSG rule winbuild-ssh (the desktop's watcher closes it once the VM is deallocated)" >&2; }; return 0; }
trap close_rule EXIT
d=$(direct) || d=
jump_power() { az vm get-instance-view -g "$rg" -n bp-win --query "instanceView.statuses[?starts_with(code,'PowerState/')].code|[0]" -o tsv 2>/dev/null || true; }
# a VM that was already running has no boot to wait for; a cold start gets 24 tries first (2 to 10 minutes)
[ "$power" = PowerState/running ] && boot_tries=3 || boot_tries=24
for i in $(seq 60); do
  [ -n "$d" ] || [ $((i % 6)) != 0 ] || d=$(direct) || d=          # the direct path's setup failed: tried again every 30 s
  for host in $d bp-winbuild bp-winbuild-vnet; do
    sshx "$host"
    if ssh -o BatchMode=yes -o ConnectTimeout=10 ${SX[@]+"${SX[@]}"} "$host" 'hostname' >/dev/null 2>&1; then [ "$host" != "$d" ] || used=1; echo "$host"; exit 0; fi
  done
  if [ -z "$d" ] && [ "$i" -ge "$boot_tries" ]; then
    jp=$(jump_power)
    case $jp in PowerState/deallocated|PowerState/deallocating|PowerState/stopped|PowerState/stopping)
      echo "$vm: no direct path, and neither Tailscale nor the jump path answers, and bp-win is ${jp#PowerState/}: the jump path needs it (hold its lock for the build: scripts/bp-lock.sh winbuild -- ...)" >&2; exit 1 ;;
    esac # running, starting or unknown (an az error): keep waiting
  fi
  sleep 5
done
echo "$vm: running but ssh does not answer, directly, by Tailscale or through bp-win" >&2; exit 1
