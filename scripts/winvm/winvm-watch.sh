#!/usr/bin/env bash
# The desktop's watcher over Backplane's Azure Windows VMs, run by cron every 30 minutes
# (scripts/winvm/winvm-cron-install.sh). A third layer under each VM's own idle watchdog
# (bp-idle-watchdog.ps1) and Azure's daily auto-shutdown:
#  - a VM that is stopped but not deallocated (it still bills for compute) is deallocated at once;
#  - a running VM whose locks have all been free for 30 minutes or more is deallocated;
#  - bp-win (the UI test box) is first asked (Azure Run Command) whether its watchdog saw an ssh or RDP
#    session or a job in the last 30 minutes, and is left running if so, but never past 3 hours with its locks free.
# A lock is held when someone has a flock on /tmp/bp-<name>.lock: the hub's named locks (lock_acquire,
# work_run lock:) and scripts/bp-lock.sh both take it. Looking it up in /proc/locks never takes it.
#
#   scripts/winvm/winvm-watch.sh            # one pass (what cron runs)
#   scripts/winvm/winvm-watch.sh --dry-run  # say what it would do
set -uo pipefail
export PATH=/usr/local/bin:/usr/bin:/bin:$PATH
rg=${WINVM_RG:-BP-WIN-RG}
free_limit_min=${WINVM_FREE_MINUTES:-30}
hard_limit_min=${WINVM_HARD_MINUTES:-180}
state=${WINVM_STATE:-$HOME/.cache/bp-winvm}
dry=; [ "${1:-}" = --dry-run ] && dry=1
mkdir -p "$state"
log() { printf '%s %s\n' "$(date -u +%FT%TZ)" "$*" | tee -a "$state/watch.log"; }
# VM, its locks, whether to ask its watchdog about sessions first
vms=(
  "bp-winbuild|winbuild-vm|no"
  "bp-win|bp-win-ui winbuild|yes"
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
session_recent() { # session_recent VM: its watchdog saw a session or a job within the free limit
  # Asked through Azure Run Command, never ssh: an ssh login is activity to the watchdog
  local s
  s=$(timeout 240 az vm run-command invoke -g "$rg" -n "$1" --command-id RunPowerShellScript \
      --scripts 'Get-Content C:\ProgramData\bp-watchdog\status.json -Raw' --query 'value[0].message' -o tsv 2>/dev/null) || return 1
  printf '%s' "$s" | FREE="$free_limit_min" python3 -I -c '
import json, os, sys
from datetime import datetime, timezone
s = json.loads(sys.stdin.read().lstrip("\ufeff"))
now = datetime.now(timezone.utc)
at = datetime.fromisoformat(s["at"])
if (now - at).total_seconds() > 600: sys.exit(1)       # a stale status: its watchdog is not running
busy = s.get("lastBusy")
if busy and (now - datetime.fromisoformat(busy)).total_seconds() < int(os.environ["FREE"]) * 60:
    print(s.get("why") or "busy %s" % busy); sys.exit(0)
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
  if [ -n "$holder" ]; then rm -f "$state/$vm.free-since"; log "$vm: $power, lock $holder held"; continue; fi
  [ -f "$state/$vm.free-since" ] || date +%s > "$state/$vm.free-since"
  free_min=$(( ($(date +%s) - $(cat "$state/$vm.free-since")) / 60 ))
  if [ "$free_min" -lt "$free_limit_min" ]; then log "$vm: $power, locks ($locks) free for $free_min min"; continue; fi
  if [ "$ask" = yes ] && why=$(session_recent "$vm"); then
    # A hold (winvm-hold.sh: a release window) outlasts the 3-hour cap; a session does not
    if [ "$free_min" -lt "$hard_limit_min" ] || [[ $why == *"hold until"* ]]; then
      log "$vm: locks free for $free_min min, but its watchdog saw: $why; left running"; continue
    fi
  fi
  deallocate "$vm" "locks ($locks) free for $free_min min"
done
tail -n 2000 "$state/watch.log" > "$state/watch.log.tmp" && mv "$state/watch.log.tmp" "$state/watch.log"
