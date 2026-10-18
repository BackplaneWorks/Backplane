#!/bin/sh
# scripts/verified.sh with a stand-in bend (no real gate run): only a full
# check.sh or an unsharded full test.sh records; check-only, proof-only, a
# shard or a failure record nothing; an edit, a new untracked file, a changed
# gate script or bend make the record not match.
#   sh test/tools/verified_test.sh
set -eu
here=$(cd "$(dirname "$0")/../.." && pwd)
tmp=$(mktemp -d "${TMPDIR:-/tmp}/bp-verified-test.XXXXXX")
trap 'rm -rf "$tmp"' 0 1 2 15
git clone -q --shared "$here" "$tmp/repo"
for f in check.sh test.sh verified.sh inputs-key.sh bendok.sh check-manifest.py; do cp "$here/scripts/$f" "$tmp/repo/scripts/"; done
mkdir -p "$tmp/bin" "$tmp/bend2"
echo 'base' >"$tmp/bend2/base.bend"
cat >"$tmp/bin/bend" <<'B'
#!/bin/sh
# stand-in: every check, proof and test passes, unless BEND_FAKE=fail;
# BEND_FAKE=edit changes a source file while it runs
[ "${BEND_FAKE:-}" = fail ] && { echo "Error: no"; exit 1; }
[ "${BEND_FAKE:-}" = edit ] && echo "# during" >>src/core/json.bend
case " $* " in *" --check-only "*) echo "All terms check."; exit 0 ;; esac
case $1 in PROOF.bend) echo "All terms check." ;; *) echo "ok fake" ;; esac
B
chmod +x "$tmp/bin/bend"
export PATH="$tmp/bin:$PATH" BACKPLANE_VERIFIED="$tmp/v" CHECK_JOBS=1 TEST_JOBS=2
cd "$tmp/repo"
fail=0
check() { if [ "$2" = "$3" ]; then echo "ok   $1"; else echo "FAIL $1: got $2 want $3"; fail=1; fi; }
has() { scripts/verified.sh has "$1" && echo yes || echo no; }
q() { "$@" >/dev/null 2>&1 || true; }
q scripts/check.sh check; check "check-only records nothing" "$(has check)" no
q scripts/check.sh proof; check "proof-only records nothing" "$(has check)" no
BEND_FAKE=fail q scripts/check.sh; check "a failed check records nothing" "$(has check)" no
q scripts/check.sh; check "a full check records" "$(has check)" yes
check "a check is no test" "$(has test)" no
TEST_SHARD_COUNT=2 TEST_SHARD=1 q scripts/test.sh; check "a shard records nothing" "$(has test)" no
BEND_FAKE=fail q scripts/test.sh; check "failed tests record nothing" "$(has test)" no
q scripts/test.sh; check "every test passing records" "$(has test)" yes
echo "# edit" >>src/core/json.bend; check "an edit: no match" "$(has check)" no
git checkout -q src/core/json.bend; check "back as it was: matches again" "$(has check)" yes
echo 'import Base' >src/core/zz_new.bend; check "a new untracked file: no match" "$(has check)" no
rm src/core/zz_new.bend
echo "# x" >>scripts/bendok.sh; check "a changed gate script: no match" "$(has check)" no
git checkout -q scripts/bendok.sh 2>/dev/null || cp "$here/scripts/bendok.sh" scripts/bendok.sh
printf '\n# changed\n' >>"$tmp/bin/bend"; check "a changed bend: no match" "$(has check)" no
BACKPLANE_VERIFIED=0 q scripts/check.sh; check "records off: never matches" "$(BACKPLANE_VERIFIED=0 scripts/verified.sh has check && echo yes || echo no)" no
git checkout -q src/core/json.bend; rm -rf "$BACKPLANE_VERIFIED"
BEND_FAKE=edit q scripts/check.sh; check "a file changed during the check: not recorded" "$(git checkout -q src/core/json.bend; has check)" no
BEND_FAKE=edit q scripts/test.sh; check "a file changed during the tests: not recorded" "$(git checkout -q src/core/json.bend; has test)" no
check "mark with no key taken before records nothing" "$(scripts/verified.sh mark check '' 2>/dev/null || true; has check)" no
check "mark with another state's key records nothing" "$(scripts/verified.sh mark check 0123 2>/dev/null || true; has check)" no
rm src/core/bom.bend; q scripts/check.sh; check "a tracked file missing: no key, nothing recorded" "$(git checkout -q src/core/bom.bend; has check)" no
# exit codes (check.sh once ended in a false `[ ] &&` and exited 1 on a pass)
rc() { r=0; "$@" >/dev/null 2>&1 || r=$?; echo $r; }
nz() { if [ "$1" != 0 ]; then echo nonzero; else echo 0; fi; }
check "check.sh check: exit 0 on a pass" "$(rc scripts/check.sh check)" 0
check "check.sh proof: exit 0 on a pass" "$(rc scripts/check.sh proof)" 0
check "check.sh (all): exit 0 on a pass" "$(rc scripts/check.sh)" 0
check "check.sh check: non-zero on a failure" "$(nz "$(rc env BEND_FAKE=fail scripts/check.sh check)")" nonzero
check "check.sh proof: non-zero on a failure" "$(nz "$(rc env BEND_FAKE=fail scripts/check.sh proof)")" nonzero
check "check.sh (all): non-zero on a failure" "$(nz "$(rc env BEND_FAKE=fail scripts/check.sh)")" nonzero
check "no half-written record left" "$(ls "$BACKPLANE_VERIFIED" | grep -c '\.[0-9]*$' || true)" 0
exit $fail
