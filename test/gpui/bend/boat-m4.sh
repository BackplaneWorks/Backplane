#!/bin/sh
# inside Boat: build native Bend policy + Rust host, run them beside the real JS hub and a bun/TS baseline
set -u
export BEND_NO_TELEMETRY=1
O=out/m4; W=$O/work; mkdir -p $O $W
M=test/gpui/bend/m4
echo "machine: $(nproc) vCPU, $(free -m | awk '/Mem/{print $2}') MiB, $(uname -m), $(rustc --version), bun $(bun --version)" | tee $O/machine.txt
bend native/bend/policy.bend -o $O/policy.c || { echo "bend policy FAILED"; exit 1; }
clang -std=c11 -O3 $O/policy.c -lpthread -lm -o $O/policy || exit 1
ls -l $O/policy.c $O/policy | tee -a $O/sizes.txt
( cd native/host && /usr/bin/time -f "cargo build --release: %e s, peak %M KiB" cargo build --release 2>&1 | tail -8 ) | tee $O/cargo.txt
HOST=native/host/target/release/bphost
[ -x $HOST ] || { echo "cargo FAILED"; exit 1; }
ls -l $HOST | tee -a $O/sizes.txt
bun $M/fixture.ts $W/store.db
# the real JS hub
H=$W/hubhome; mkdir -p $H $W/agents
BACKPLANE_AGENTS=$PWD/$W/agents BACKPLANE_NO_UPDATE=1 BACKPLANE_TELEMETRY=off BACKPLANE_ORDER_SERVER=http://127.0.0.1:9 \
  ./test/gpui/bend/m4/jshub --home $PWD/$H --port 3810 --host 127.0.0.1 --no-tailscale --foreground > $O/jshub.log 2>&1 &
HUB=$!
sleep 6
curl -s -i http://127.0.0.1:3810/hello > $O/hello.jshub.http
VER=$(./test/gpui/bend/m4/jshub --version | awk '{print $2}')
COMMIT=$(python3 -c "
d=open('$O/hello.jshub.http','rb').read().split(b'\r\n\r\n',1)[1]
i=d.index(b'commit')+6
n=d[i]-0x60 if d[i]<0x78 else d[i+1]
s=i+(1 if d[i]<0x78 else 2)
print(d[s:s+n].decode())")
echo "hub version=$VER commit=$COMMIT"
$HOST --port 3811 --db $W/store.db --policy $PWD/$O/policy --version "$VER" --commit "$COMMIT" --busy 0 --tmp $W > $O/host.log 2>&1 &
HOSTPID=$!
bun $M/baseline.ts $W/store.db 3812 > $O/baseline.log 2>&1 &
BASE=$!
sleep 2
echo "== /hello bytes"
curl -s -i http://127.0.0.1:3811/hello > $O/hello.host.http
cmp $O/hello.jshub.http $O/hello.host.http && echo "HELLO IDENTICAL ($(wc -c < $O/hello.host.http) bytes, headers + body)" || { echo "HELLO DIFFERS"; od -c $O/hello.jshub.http | head -12; od -c $O/hello.host.http | head -12; }
echo "== /history bodies"
for t in t0 t1 t2 t3; do
  bun $M/oracle.ts $W/store.db $t > $W/oracle.$t.json
  curl -s "http://127.0.0.1:3811/history?thread=$t" > $W/host.$t.json
  echo >> $W/host.$t.json
  cmp $W/oracle.$t.json $W/host.$t.json && echo "history $t IDENTICAL ($(wc -c < $W/host.$t.json) bytes)" || { echo "history $t DIFFERS"; head -c 300 $W/oracle.$t.json; echo; head -c 300 $W/host.$t.json; }
done
echo "== websocket"
bun $M/ws.ts 3811 t0 t1 t2 t3 > $W/ws.out
cat $W/oracle.t0.json $W/oracle.t1.json $W/oracle.t2.json $W/oracle.t3.json | cmp - $W/ws.out && echo "WS replies IDENTICAL to oracle" || echo "WS DIFFERS"
echo "== bench"
for spec in "3810/hello 1" "3811/hello 1" "3811/history?thread=t1 1" "3812/history?thread=t1 1" "3811/history?thread=t1 8" "3812/history?thread=t1 8" "3810/hello 8" "3811/hello 8"; do
  set -- $spec
  bun $M/bench.ts "http://127.0.0.1:$1" 1000 $2 | tee -a $O/bench.jsonl
done
curl -s http://127.0.0.1:3811/stats | tee $O/stats.json; echo
grep -E "VmHWM|VmRSS" /proc/$HUB/status | tee $O/rss.jshub.txt
grep -E "VmHWM|VmRSS" /proc/$BASE/status | tee $O/rss.baseline.txt
grep -E "VmHWM|VmRSS" /proc/$HOSTPID/status | tee $O/rss.host.txt
kill $HUB $HOSTPID $BASE 2>/dev/null
cp $W/oracle.t1.json $O/; cp $W/host.t1.json $O/host.t1.json
