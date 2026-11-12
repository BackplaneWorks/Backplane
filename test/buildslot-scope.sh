#!/bin/bash
# scripts/lib-buildslot.sh: a build holding a slot runs in a scope of its own in bpbuild.slice (systemd-oomd's candidate, not the hub's scope that started it),
# keeps its exit code, does not carry the slot's lock, and -j is this slot's share of free memory. Skips where there is no user systemd.
#   test/buildslot-scope.sh        exit 0 when it holds
set -eu
HERE=$(cd "$(dirname "$0")/.." && pwd)
. "$HERE/scripts/lib-bptmp.sh"
T=$(mktemp -d "${BP_TMP:-${TMPDIR:-/tmp}}/bp-bqscope-XXXX"); trap 'rm -rf "$T"' EXIT
fail() { echo "FAIL: $*"; exit 1; }
. "$HERE/scripts/lib-buildslot.sh"
export BUILD_LOCK_DIR=$T/locks BUILD_SLOTS=1 BP_NO_BOAT=1
if [ -z "$(BQ_NO_SCOPE= bash -c ". '$HERE/scripts/lib-buildslot.sh'; bq_scope; echo \${BQ_SCOPE[*]-}")" ]; then echo "skip: no user systemd here"; exit 0; fi
rc=0; bq_run '' sh -c 'cat /proc/self/cgroup > "$1"; ls -l /proc/self/fd > "$1.fd"; exit 7' sh "$T/cg" || rc=$?
[ "$rc" = 7 ] || fail "exit code $rc, not 7"
grep -q '/bpbuild.slice/run-.*\.scope$' "$T/cg" || fail "not in a scope of bpbuild.slice: $(cat "$T/cg")"
! grep -q "$T/locks/slot" "$T/cg.fd" || fail "the build holds the slot's lock: $(cat "$T/cg.fd")"
rc=0; BQ_NO_SCOPE=1 bq_run '' sh -c 'cat /proc/self/cgroup > "$1"' sh "$T/cg2" || rc=$?
[ "$rc" = 0 ] && ! grep -q bpbuild.slice "$T/cg2" || fail "BQ_NO_SCOPE=1 still made a scope"
# -j: three slots and 30 GB free give 5 jobs each (2 GB a job from a third of it), never all 15
J=$(BUILD_SLOTS=3 bash -c ". '$HERE/scripts/lib-buildslot.sh'; bq_mem_gb() { echo 30; }; nproc() { echo 32; }; bq_jobs")
[ "$J" = 5 ] || fail "bq_jobs gave $J with 3 slots and 30 GB, not 5"
echo "ok: a build runs in its own scope in bpbuild.slice, keeps its exit code, holds no slot lock, and -j is its slot's share of free memory"
