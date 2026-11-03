#!/bin/sh
# Lane 4 benchmark, for a Boat sandbox only (scripts/boat-job.sh -- sh scripts/lane4-bench.sh).
# Refuses to run anything heavy unless the sandbox's own cgroup caps memory at
# 3,700,000,000 bytes or less with swap 0 (read here, not assumed). Per stage: wall time,
# peak RSS of the stage, the cgroup's memory.max/swap.max/peak and OOM events.
set -u
cd "$(dirname "$0")/.."
# The sandbox has no cap of its own: put the whole run (controller and worker
# together) in a transient cgroup scope of 3.6 GB (3,600,000,000 B) with swap 0, then read the
# limits back below and refuse unless they are really enforced.
if [ -z "${BP_L4_SCOPED:-}" ] && command -v systemd-run >/dev/null 2>&1; then
  rc=0
  BP_L4_SCOPED=1 systemd-run --user --scope -q -p MemoryMax=3600000000 -p MemorySwapMax=0 -- sh "$0" "$@" || rc=$?
  # the scope's own status is the run's status: never retried uncapped
  exit "$rc"
fi
if [ -z "${BP_L4_SCOPED:-}" ]; then
  echo "lane4-bench: no systemd scope to cap memory in; running nothing heavy" >&2
  exit 78
fi
CAP=3700000000
g=$(sed -n 's/^0:://p' /proc/self/cgroup); d=/sys/fs/cgroup$g
rd() { cat "$d/$1" 2>/dev/null || echo unavailable; }
echo "identity: git=$(git rev-parse HEAD 2>/dev/null || echo none) bend=$(command -v bend) sha=$(sha256sum "$(readlink -f "$(command -v bend)")" | cut -c1-64)"
echo "host: MemTotal_kB=$(awk '/MemTotal/{print $2}' /proc/meminfo) SwapTotal_kB=$(awk '/SwapTotal/{print $2}' /proc/meminfo) cgroup=$g"
echo "caps: memory.max=$(rd memory.max) memory.swap.max=$(rd memory.swap.max) parents: $(cat "$d/../memory.max" 2>/dev/null || echo -)"
mm=$(rd memory.max); sw=$(rd memory.swap.max); st=$(awk '/SwapTotal/{print $2}' /proc/meminfo)
ok=0
case $mm in *[!0-9]*|"") ;; *) [ "$mm" -le "$CAP" ] && ok=1 ;; esac
if [ "$ok" = 1 ]; then [ "$sw" = 0 ] || ok=0; fi
if [ "$ok" != 1 ]
then
  echo "lane4-bench: no enforced cap of 3,700,000,000 B with swap 0 here (memory.max=$mm swap.max=$sw SwapTotal_kB=$st); running nothing heavy" >&2
  exit 78
fi
echo "cap verified from inside: memory.max=$mm (<= $CAP) memory.swap.max=$sw"
export BP_CONTENT_CACHE=${TMPDIR:-/tmp}/cc-lane4
rm -rf "$BP_CONTENT_CACHE"
failed=0
stage() {
  name=$1; shift
  oom0=$(grep -E '^oom_kill ' "$d/memory.events" 2>/dev/null | cut -d' ' -f2); s=$(date +%s)
  rc=0; /usr/bin/time -f "%M" -o /tmp/lane4.rss "$@" >/tmp/lane4.out 2>&1 || rc=$?
  e=$(date +%s); oom1=$(grep -E '^oom_kill ' "$d/memory.events" 2>/dev/null | cut -d' ' -f2)
  tail -4 /tmp/lane4.out | sed "s/^/  [$name] /"
  [ "$rc" = 0 ] || failed=1
  echo "STAGE $name rc=$rc wall=$((e - s))s peak_rss_kb=$(tail -1 /tmp/lane4.rss) cgroup_peak=$(rd memory.peak) oom_kill_before=${oom0:-na} after=${oom1:-na} swap_max=$(rd memory.swap.max) mem_max=$(rd memory.max)"
}
stage baseline scripts/check.sh check
stage cold scripts/cached-check.sh
stage warm scripts/cached-check.sh
# the edit stage changes a source file, so only after every stage before it
# passed (a refused admission mutates nothing); the file's exact bytes,
# including any edit it already had, are put back from a copy, never by git
if [ "$failed" = 0 ]; then
  cp -p src/core/fm.bend /tmp/lane4.fm.orig || { echo "lane4-bench: cannot save src/core/fm.bend" >&2; exit 1; }
  printf '\n' >> src/core/fm.bend
  stage edit-leaf scripts/cached-check.sh
  cp -p /tmp/lane4.fm.orig src/core/fm.bend
else
  echo "lane4-bench: skipped edit-leaf (an earlier stage did not pass); no source touched" >&2
fi
# a stage that did not pass (a refused admission included) is not a benchmark
[ "$failed" = 0 ] || { echo "lane4-bench: not every stage passed; this is no benchmark result" >&2; exit 1; }
