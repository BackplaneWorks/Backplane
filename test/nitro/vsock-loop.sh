#!/bin/bash
# The parent <-> guest protocol of scripts/nitro (bp-vsock.py <-> bp-guest.py) through a stand-in for Firecracker's vsock socket: ping, a run's
# stdout/stderr/exit code, the folder never leaving src/, rsync in and out through `pipe`, boat-prune by the list, and a missing socket = 97.
# No VM and no AWS. Run: test/nitro/vsock-loop.sh
set -u; cd "$(dirname "$0")/../.." || exit 1; R=$PWD
. scripts/lib-bptmp.sh 2>/dev/null; T=$(mktemp -d "${BP_TMP:-${TMPDIR:-/tmp}}/bp-nitro-loop.XXXXXX"); FC=
trap '[ -n "$FC" ] && kill "$FC" 2>/dev/null; rm -rf "$T"' EXIT
fail=0; ok() { echo "ok   $1"; }; no() { echo "FAIL $1"; fail=1; }
mkdir -p "$T/work"; export BP_GUEST_WORK=$T/work
python3 -I test/nitro/fake-firecracker.py "$T/v.sock" "$R/scripts/nitro/bp-guest.py" & FC=$!
for _ in $(seq 50); do [ -S "$T/v.sock" ] && break; sleep 0.1; done
V() { python3 -I "$R/scripts/nitro/bp-vsock.py" "$T/v.sock" 5000 "$@"; }
b() { if [ -z "$1" ]; then echo -; else printf '%s' "$1" | base64 -w0; fi; }
av() { local IFS=$'\x01'; printf '%s' "$*" | tr '\001' '\000' | base64 -w0; }

V ping && ok ping || no ping
out=$(V run - "$(av sh -c 'echo out; echo err >&2; exit 7')" - 2> "$T/err"); rc=$?
[ "$out" = out ] && [ "$(cat "$T/err")" = err ] && [ "$rc" = 7 ] && ok "run: stdout, stderr apart, exit 7" || no "run: '$out' '$(cat "$T/err")' $rc"
mkdir -p "$T/work/src/a/b"
[ "$(V run "$(b a/b)" "$(av pwd)" -)" = "$(realpath "$T/work/src/a/b")" ] && ok "run in src/a/b" || no "cwd"
[ "$(V run "$(b ../../..)" "$(av pwd)" -)" = "$(realpath "$T/work/src")" ] && ok "a folder outside src/ is src/" || no "cwd escape"
[ "$(V run - "$(av sh -c 'printf %s "$FOO"')" "$(b 'FOO=bar baz')")" = "bar baz" ] && ok "env" || no env
# rsync in and out through pipe (the -e wrapper drops the host name, as scripts/nitro-ssh.sh does on the hub's side)
printf '#!/bin/sh\nshift\nexec python3 -I %s %s 5000 pipe "$@"\n' "$R/scripts/nitro/bp-vsock.py" "$T/v.sock" > "$T/e.sh"; chmod +x "$T/e.sh"
mkdir -p "$T/tree/d"; echo one > "$T/tree/one"; echo two > "$T/tree/d/two"; echo stale > "$T/work/src/stale"
printf 'one\0d/two\0' > "$T/list"
rsync -rlp --checksum --no-times --from0 --files-from="$T/list" -e "$T/e.sh" "$T/tree/" box:./ 2> "$T/rs" && [ "$(cat "$T/work/src/d/two")" = two ] && ok "rsync in" || no "rsync in: $(cat "$T/rs")"
rsync -q -e "$T/e.sh" "$T/list" box:../.bp/list 2> "$T/rs" && cmp -s "$T/list" "$T/work/.bp/list" && ok "the list in .bp" || no "list: $(cat "$T/rs")"
V run - "$(av python3 -I "$R/scripts/boat-prune.py" "$T/work/src" "$T/work/.bp/list")" - > /dev/null; [ ! -e "$T/work/src/stale" ] && [ -e "$T/work/src/one" ] && ok "prune by the list" || no prune
mkdir -p "$T/back"; printf 'd/two\0' > "$T/art"
rsync -rlp --no-times --from0 --files-from="$T/art" -e "$T/e.sh" box:./ "$T/back/" 2> "$T/rs" && [ "$(cat "$T/back/d/two")" = two ] && ok "rsync out" || no "rsync out: $(cat "$T/rs")"
python3 -I scripts/nitro/bp-vsock.py "$T/none.sock" 5000 ping 2> /dev/null; [ $? = 97 ] && ok "no socket: 97 (the transport)" || no "no socket"
exit $fail
