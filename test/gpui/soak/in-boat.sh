#!/bin/sh
# in-boat.sh MODE [args] : runs INSIDE a Boat sandbox from the repo root. Refuses on the desktop.
#   proof SECS      : selftest, then one soak hop of SECS (default 3600) with 20 stand-in agents, analysis
#   crash SCENARIO ROUNDS [EVENTS]
case "$HOME" in /home/h|/home/h/*) echo "refuses to run on the desktop" >&2; exit 3;; esac
[ "$(id -un)" = user ] || { echo "not a Boat sandbox; refusing" >&2; exit 3; }
R=$PWD; O=$R/out/soak; mkdir -p $O
(cd native/host && cargo build --release 2>&1 | tail -1) || exit 1
HUB=$R/native/host/target/release/backplane-hub; POL=$R/test/gpui/soak/pack/bp-policy-page; chmod +x $POL
SZ=$(stat -c %s $HUB); echo "hub $SZ bytes, $(nproc) vCPU, $(free -m | awk '/Mem/{print $2}') MB, $(uname -r)" | tee $O/machine.txt
mode=$1; shift
[ "$2" = nopol ] && NOPOL=1
case $mode in
 proof)
  bun test/gpui/soak/selftest.mjs | tee $O/selftest.txt || exit 1
  H=$HOME/soak-home; rm -rf $H
  bun test/gpui/soak/run.mjs bin=$HUB ${NOPOL:+} $([ -n "$NOPOL" ] || echo policy=$POL) home=$H out=$O/proof${NOPOL:+-nopolicy} fresh=1 secs=${1:-3600} sample=30 port=3830 stop=kill commit=$(git rev-parse --short HEAD 2>/dev/null)
  rc=$?
  python3 test/gpui/soak/analyze.py $O/proof${NOPOL:+-nopolicy}/samples.jsonl ${WARM:-900} --json $O/proof${NOPOL:+-nopolicy}/analysis.json | tee $O/proof${NOPOL:+-nopolicy}/analysis.txt
  exit $rc ;;
 hop)
  # hop N SECS : one link of the hub-only chain. Carry (home + chain/) arrives as test/gpui/soak/carry/carry.tar.gz (hop > 1).
  N=$1; SECS=$2; C=$HOME/chain; H=$HOME/soak-home; rm -rf $C $H; mkdir -p $C
  if [ "$N" -gt 1 ]; then tar xzf test/gpui/soak/carry/carry.tar.gz -C $HOME || exit 6; mv $HOME/carry/home $H; mv $HOME/carry/chain $C; rm -rf $HOME/carry; fi
  OFF=$(( (N - 1) * SECS ))
  bun test/gpui/soak/run.mjs bin=$HUB home=$H out=$C hop=$N offset=$OFF secs=$SECS sample=30 port=3830 stop=kill fresh=$([ "$N" -eq 1 ] && echo 1 || echo 0) commit=$(git rev-parse --short HEAD 2>/dev/null)
  rc=$?
  bun test/gpui/soak/chain-verify.mjs $H $C $N; vrc=$?
  mkdir -p $O/chain; cp $C/*.json $C/*.jsonl $C/*.log $O/chain/ 2>/dev/null; rm -f $O/chain/ledger-hop*.jsonl
  python3 test/gpui/soak/analyze.py $C/samples.jsonl 600 --json $O/chain/analysis.json > $O/chain/analysis.txt 2>&1
  mkdir -p $HOME/carry && mv $H $HOME/carry/home && mv $C $HOME/carry/chain && tar czf $O/carry.tar.gz -C $HOME carry
  ls -l $O/carry.tar.gz
  [ $rc -eq 0 ] && [ $vrc -eq 0 ] ;;
 crash)
  scen=$1; rounds=${2:-10}; ev=${3:-1000000}
  bun test/gpui/soak/crash.mjs bin=$HUB policy=$POL out=$O/crash scenario=$scen rounds=$rounds events=$ev port=3900 work=$HOME/crash-$scen ;;
esac
