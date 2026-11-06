#!/bin/sh
# in-boat.sh MODE [args] : runs INSIDE a Boat sandbox from the repo root. Refuses on the desktop.
#   proof SECS [nopol]      : selftest, one soak hop of SECS with 20 stand-in agents (policy child unless nopol), ledger check, analysis
#   hop N SECS              : one link of the hub-only chain (carry arrives as test/gpui/soak/carry/carry.tar.gz when N > 1)
#   crash SCENARIO ROUNDS [EVENTS]
case "$HOME" in /home/h|/home/h/*) echo "refuses to run on the desktop" >&2; exit 3;; esac
[ "$(id -un)" = user ] || { echo "not a Boat sandbox; refusing" >&2; exit 3; }
R=$PWD; O=$R/out/soak; mkdir -p $O
cargo_log=$O/cargo.log
(cd native/host && cargo build --release > $cargo_log 2>&1); crc=$?; tail -2 $cargo_log
[ $crc -eq 0 ] || { echo "cargo build failed ($crc)"; exit 1; }
HUB=$R/native/host/target/release/backplane-hub; POL=$R/test/gpui/soak/pack/bp-policy-page
[ -x $HUB ] || { echo "no hub binary"; exit 1; }
[ -f $POL ] && chmod +x $POL
echo "hub $(stat -c %s $HUB) bytes, $(nproc) vCPU, $(free -m | awk '/Mem/{print $2}') MB, $(uname -r)" | tee $O/machine.txt
mode=$1; shift
selftest() { bun test/gpui/soak/selftest.mjs > $O/selftest.txt 2>&1; src=$?; tail -3 $O/selftest.txt; [ $src -eq 0 ] || { echo "selftest failed"; exit 1; }; }
case $mode in
 proof)
  selftest
  NP=; [ "$2" = nopol ] && NP=1
  H=$HOME/soak-home; rm -rf $H; D=$O/proof${NP:+-nopolicy}; mkdir -p $D
  POLARG=; [ -z "$NP" ] && POLARG=policy=$POL
  bun test/gpui/soak/run.mjs bin=$HUB $POLARG home=$H out=$D fresh=1 secs=${1:-3600} sample=30 port=3830 stop=kill commit=$(git rev-parse --short HEAD 2>/dev/null)
  rc=$?
  bun test/gpui/soak/chain-verify.mjs $H $D 1; vrc=$?
  python3 test/gpui/soak/analyze.py $D/samples.jsonl ${WARM:-900} --json $D/analysis.json > $D/analysis.txt 2>&1; arc=$?; cat $D/analysis.txt
  [ $rc -eq 0 ] && [ $vrc -eq 0 ] && [ $arc -eq 0 ] ;;
 hop)
  selftest
  N=$1; SECS=$2; C=$HOME/chain; H=$HOME/soak-home; rm -rf $C $H $HOME/carry
  if [ "$N" -gt 1 ]; then
    tar xzf test/gpui/soak/carry/carry.tar.gz -C $HOME || { echo "carry extract failed"; exit 6; }
    [ -d $HOME/carry/home ] && [ -d $HOME/carry/chain ] || { echo "carry has no home/chain"; exit 6; }
    mv $HOME/carry/home $H; mv $HOME/carry/chain $C; rm -rf $HOME/carry
    P=$((N - 1)); h=1
    while [ $h -le $P ]; do [ -f $C/ledger-hop$h.jsonl ] && [ -f $C/hop-$h.json ] || { echo "carry lacks hop $h ledger or summary"; exit 6; }; h=$((h+1)); done
  else mkdir -p $C; fi
  # offset = the measured run time of the earlier hops (hop-<n>.json secs), not the nominal length
  OFF=$(python3 -c "import json,glob;print(sum(int(json.load(open(f))['secs']) for f in glob.glob('$C/hop-*.json')))")
  bun test/gpui/soak/run.mjs bin=$HUB home=$H out=$C hop=$N offset=$OFF secs=$SECS sample=30 port=3830 stop=kill fresh=$([ "$N" -eq 1 ] && echo 1 || echo 0) commit=$(git rev-parse --short HEAD 2>/dev/null)
  rc=$?
  bun test/gpui/soak/chain-verify.mjs $H $C $N; vrc=$?
  python3 test/gpui/soak/analyze.py $C/samples.jsonl 600 --json $C/analysis.json > $C/analysis.txt 2>&1
  mkdir -p $O/chain && cp -r $C/. $O/chain/        # everything small, ledgers included, comes back beside the carry
  mkdir -p $HOME/carry && mv $H $HOME/carry/home && mv $C $HOME/carry/chain && tar czf $O/carry.tar.gz -C $HOME carry || { echo "carry tar failed"; exit 6; }
  ls -l $O/carry.tar.gz
  [ $rc -eq 0 ] && [ $vrc -eq 0 ] ;;
 crash)
  scen=$1; rounds=${2:-10}; ev=${3:-1000000}
  bun test/gpui/soak/crash.mjs bin=$HUB policy=$POL out=$O/crash scenario=$scen rounds=$rounds events=$ev port=3900 work=$HOME/crash-$scen ;;
esac
