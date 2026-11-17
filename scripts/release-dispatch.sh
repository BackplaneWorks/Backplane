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
var() { gh variable get "$1" -R "$repo" 2>/dev/null || true; }
# the variables, or the same names in the environment (a branch's test run, where the workflow takes the labels as inputs)
win=${RELEASE_WIN_RUNNER-$(var RELEASE_WIN_RUNNER)}; mac=${RELEASE_MAC_RUNNER-$(var RELEASE_MAC_RUNNER)}

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
    ssh -o BatchMode=yes hs-mac-mini "cd ~/bp-runner && df -h ~ | tail -1 && nohup ./run.sh >> runner.log 2>&1 < /dev/null & echo \$! > runner.pid" && echo "release-dispatch: mini runner up"
    "$1" _wait "$2" darwin-arm64; r=$?
    ssh -o BatchMode=yes hs-mac-mini "cd ~/bp-runner && kill \$(cat runner.pid) 2>/dev/null; rm -f runner.pid"; echo "release-dispatch: mini runner stopped"; exit $r' _ "$0" "$run" &
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
