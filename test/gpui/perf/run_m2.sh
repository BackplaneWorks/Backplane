#!/bin/bash
# M2 network driver. Usage: run_m2.sh   (desktop; drives the Mac clients over ssh)
R=/home/h/Documents/i2cjak/backplane-gpui; cd $R/test/gpui/perf
export RSYNC_RSH=$R/build/gpui/mac-ssh; MAC="$R/build/gpui/mac-ssh hs-mac-mini"
OUT=$R/build/gpui/results/perf/m2; export OUT; mkdir -p $OUT
TS=100.64.84.41
srv(){ # name host port agents
  (bun m2_server.mjs $2 $3 $4 $1 > $OUT/$1.srv 2>&1 &)
  for i in $(seq 90); do [ -f $R/build/gpui/perf-run/m2-$1/state.json ] && break; sleep 1; done
  ST=$R/build/gpui/perf-run/m2-$1/state.json
}
stopsrv(){ touch $R/build/gpui/perf-run/m2-$1/stop; sleep 4; }
mac(){ # label scenario [arg] ; env passed through DELTA/SECS
  rsync -aq $ST hs-mac-mini:bp-gpui-perf/state/$NAME.json
  $MAC "cd ~/bp-gpui-perf && DELTA=${DELTA:-1} SECS=${SECS:-30} OUT=out ~/.bun/bin/bun test/gpui/perf/m2_client.mjs state/$NAME.json $TS $2 $1 $3" > $OUT/$1.stdout 2>&1
  rsync -aq hs-mac-mini:bp-gpui-perf/out/$1.json $OUT/
}
loc(){ bun m2_client.mjs $ST ${HOSTC:-127.0.0.1} $2 $1 $3 > $OUT/$1.stdout 2>&1; }
# ---- loopback, 20 agents
NAME=loop-load20; srv $NAME 127.0.0.1 3805 20
loc loopback-load20-rtt rtt
for K in 1 5 20; do for D in 0 1; do DELTA=$D loc loopback-load20-viewers$K-delta$D viewers $K; done; done
SECS=60 loc loopback-load20-slow slow
loc loopback-load20-drop drop 3000
stopsrv $NAME
# ---- tailnet idle, Mac client
NAME=ts-idle; srv $NAME $TS 3806 0
mac tailnet-idle-rtt rtt
stopsrv $NAME
# ---- tailnet load20, Mac client + local viewers through the ts address
NAME=ts-load20; srv $NAME $TS 3807 20
mac tailnet-load20-rtt rtt
for K in 1 5 20; do for D in 0 1; do DELTA=$D mac tailnet-load20-viewers$K-delta$D viewers $K; done; done
stopsrv $NAME
echo ALLDONE
