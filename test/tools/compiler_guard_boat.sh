#!/bin/bash
# Boat-only end-to-end check of the compiler guard with the REAL pinned bend (and bun when present).
# Refuses outside a Boat sandbox. Installs the shims into the sandbox's own HOME, proves an absolute-path call
# is refused outside an approved context and allowed inside one, then uninstalls and compares bytes.
set -u
[ "${BACKPLANE_HEAVY_BOAT_SANDBOX:-}" = 1 ] && [ "${BACKPLANE_HEAVY_OFFLOADED:-}" = 1 ] || { echo "refusing: not a Boat sandbox" >&2; exit 75; }
export BEND_NO_TELEMETRY=1
cd "$(dirname "$0")/../.." || exit 70
G=scripts/compiler-guard.py
BEND="$HOME/.bend/bin/bend"
BUN=$(command -v bun || true)
fail=0
say() { echo "[guard-boat] $*"; }
ok() { if [ "$1" = "$2" ]; then say "ok   $3 (rc=$1)"; else say "FAIL $3 (rc=$1, wanted $2)"; fail=1; fi; }
printf 'import Base\n\ndef one() -> Nat:\n  1n\n' > /tmp/guard-fixture.bend
before=$(sha256sum "$BEND" | cut -d' ' -f1); say "real bend sha256 $before"
bunbefore=""; [ -n "$BUN" ] && bunbefore=$(sha256sum "$BUN" | cut -d' ' -f1)
tools="--tool bend"; [ -n "$BUN" ] && tools="--tool bend --tool bun"
python3 $G install $tools >/dev/null; ok $? 0 "dry run exits 0"
[ "$(sha256sum "$BEND" | cut -d' ' -f1)" = "$before" ] && say "ok   dry run left bend untouched" || { say "FAIL dry run changed bend"; fail=1; }
python3 $G install $tools --apply; ok $? 0 "install --apply"
python3 $G check $tools; ok $? 0 "check after install"
# 1. absolute path, no approved context
env -u BACKPLANE_HEAVY_BOAT_SANDBOX -u BACKPLANE_HEAVY_OFFLOADED -u BACKPLANE_HEAVY_LOCK_FD "$BEND" /tmp/guard-fixture.bend --check-only >/tmp/g1.out 2>&1; ok $? 75 "absolute-path bend refused outside a context"
grep -q "not in an approved capped context" /tmp/g1.out && say "ok   refusal message" || { say "FAIL refusal message"; fail=1; }
# 2. the same call inside the Boat sandbox (parallel, no local lock)
"$BEND" /tmp/guard-fixture.bend --check-only >/tmp/g2.out 2>&1; ok $? 0 "absolute-path bend allowed in the Boat sandbox"
grep -q "ALL PROOFS CHECK" /tmp/g2.out && say "ok   real bend verdict through the shim" || { say "FAIL no verdict"; fail=1; }
# 3. light subcommand outside a context
env -u BACKPLANE_HEAVY_BOAT_SANDBOX -u BACKPLANE_HEAVY_OFFLOADED "$BEND" version >/tmp/g3.out 2>&1; ok $? 0 "bend version outside a context"
# 4. capped local context: inherited native flock + own cgroup <= 3.7e9 / swap 0 (Boat env removed)
cat > /tmp/g4.py <<'PY'
import fcntl, os, subprocess, sys
fd = os.open('/tmp/bp-bend.lock', os.O_CREAT | os.O_RDWR, 0o666)
fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
os.set_inheritable(fd, True)
env = {k: v for k, v in os.environ.items() if k not in ('BACKPLANE_HEAVY_BOAT_SANDBOX', 'BACKPLANE_HEAVY_OFFLOADED')}
env['BACKPLANE_HEAVY_LOCK_FD'] = str(fd)
print(subprocess.run([sys.argv[1], '/tmp/guard-fixture.bend', '--check-only'], env=env, pass_fds=(fd,)).returncode)
PY
out=$(systemd-run --user --scope -q -p MemoryMax=3700000000 -p MemorySwapMax=0 python3 /tmp/g4.py "$BEND" 2>&1 | tail -1); ok "$out" 0 "capped scope + native flock allows bend"
out=$(systemd-run --user --scope -q -p MemoryMax=3700000000 -p MemorySwapMax=0 env -u BACKPLANE_HEAVY_BOAT_SANDBOX -u BACKPLANE_HEAVY_OFFLOADED "$BEND" /tmp/guard-fixture.bend --check-only >/dev/null 2>&1; echo $?); ok "$out" 75 "capped scope WITHOUT the native flock is refused"
out=$(env -u BACKPLANE_HEAVY_BOAT_SANDBOX -u BACKPLANE_HEAVY_OFFLOADED python3 /tmp/g4.py "$BEND" 2>&1 | tail -1); ok "$out" 75 "native flock WITHOUT a cap is refused"
if [ -n "$BUN" ]; then
  env -u BACKPLANE_HEAVY_BOAT_SANDBOX -u BACKPLANE_HEAVY_OFFLOADED "$BUN" build /tmp/none.ts >/dev/null 2>&1; ok $? 75 "bun build refused outside a context"
  env -u BACKPLANE_HEAVY_BOAT_SANDBOX -u BACKPLANE_HEAVY_OFFLOADED "$BUN" --version >/dev/null 2>&1; ok $? 0 "bun --version allowed"
fi
python3 $G uninstall $tools --apply; ok $? 0 "uninstall"
[ "$(sha256sum "$BEND" | cut -d' ' -f1)" = "$before" ] && say "ok   bend bytes restored" || { say "FAIL bend bytes differ"; fail=1; }
[ -n "$BUN" ] && { [ "$(sha256sum "$BUN" | cut -d' ' -f1)" = "$bunbefore" ] && say "ok   bun bytes restored" || { say "FAIL bun bytes differ"; fail=1; }; }
say "result: $([ $fail = 0 ] && echo PASS || echo FAIL)"
exit $fail
