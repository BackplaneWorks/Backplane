#!/usr/bin/env bash
# Bring up the self-hosted release runners a release.yml run will ask for, hold each machine's lock while its jobs run, and let go of them after
# (docs/release-runners.md). GitHub cannot start a deallocated Azure VM nor the Mac mini's runner (not a service), so this runs on the desktop:
#
#   scripts/release-dispatch.sh dispatch [--ref main] [-f tag=v0.0.0-dryrun.N ...]   start release.yml by hand (a dry run) and see it through
#   scripts/release-dispatch.sh watch RUN_ID                                          the same for a run already started (a tag push, a nightly)
#
# Which machines: the repo variables. RELEASE_WIN_RUNNER=bp-winbuild-release starts bp-winbuild (scripts/winbuild-up.sh) under the winbuild-vm lock
# (scripts/bp-lock.sh) and holds it until the run's windows-x64 build ends; the desktop's watcher deallocates the VM 30 min after the lock is free.
# RELEASE_MAC_RUNNER=bp-mac starts ~/bp-runner/run.sh on the Mac mini under mac-heavy (ssh only: nothing of the person's session) and stops it after
# the darwin-arm64 build. RELEASE_LINUX_RUNNER needs nothing: the desktop's runner container is always up (bp-ci-runner.service).
# Prints each build job's minutes at the end.
set -euo pipefail
repo=BackplaneWorks/Backplane; here=$(cd "$(dirname "$0")" && pwd)
mode=${1:?usage: release-dispatch.sh dispatch [--ref REF] [-f k=v ...] | watch RUN_ID}; shift
# gh 2.46 has no `gh variable get`: read the list once (a failure to read is fatal, never "unset": an empty answer started no machine on rc.2's first run)
vars=$(gh variable list -R "$repo" --json name,value 2>&1) || { echo "release-dispatch: cannot read the repo variables: $vars" >&2; exit 1; }
var() { printf '%s' "$vars" | python3 -c 'import json,sys; print(next((v["value"] for v in json.load(sys.stdin) if v["name"] == sys.argv[1]), ""), end="")' "$1"; }
# the variables, or the same names in the environment (a branch's test run, where the workflow takes the labels as inputs)
win=${RELEASE_WIN_RUNNER-$(var RELEASE_WIN_RUNNER)}; mac=${RELEASE_MAC_RUNNER-$(var RELEASE_MAC_RUNNER)}; lin=${RELEASE_LINUX_RUNNER-$(var RELEASE_LINUX_RUNNER)}
box=$(cat "$HOME/.config/bp-ci/linux-box" 2>/dev/null || true)
# a RELEASE_* value this script cannot start a machine for is an error, never a job left queued
[ -z "$win" ] || [ "$win" = bp-winbuild-release ] || [ "$win" = bp-win-nightly ] || { echo "release-dispatch: RELEASE_WIN_RUNNER=$win is not a runner this script knows" >&2; exit 1; }
[ -z "$mac" ] || [ "$mac" = bp-mac ] || { echo "release-dispatch: RELEASE_MAC_RUNNER=$mac is not a runner this script knows" >&2; exit 1; }
[ -z "$lin" ] || [ "$lin" = bp-linux-release ] || { echo "release-dispatch: RELEASE_LINUX_RUNNER=$lin is not a runner this script knows" >&2; exit 1; }
[ "$win" != bp-win-nightly ] || echo "release-dispatch: bp-win-nightly is started by hand (this script starts no bp-win); make sure it is up" >&2
# the desktop's container is the linux runner when no Boat box is named: it must be running
if [ "$lin" = bp-linux-release ] && [ -z "$box" ] && [ "$mode" != _resolve ]; then
  [ -n "$(docker ps -q -f name=bp-linux-release-runner 2>/dev/null)" ] || { echo "release-dispatch: RELEASE_LINUX_RUNNER is set, but the bp-linux-release-runner container is not running here and no Boat box is named" >&2; exit 1; }
fi
[ "$mode" != _resolve ] || { echo "win=$win mac=$mac lin=$lin"; exit 0; }

# the job "build <target>" of run $1: its status (queued, in_progress, completed) or "none"
job_status() { gh run view "$1" -R "$repo" --json jobs -q ".jobs[] | select(.name == \"build $2\") | .status" 2>/dev/null | head -1 | grep . || echo none; }
# wait for the run's build of target $2 to end (or the run to end without one)
wait_job() {
  local run=$1 t=$2 s
  while :; do
    s=$(job_status "$run" "$t")
    [ "$s" = completed ] && return 0
    [ "$s" = none ] && [ "$(gh run view "$run" -R "$repo" --json status -q .status)" = completed ] && return 0
    sleep 30
  done
}

# internal: wait for one build job (run under a machine's lock above)
if [ "$mode" = _wait ]; then wait_job "$1" "$2"; exit 0; fi

run=
if [ "$mode" = dispatch ]; then
  ref=main; args=()
  while [ $# -gt 0 ]; do case "$1" in --ref) ref=$2; shift 2;; -f) args+=(-f "$2"); shift 2;; *) echo "unknown $1" >&2; exit 2;; esac; done
  before=$(gh run list -R "$repo" -w release.yml -e workflow_dispatch -L 1 --json databaseId -q '.[0].databaseId' || true)
  gh workflow run release.yml -R "$repo" --ref "$ref" "${args[@]}"
  for _ in $(seq 30); do
    run=$(gh run list -R "$repo" -w release.yml -e workflow_dispatch -L 1 --json databaseId -q '.[0].databaseId' || true)
    [ -n "$run" ] && [ "$run" != "$before" ] && break; sleep 4
  done
  [ -n "$run" ] && [ "$run" != "$before" ] || { echo "release-dispatch: the dispatched run did not appear" >&2; exit 1; }
elif [ "$mode" = watch ]; then run=${1:?watch RUN_ID}
else echo "usage: release-dispatch.sh dispatch|watch" >&2; exit 2; fi
echo "release-dispatch: run $run (https://github.com/$repo/actions/runs/$run); windows: ${win:-github}, mac: ${mac:-github}"

pids=()
if [ "$win" = bp-winbuild-release ]; then
  "$here/bp-lock.sh" winbuild-vm -- bash -c '"$1/winbuild-up.sh" >/dev/null && echo "release-dispatch: bp-winbuild up" && "$2" _wait "$3" windows-x64' _ "$here" "$0" "$run" &
  pids+=($!)
fi
if [ "$mac" = bp-mac ]; then
  "$here/bp-lock.sh" mac-heavy -- bash -c '
    ssh -o BatchMode=yes hs-mac-mini "cd ~/bp-runner && df -h ~ | tail -1 && perl -MPOSIX -e \"setsid(); exec @ARGV\" ./run.sh >> runner.log 2>&1 < /dev/null & echo \$! > runner.pid" && echo "release-dispatch: mini runner up"
    "$1" _wait "$2" darwin-arm64; r=$?
    ssh -o BatchMode=yes hs-mac-mini "cd ~/bp-runner && p=\$(cat runner.pid 2>/dev/null) && [ -n \"\$p\" ] && kill -TERM -- -\$p; rm -f runner.pid"; echo "release-dispatch: mini runner stopped"; exit $r' _ "$0" "$run" &
  pids+=($!)
fi
if [ "$lin" = bp-linux-release ] && [ -n "$box" ]; then
  "$here/bp-lock.sh" linux-release -- bash -c '
    boat --no-update resume "$1" >/dev/null 2>&1 || true
    for _ in $(seq 60); do boat --no-update ssh "$1" -- "docker start bp-linux-release-runner >/dev/null 2>&1; docker ps -q -f name=bp-linux-release-runner" 2>/dev/null | grep -q . && break; sleep 10; done
    echo "release-dispatch: linux runner box $1 up"
    "$2" _wait "$3" linux-x64; r=$?
    boat --no-update stop "$1" >/dev/null 2>&1 && echo "release-dispatch: linux runner box $1 stopped"; exit $r' _ "$box" "$0" "$run" &
  pids+=($!)
fi
rc=0; for p in "${pids[@]}"; do wait "$p" || rc=1; done
[ ${#pids[@]} -gt 0 ] || gh run watch "$run" -R "$repo" --exit-status >/dev/null || true
wait_job "$run" linux-x64
gh run view "$run" -R "$repo" --json jobs -q '.jobs[] | select(.name | startswith("build")) | [.name, .conclusion, .startedAt, .completedAt] | @tsv' |
  while IFS=$'\t' read -r n c s e; do
    [ -n "$e" ] && [ "$e" != 0001-01-01T00:00:00Z ] || { echo "$n: $c (not finished)"; continue; }
    awk -v n="$n" -v c="$c" -v d=$(( $(date -d "$e" +%s) - $(date -d "$s" +%s) )) 'BEGIN { printf "%-22s %-9s %5.1f min\n", n, c, d / 60 }'
  done
exit $rc
