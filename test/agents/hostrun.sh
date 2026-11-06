#!/bin/sh
# One job on h@backplane (through scripts/bp-build.sh agents 'sh test/agents/hostrun.sh'): the whole crate, all targets, three times, with the scope's memory peak and OOM kills read at the end.
# Provisional, never a gate. The peak is the cgroup's memory.peak (the 12G cap of bp-build's systemd scope); oom_kill in memory.events says whether the kernel killed anything in it.
cd native/host || exit 1
cg=/sys/fs/cgroup$(sed 's/^0:://' /proc/self/cgroup 2>/dev/null)
echo "cgroup: $cg"; [ -r "$cg/memory.max" ] && echo "memory.max: $(cat $cg/memory.max)"
for i in 1 2 3; do
  cargo test --all-targets > ../../.hostrun-$i.log 2>&1; rc=$?
  echo "run $i rc=$rc"; grep -E "^test result" ../../.hostrun-$i.log | tail -3
  grep -E "^test .*FAILED|agy_permission_matches_the_js_hub \.\.\." ../../.hostrun-$i.log | cut -c1-160
done
for f in memory.peak memory.events; do if [ -r "$cg/$f" ]; then echo "$f: $(cat $cg/$f | tr '\n' ' ')"; else echo "$f: not readable ($cg)"; fi; done
dmesg 2>/dev/null | grep -i "out of memory\|killed process" | tail -3 || true
