#!/usr/bin/env bash
# The desktop's watcher over Backplane's Azure Windows VMs, run by cron every 10 minutes
# (scripts/winvm/winvm-cron-install.sh). A third layer under each VM's own idle watchdog
# (bp-idle-watchdog.ps1) and Azure's daily auto-shutdown:
#  - a VM that is stopped but not deallocated (it still bills for compute) is deallocated at once;
#  - a running VM whose locks have all been free for 30 minutes or more is deallocated;
#  - while bp-win's lock is held, its watchdog's hold is kept at least 60 minutes ahead (the VM can't see
#    the lock), never shortened;
#  - bp-win (the UI test box) is first asked (Azure Run Command) whether its watchdog saw an ssh or RDP
#    session, test process, copy, write or hold in the last 30 minutes, and is left running if so.
# A lock is held when someone has a flock on /tmp/bp-<name>.lock: the hub's named locks (lock_acquire,
# work_run lock:) and scripts/bp-lock.sh both take it. Looking it up in /proc/locks never takes it.
#
#   scripts/winvm/winvm-watch.sh            # one pass (what cron runs)
#   scripts/winvm/winvm-watch.sh --dry-run  # say what it would do
set -uo pipefail
export PATH=/usr/local/bin:/usr/bin:/bin:$PATH
rg=${WINVM_RG:-BP-WIN-RG}
free_limit_min=${WINVM_FREE_MINUTES:-30}
state=${WINVM_STATE:-$HOME/.cache/bp-winvm}
dry=; [ "${1:-}" = --dry-run ] && dry=1
mkdir -p "$state"
log() { printf '%s %s\n' "$(date -u +%FT%TZ)" "$*" | tee -a "$state/watch.log"; }
# VM, its locks, whether to ask its watchdog about sessions first
# (bp-win-gate: scripts/land.sh's Windows gate, scripts/land-windows.sh, which runs on bp-win or else bp-winbuild)
vms=(
  "bp-winbuild|winbuild-vm bp-win-gate|no"
  "bp-win|bp-win-ui winbuild bp-win-gate|yes"
)

held() { # held NAME: a flock on /tmp/bp-NAME.lock, from /proc/locks
  local f=/tmp/bp-$1.lock dev maj min ino
  [ -e "$f" ] || return 1
  dev=$((16#$(stat -c %D "$f"))); ino=$(stat -c %i "$f")
  maj=$(( (dev >> 8) & 0xfff )); min=$(( (dev & 0xff) | ((dev >> 12) & 0xfff00) ))
  grep -q " $(printf '%02x:%02x:%d' "$maj" "$min" "$ino") " /proc/locks
}
deallocate() { # deallocate VM WHY
  log "$1: deallocating ($2)"
  [ -n "$dry" ] && { log "$1: dry run, not deallocated"; return; }
  if az vm deallocate -g "$rg" -n "$1" --no-wait -o none 2>>"$state/watch.log"; then
    log "$1: deallocate requested"; rm -f "$state/$1.free-since"
  else log "$1: deallocate FAILED"; fi
}
extend_hold() { # extend_hold VM: its watchdog hold to now + WINVM_LOCK_HOLD_MINUTES, never shorter than it is
  local until out
  until=$(date -u -d "+${WINVM_LOCK_HOLD_MINUTES:-60} minutes" +%FT%TZ)
  [ -n "$dry" ] && { log "$1: dry run, hold not extended to $until"; return; }
  # Run Command, never ssh (an ssh login is activity to the watchdog). Rewrites the file only when the new
  # time is later than the one there, so a lane's longer hold (winvm-hold.sh) is never shortened.
  out=$(timeout 240 az vm run-command invoke -g "$rg" -n "$1" --command-id RunPowerShellScript --scripts "
\$f = 'C:\\ProgramData\\bp-watchdog\\hold-until.txt'; \$new = [datetime]'$until'
\$old = \$null; if (Test-Path \$f) { try { \$old = [datetime](Get-Content \$f -Raw).Trim() } catch { } }
if (\$old -and \$old -ge \$new) { \"kept \$(\$old.ToUniversalTime().ToString('o'))\" }
else { Set-Content -LiteralPath \$f -Value '$until'; 'set $until' }" --query 'value[0].message' -o tsv 2>&1) || out="FAILED: $out"
  log "$1: hold for the lock: $(printf '%s' "$out" | tr -d '\r' | head -c 200)"
}
session_recent() { # session_recent VM: its watchdog saw a session or a job within the free limit
  # Asked through Azure Run Command, never ssh: an ssh login is activity to the watchdog
  local s
  s=$(timeout 240 az vm run-command invoke -g "$rg" -n "$1" --command-id RunPowerShellScript \
      --scripts 'Get-Content C:\ProgramData\bp-watchdog\status.json -Raw' --query 'value[0].message' -o tsv 2>/dev/null) || { echo "watchdog unavailable: left running"; return 0; }
  printf '%s' "$s" | FREE="$free_limit_min" python3 -I -c '
import json, os, sys
from datetime import datetime, timezone
try:
    s = json.loads(sys.stdin.read().lstrip("\ufeff"))
    now = datetime.now(timezone.utc)
    at = datetime.fromisoformat(s["at"])
    if not 0 <= (now - at).total_seconds() <= 600:
        print("watchdog stale: left running"); sys.exit(0)
    if s.get("dryRun"):
        print("watchdog dry run: left running"); sys.exit(0)
    if s["busy"]:
        print(s.get("why") or "busy"); sys.exit(0)
    busy = s.get("lastBusy")
    if busy and (now - datetime.fromisoformat(busy)).total_seconds() < int(os.environ["FREE"]) * 60:
        print(s.get("why") or "busy %s" % busy); sys.exit(0)
except (ValueError, KeyError, TypeError):
    print("watchdog invalid: left running"); sys.exit(0)
sys.exit(1)'
}

for entry in "${vms[@]}"; do
  IFS='|' read -r vm locks ask <<<"$entry"
  power=$(az vm get-instance-view -g "$rg" -n "$vm" \
    --query "instanceView.statuses[?starts_with(code,'PowerState/')].code|[0]" -o tsv 2>>"$state/watch.log")
  power=${power#PowerState/}
  case "$power" in
    deallocated|deallocating) rm -f "$state/$vm.free-since"; continue ;;
    stopped|stopping) deallocate "$vm" "powered off but not deallocated: it still bills"; continue ;;
    running|starting) ;;
    *) log "$vm: power state unknown ('$power')"; continue ;;
  esac
  holder=
  for l in $locks; do held "$l" && { holder=$l; break; }; done
  if [ -n "$holder" ]; then
    rm -f "$state/$vm.free-since"; log "$vm: $power, lock $holder held"
    # The VM can't see the desktop's locks: while one is held, keep its watchdog's hold at least
    # WINVM_LOCK_HOLD_MINUTES (60) ahead, so it outlasts the lock by 30 minutes or more (this runs every 10)
    [ "$ask" = yes ] && extend_hold "$vm"
    continue
  fi
  [ -f "$state/$vm.free-since" ] || date +%s > "$state/$vm.free-since"
  free_min=$(( ($(date +%s) - $(cat "$state/$vm.free-since")) / 60 ))
  if [ "$free_min" -lt "$free_limit_min" ]; then log "$vm: $power, locks ($locks) free for $free_min min"; continue; fi
  if [ "$ask" = yes ] && why=$(session_recent "$vm"); then
    # Live activity and holds have no hard cap. Azure's 07:00 UTC shutdown remains bp-win's backstop.
    log "$vm: locks free for $free_min min, but its watchdog saw: $why; left running"; continue
  fi
  deallocate "$vm" "locks ($locks) free for $free_min min"
done
tail -n 2000 "$state/watch.log" > "$state/watch.log.tmp" && mv "$state/watch.log.tmp" "$state/watch.log"
