#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Exercise the desktop watcher with Azure and lock stand-ins; never contacts a VM.
set -euo pipefail
HERE=$(cd "$(dirname "$0")/.." && pwd)
. "$HERE/scripts/lib-bptmp.sh"
T=$(mktemp -d "$BP_TMP/bp-winvm-proof-XXXX")
trap 'rm -rf "$T"' EXIT
# Source definitions, replacing external commands before running the unchanged main loop.
sed '/^for entry in /,$d' "$HERE/scripts/winvm/winvm-watch.sh" > "$T/functions"
sed -n '/^for entry in /,$p' "$HERE/scripts/winvm/winvm-watch.sh" > "$T/loop"
export WINVM_STATE=$T/state
. "$T/functions"
held() { [[ ${HELD:-} == "$1" ]]; }
az() {
  case "$*" in
    *get-instance-view*) echo PowerState/running ;;
    *run-command*)
      if [[ $* == *hold-until* ]]; then printf '%s\n' "$*" >> "$T/holds"; echo kept
      elif [[ $STATUS == unavailable ]]; then return 1
      else printf '%s' "$STATUS"; fi ;;
    *deallocate*) printf '%s\n' "$*" >> "$T/deallocations" ;;
    *) echo "unexpected Azure call $*" >&2; return 1 ;;
  esac
}
# timeout is only a wrapper around our Azure stand-in.
timeout() { shift; "$@"; }
run_case() {
  local scenario=$1 expected=$2
  rm -f "$T/deallocations" "$T/holds"
  mkdir -p "$WINVM_STATE"
  date -d '5 hours ago' +%s > "$WINVM_STATE/bp-win.free-since"
  date -d '5 hours ago' +%s > "$WINVM_STATE/bp-winbuild.free-since"
  . "$T/loop" > "$T/output"
  if [[ $expected == keep ]]; then
    if [[ -f $T/deallocations ]] && grep -q -- '-n bp-win ' "$T/deallocations"; then cat "$T/output"; echo "FAIL $scenario"; exit 1; fi
  else
    grep -q -- '-n bp-win ' "$T/deallocations" || { cat "$T/output"; echo "FAIL $scenario"; exit 1; }
  fi
  echo "PASS $scenario"
}
now=$(date -u +%FT%TZ)
old=$(date -u -d '5 hours ago' +%FT%TZ)
STATUS="{\"at\":\"$now\",\"busy\":true,\"why\":\"process python\",\"lastBusy\":\"$now\"}"
run_case 'active Python after five hours' keep
STATUS="{\"at\":\"$now\",\"busy\":false,\"lastBusy\":\"$now\"}"
run_case 'recent activity' keep
STATUS="{\"at\":\"$now\",\"busy\":true,\"why\":\"hold until tomorrow\"}"
run_case 'hold without lastBusy' keep
STATUS="{\"at\":\"$now\",\"busy\":false,\"dryRun\":true,\"lastBusy\":\"$old\"}"
run_case 'dry run blocks desktop shutdown too' keep
STATUS="{\"at\":\"$old\",\"busy\":false,\"lastBusy\":\"$old\"}"
run_case 'stale watchdog' keep
STATUS=broken
run_case 'invalid watchdog' keep
STATUS=unavailable
run_case 'unavailable watchdog' keep
STATUS="{\"at\":\"$now\",\"busy\":false,\"lastBusy\":\"$old\"}"
run_case 'fresh confirmed idle can deallocate' stop
HELD=bp-win-ui
run_case 'UI lock keeps VM up and extends hold' keep
[[ -s $T/holds ]] || { echo 'FAIL no hold refresh'; exit 1; }
HELD=bp-win-gate
run_case 'Windows gate lock keeps VM up and extends hold' keep
[[ -s $T/holds ]] || { echo 'FAIL no hold refresh for the gate'; exit 1; }
HELD=
dry=1
run_case 'desktop dry run never deallocates' keep
[[ ! -f $T/deallocations ]] || { echo 'FAIL dry-run mutation'; exit 1; }
echo 'PASS desktop watcher proofs (Azure and locks simulated)'
