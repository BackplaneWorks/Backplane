#!/bin/bash
R=$(git -C "$(dirname "$0")" rev-parse --show-toplevel); cd $R/test/perf; OUT=$R/build/gpui/results/perf/wire; export OUT
port=3803
for T in 22 300; do NAME=wireops-t$T; port=$((port+1))
  (THREADS=$T bun m2_server.mjs 127.0.0.1 $port 0 $NAME > $OUT/$NAME.srv 2>&1 &)
  rm -f $R/build/gpui/perf-run/m2-$NAME/state.json
  for i in $(seq 120); do [ -f $R/build/gpui/perf-run/m2-$NAME/state.json ] && break; sleep 1; done
  ST=$R/build/gpui/perf-run/m2-$NAME/state.json
  [ $T = 22 ] && bun m2b_wire.mjs $ST t22-a0-push-delta0 70 push 0 > $OUT/t22-a0-push-delta0.out 2>&1
  [ $T = 22 ] && bun m2b_wire.mjs $ST t22-a0-push-delta1 70 push 1 > $OUT/t22-a0-push-delta1.out 2>&1
  [ $T = 22 ] && bun m2b_wire.mjs $ST t22-a0-poll-delta0 70 poll 0 > $OUT/t22-a0-poll-delta0.out 2>&1
  [ $T = 22 ] && bun m2b_wire.mjs $ST t22-a0-poll-delta1 70 poll 1 > $OUT/t22-a0-poll-delta1.out 2>&1
  bun wire_ops.mjs $ST idle-t$T 200 > $OUT/idle-t$T-ops.out 2>&1
  touch $R/build/gpui/perf-run/m2-$NAME/stop; sleep 5
done; echo W2DONE
