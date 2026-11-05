#!/bin/sh
export BEND_NO_TELEMETRY=1
O=out/efx; mkdir -p $O
bend native/bend/efx.bend -o $O/efx.c > $O/emit.log 2>&1; echo "emit rc=$?" | tee $O/rc.txt
head -30 $O/emit.log
if [ -s $O/efx.c ]; then grep -n "db_open\|Db.open\|Db\$open" $O/efx.c | head; clang -std=c11 -O2 $O/efx.c -lpthread -lm -o $O/efx > $O/clang.log 2>&1; echo "clang rc=$?"; head -20 $O/clang.log; [ -x $O/efx ] && ($O/efx; echo "run rc=$?") 2>&1 | head -5; fi
