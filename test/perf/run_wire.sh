#!/bin/bash
R=/home/h/Documents/i2cjak/backplane-gpui; cd $R/test/perf; OUT=$R/build/gpui/results/perf/wire; export OUT; mkdir -p $OUT
port=3802
for T in 22 300; do for A in 0 5; do
  NAME=wire-t$T-a$A; port=$((port+1))
  (THREADS=$T bun m2_server.mjs 127.0.0.1 $port $A $NAME > $OUT/$NAME.srv 2>&1 &)
  for i in $(seq 120); do [ -f $R/build/gpui/perf-run/m2-$NAME/state.json ] && break; sleep 1; done
  ST=$R/build/gpui/perf-run/m2-$NAME/state.json
  for D in 0 1; do for M in push poll; do bun m2b_wire.mjs $ST t$T-a$A-$M-delta$D 70 $M $D > $OUT/t$T-a$A-$M-delta$D.out 2>&1; done; done
  touch $R/build/gpui/perf-run/m2-$NAME/stop; sleep 5
done; done
echo WIREDONE
