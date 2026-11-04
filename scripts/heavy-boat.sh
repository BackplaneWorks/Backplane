#!/bin/sh
# Called only in a fresh Boat sandbox. The command still performs local guard checks there.
#
# Fail closed: the command starts only inside a cgroup whose OWN memory.max, read from the kernel, is finite and
# <= 3,700,000,000 B with memory.swap.max 0. Already inside such a cgroup: run directly (no environment variable is
# trusted). Otherwise start it in a systemd scope with that cap and re-verify the cgroup from INSIDE the scope, so a
# missing or substitute systemd-run (one that just runs the command) cannot yield an uncapped run. Nothing verifiable:
# exit 75 before the command or any tool setup runs.
set -eu
CAP=3700000000
# prints "ok" iff this process's own cgroup is capped as required
CHECK='g=$(cut -d: -f3 /proc/self/cgroup 2>/dev/null) || exit 1; b=/sys/fs/cgroup$g
mx=$(cat "$b/memory.max" 2>/dev/null) || exit 1; sw=$(cat "$b/memory.swap.max" 2>/dev/null) || exit 1
case "$mx" in ""|*[!0-9]*) exit 1;; esac
[ "$mx" -gt 0 ] && [ "$mx" -le '"$CAP"' ] && [ "$sw" = 0 ]'
refuse() { echo "heavy-boat: $1; refusing to start the command without a kernel-verified cap (memory.max <= $CAP, swap 0)" >&2; exit 75; }
if ! sh -c "$CHECK"; then
  command -v systemd-run >/dev/null 2>&1 || refuse "no systemd-run and this cgroup is not capped"
  # the child verifies its own cgroup before exec'ing the command; a fake systemd-run that only runs it fails here
  exec systemd-run --user --scope -q -p MemoryAccounting=yes -p MemoryMax=$CAP -p MemorySwapMax=0 -- \
    sh -c "$CHECK || { echo 'heavy-boat: the scope is not capped as required; refusing' >&2; exit 75; }; exec sh \"\$0\" \"\$@\"" "$0" "$@"
fi
python3 -c "import runpy; runpy.run_path('scripts/local-phase.py')['tools']()"
export PATH="$HOME/.local/backplane-ci/bin:$PATH" BACKPLANE_HEAVY_OFFLOADED=1
export BACKPLANE_TELEMETRY=off BACKPLANE_TELEMETRY_BAKE=0 BACKPLANE_GOOGLE_BAKE=0 BACKPLANE_KISTACK_SYNC=0 BACKPLANE_NO_UPDATE=1
exec "$@"
