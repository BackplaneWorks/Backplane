#!/bin/bash
# Boat-only end-to-end check of the compiler guard with the REAL pinned bend (and bun when its directory is writable).
# Refuses outside a Boat sandbox. Run it INSIDE a scope with MemoryMax=3700000000 MemorySwapMax=0 (the Boat rule needs a
# kernel-read cap). Installs the shims into the sandbox's own HOME and proves, on the real executables, that an
# absolute-path call is refused outside an approved context and allowed inside one, then uninstalls byte-exactly.
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
show() { echo "---- $1"; sed -n 1,6p "$1"; }
CAPPED=(systemd-run --user --scope -q -p MemoryMax=3700000000 -p MemorySwapMax=0)
UNCAPPED=(systemd-run --user --scope -q -p MemoryMax=infinity -p MemorySwapMax=infinity)
NOCTX=(env -u BACKPLANE_HEAVY_BOAT_SANDBOX -u BACKPLANE_HEAVY_OFFLOADED -u BACKPLANE_HEAVY_LOCK_FD)
read -r own_max own_swap < <(g=$(cut -d: -f3 /proc/self/cgroup); echo "$(cat /sys/fs/cgroup$g/memory.max) $(cat /sys/fs/cgroup$g/memory.swap.max)")
say "own cgroup: memory.max=$own_max swap.max=$own_swap (the Boat rule needs 3699998720-or-lower / 0)"
printf 'import Base\n\ndef one() -> Nat:\n  1n\n' > /tmp/guard-fixture.bend
cat > /tmp/g4.py <<'PY'
import fcntl, os, subprocess, sys
mode, exe = sys.argv[1], sys.argv[2]
fd = os.open('/tmp/bp-bend.lock', os.O_CREAT | os.O_RDWR, 0o666)
if mode == 'locked':
    fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)      # genuinely held
os.set_inheritable(fd, True)                            # mode 'opened': descriptor exists, flock NOT taken
env = {k: v for k, v in os.environ.items() if k not in ('BACKPLANE_HEAVY_BOAT_SANDBOX', 'BACKPLANE_HEAVY_OFFLOADED')}
env['BACKPLANE_HEAVY_LOCK_FD'] = str(fd)
print(subprocess.run([exe, '/tmp/guard-fixture.bend', '--check-only'], env=env, pass_fds=(fd,), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode)
PY
before=$(sha256sum "$BEND" | cut -d' ' -f1); say "real bend sha256 $before at $BEND"
bunbefore=""; bunpath=""
if [ -n "$BUN" ] && [ -w "$(dirname "$BUN")" ]; then bunpath=$(readlink -f "$BUN"); bunbefore=$(sha256sum "$bunpath" | cut -d' ' -f1); say "bun $bunpath is writable: included"; else say "bun ${BUN:-absent} not writable here: bun cases SKIPPED (a root-owned bun is a documented bypass)"; fi
tools=(--tool bend); [ -n "$bunpath" ] && tools+=(--tool bun --path "bun=$bunpath")
# install safety on the REAL bend: an unexpected .real is never overwritten; any failure restores the original exactly
BINDIR=$(dirname "$BEND")
intact() { [ "$(sha256sum "$BEND" | cut -d' ' -f1)" = "$before" ] && [ ! -e "$BEND.real" ] && [ -z "$(ls -A "$BINDIR" | grep -E '^\.guard-' || true)" ] && ! grep -q "backplane-compiler-guard shim" "$BEND"; }
echo "precious unrelated file" > "$BEND.real"
python3 $G install --tool bend --apply >/tmp/gs1.out 2>&1; ok $? 1 "install refuses an unexpected existing bend.real"
[ "$(cat "$BEND.real")" = "precious unrelated file" ] && [ "$(sha256sum "$BEND" | cut -d' ' -f1)" = "$before" ] && say "ok   the unrelated file and bend are untouched" || { say "FAIL existing .real was disturbed"; show /tmp/gs1.out; fail=1; }
python3 $G install --tool bend >/tmp/gs2.out 2>&1; grep -q REFUSED /tmp/gs2.out && say "ok   dry run reports REFUSED" || { say "FAIL dry run did not report"; fail=1; }
rm -f "$BEND.real"
for point in after-journal after-move after-shim before-manifest; do
  COMPILER_GUARD_TEST_FAULT=$point python3 $G install --tool bend --apply >/tmp/gs3.out 2>&1; ok $? 1 "install fails at injected fault $point"
  intact && say "ok   original bend restored exactly after $point (bytes, no .real, no shim, no temp files)" || { say "FAIL bend not restored after $point"; ls -la "$BINDIR"; fail=1; }
  python3 $G check --tool bend >/dev/null 2>&1; ok $? 1 "check still reports bend unguarded after $point"
done
python3 $G install "${tools[@]}" >/dev/null; ok $? 0 "dry run exits 0"
[ "$(sha256sum "$BEND" | cut -d' ' -f1)" = "$before" ] && say "ok   dry run left bend untouched" || { say "FAIL dry run changed bend"; fail=1; }
python3 $G install "${tools[@]}" --apply; ok $? 0 "install --apply"
python3 $G check "${tools[@]}"; ok $? 0 "check after install"
ls -la "$HOME/.bend/bin"
# 1. absolute path, no approved context at all
"${NOCTX[@]}" "${UNCAPPED[@]}" "$BEND" /tmp/guard-fixture.bend --check-only >/tmp/g1.out 2>&1; ok $? 75 "absolute-path bend refused with no context"
grep -q "not in an approved capped context" /tmp/g1.out && say "ok   refusal message" || { say "FAIL refusal message"; show /tmp/g1.out; fail=1; }
# 2. Boat environment + kernel-read cap (this script itself runs in the capped scope)
"$BEND" /tmp/guard-fixture.bend --check-only >/tmp/g2.out 2>&1; ok $? 0 "absolute-path bend allowed on Boat inside the cap"
grep -q "ALL PROOFS CHECK" /tmp/g2.out && say "ok   real bend verdict through the shim" || { say "FAIL no verdict"; show /tmp/g2.out; fail=1; }
# 3. Boat environment but an UNCAPPED scope: refused
"${UNCAPPED[@]}" "$BEND" /tmp/guard-fixture.bend --check-only >/tmp/g3.out 2>&1; ok $? 75 "Boat environment without a cap is refused"
# 4. light subcommand with no context
"${NOCTX[@]}" "${UNCAPPED[@]}" "$BEND" version >/tmp/g4v.out 2>&1; ok $? 0 "bend version with no context"
# 5. capped + genuinely held native flock, Boat environment removed
out=$("${CAPPED[@]}" python3 /tmp/g4.py locked "$BEND" 2>&1 | tail -1); ok "$out" 0 "capped scope + native flock allows bend"
# 6. capped, no flock
out=$("${CAPPED[@]}" "${NOCTX[@]}" "$BEND" /tmp/guard-fixture.bend --check-only >/dev/null 2>&1; echo $?); ok "$out" 75 "capped scope WITHOUT the native flock is refused"
# 7. descriptor opened on the lock file but the flock NOT taken
out=$("${CAPPED[@]}" python3 /tmp/g4.py opened "$BEND" 2>&1 | tail -1); ok "$out" 75 "opened-but-unlocked descriptor is refused"
# 8. flock held but no cap
out=$("${UNCAPPED[@]}" python3 /tmp/g4.py locked "$BEND" 2>&1 | tail -1); ok "$out" 75 "native flock WITHOUT a cap is refused"
if [ -n "$bunpath" ]; then
  "${NOCTX[@]}" "${UNCAPPED[@]}" "$bunpath" build /tmp/none.ts >/dev/null 2>&1; ok $? 75 "bun build refused with no context"
  "${NOCTX[@]}" "${UNCAPPED[@]}" "$bunpath" --version >/dev/null 2>&1; ok $? 0 "bun --version allowed"
fi
python3 $G uninstall "${tools[@]}" --apply; ok $? 0 "uninstall"
[ "$(sha256sum "$BEND" | cut -d' ' -f1)" = "$before" ] && say "ok   bend bytes restored" || { say "FAIL bend bytes differ"; fail=1; }
[ -n "$bunpath" ] && { [ "$(sha256sum "$bunpath" | cut -d' ' -f1)" = "$bunbefore" ] && say "ok   bun bytes restored" || { say "FAIL bun bytes differ"; fail=1; }; }
say "result: $([ $fail = 0 ] && echo PASS || echo FAIL)"
exit $fail
