#!/bin/bash
# Boat-only controls for scripts/heavy-boat.sh: it must never start its command outside a kernel-verified cap
# (own memory.max finite and <= 3.7e9 B, swap 0), whatever systemd-run or BACKPLANE_HEAVY_SCOPED look like.
set -u
[ "${BACKPLANE_HEAVY_BOAT_SANDBOX:-}" = 1 ] && [ "${BACKPLANE_HEAVY_OFFLOADED:-}" = 1 ] || { echo "refusing: not a Boat sandbox" >&2; exit 75; }
cd "$(dirname "$0")/../.." || exit 70
fail=0
say() { echo "[heavy-boat-scope] $*"; }
ok() { if [ "$1" = "$2" ]; then say "ok   $3 ($1)"; else say "FAIL $3 (got '$1', wanted '$2')"; fail=1; fi; }
UNCAPPED=(systemd-run --user --scope -q -p MemoryMax=infinity -p MemorySwapMax=infinity)
PAYLOAD='g=$(cut -d: -f3 /proc/self/cgroup); echo "ran max=$(cat /sys/fs/cgroup$g/memory.max) swap=$(cat /sys/fs/cgroup$g/memory.swap.max)"; touch /tmp/hb-ran'
want='ran max=3699998720 swap=0'
work=$(mktemp -d); trap 'rm -rf "$work"' EXIT
mkdir -p "$work/fake" "$work/nosd"
cat > "$work/fake/systemd-run" <<'F'
#!/bin/sh
# a substitute that ignores the cap and just runs the command
while [ "$1" != "--" ]; do shift; done; shift; exec "$@"
F
chmod +x "$work/fake/systemd-run"
for t in sh cut cat python3 env touch rm true; do ln -s "$(command -v $t)" "$work/nosd/$t"; done
run() { rm -f /tmp/hb-ran; "$@" >"$work/out" 2>"$work/err"; echo $?; }
# A. a fake systemd-run in front, uncapped start: refused before the payload
rc=$("${UNCAPPED[@]}" env PATH="$work/fake:$PATH" sh scripts/heavy-boat.sh sh -c "$PAYLOAD" >"$work/out" 2>"$work/err"; echo $?)
ok "$rc" 75 "fake systemd-run that ignores the cap -> exit 75"
[ ! -e /tmp/hb-ran ] && say "ok   payload did not run (fake systemd-run)" || { say "FAIL payload ran uncapped"; fail=1; }
grep -q "not capped as required" "$work/err" && say "ok   refusal named the scope" || { say "FAIL message"; sed -n 1,3p "$work/err"; fail=1; }
# B. no systemd-run at all, uncapped start
rm -f /tmp/hb-ran
rc=$("${UNCAPPED[@]}" env -i PATH="$work/nosd" HOME="$HOME" sh scripts/heavy-boat.sh sh -c "$PAYLOAD" >"$work/out" 2>"$work/err"; echo $?)
ok "$rc" 75 "no systemd-run on an uncapped start -> exit 75"
[ ! -e /tmp/hb-ran ] && say "ok   payload did not run (no systemd-run)" || { say "FAIL payload ran"; fail=1; }
# C. spoofed BACKPLANE_HEAVY_SCOPED=1 from an uncapped start: ignored, payload still runs capped
rm -f /tmp/hb-ran
out=$("${UNCAPPED[@]}" env BACKPLANE_HEAVY_SCOPED=1 sh scripts/heavy-boat.sh sh -c "$PAYLOAD" 2>/dev/null | tail -1)
ok "$out" "$want" "spoofed BACKPLANE_HEAVY_SCOPED=1 does not skip the cap"
# D. already inside a correct scope: runs directly
out=$(systemd-run --user --scope -q -p MemoryMax=3700000000 -p MemorySwapMax=0 sh scripts/heavy-boat.sh sh -c "$PAYLOAD" 2>/dev/null | tail -1)
ok "$out" "$want" "already capped scope runs the payload"
# E. a scope with the right memory but swap allowed is not accepted as capped: the payload still ends up swap 0
out=$(systemd-run --user --scope -q -p MemoryMax=3700000000 -p MemorySwapMax=infinity sh scripts/heavy-boat.sh sh -c "$PAYLOAD" 2>/dev/null | tail -1)
ok "$out" "$want" "swap not zero is rewrapped into a swap-0 scope"
# F. a cap above 3.7e9 is not accepted
out=$(systemd-run --user --scope -q -p MemoryMax=3800000000 -p MemorySwapMax=0 sh scripts/heavy-boat.sh sh -c "$PAYLOAD" 2>/dev/null | tail -1)
ok "$out" "$want" "a 3.8e9 cap is rewrapped into a 3.7e9 scope"
say "result: $([ $fail = 0 ] && echo PASS || echo FAIL)"
exit $fail
