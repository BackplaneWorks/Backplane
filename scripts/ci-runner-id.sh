#!/bin/sh
# What of the machine a gate's verdict can depend on, printed as text (no
# secrets, no paths of this machine): the runner part of scripts/ci-keys.py's
# keys. A runner prints it before its gates run and local CI records a pass
# only under the text the runner printed, so a pass on one toolchain is never
# reused for another. Run it with the environment the gates get.
#   scripts/ci-runner-id.sh        the text; exits 1 when bend is missing
set -u
cd "$(dirname "$0")/.." || exit 1
v() { "$@" 2>/dev/null | head -n 1 || true; }
treehash() { if [ -d "$1" ]; then ( cd "$1" && find . -type f -print0 | LC_ALL=C sort -z | xargs -0 -r sha256sum | sha256sum | cut -c1-64 ); else echo none; fi; }
b=$(command -v bend) || exit 1
real=$(readlink -f "$b") || exit 1
bin=$(sha256sum "$real" | cut -c1-64) || exit 1
# the official builds of one release are one compiler (scripts/inputs-key.sh)
case $bin in
  b2b183bddc3c3fa4bc45d99f6a6bdbb192ebc616e6e697777f49e74c545b1643|\
  301a69a8e20937b37a431ecff5a80e85fdc5a2224ead972088e4ee9f3f89a300|\
  4d7280f366661d9b63b418eb70ca54ea1a867a23b196cc4b5f1f75e049cb80d1|\
  db96d899b65a1a1415291bfb9fb33d1ab43b0d076071d3489cc4a678670b2e00)
    bin="release 2.0.36" ;;
esac
echo "ci-runner-v1"
echo "bend $bin"
echo "bend2 $(treehash "$(dirname "$real")/../bend2")"
echo "bendlib $(treehash "${BEND_LIB:-$HOME/.bend/lib}")"
if command -v bun >/dev/null 2>&1; then echo "bun $(v bun --revision)"; else echo "bun none"; fi
for t in clang cc python3 node git sh; do
  if command -v $t >/dev/null 2>&1; then echo "$t $(v $t --version)"; else echo "$t none"; fi
done
echo "os $(uname -sm)"
echo "tz ${TZ-unset} $(date +%Z%z) $(readlink /etc/localtime 2>/dev/null | sed 's|.*zoneinfo/||')"
echo "stack soft $(ulimit -s) hard $(ulimit -Hs)"
env | LC_ALL=C sort | grep -E '^(BUN_[A-Z_]*|BEND_[A-Z_]*|BENDTT|CC|NODE_OPTIONS|LANG|LC_[A-Z]*|TEST_MEM_GB|CI)=' || true
