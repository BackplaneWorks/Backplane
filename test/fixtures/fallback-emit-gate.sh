#!/bin/sh
# Native Settings arity regression. Optional arguments select smaller roots.
# Runs in a disposable Boat sandbox; no native dependencies or credentials.
set -u
. scripts/bendok.sh
export BACKPLANE_LOW_MEM=1 BUN_JSC_forceRAMSize=4294967296
mkdir -p build/fallback-emits
rc=0
[ "$#" != 0 ] || set -- settings
for root in "$@"; do
  src="test/native/fallback_emit_$root.bend"
  echo "INTEGRATION EMIT $src"
  if bend_check "$src"; then
    one=0
    bend "$src" -o "build/fallback-emits/$root.c" >"build/fallback-emits/$root.log" 2>&1 || one=$?
    cat "build/fallback-emits/$root.log"
    echo "EMIT RESULT $root: $one"
    [ "$one" = 0 ] || rc=1
  else
    rc=1
  fi
done
exit "$rc"
