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
case $mode in
 proof)
  bun test/gpui/soak/selftest.mjs | tee $O/selftest.txt || exit 1
  H=$HOME/soak-home; rm -rf $H
  bun test/gpui/soak/run.mjs bin=$HUB policy=$POL home=$H out=$O/proof fresh=1 secs=${1:-3600} sample=30 port=3830 stop=kill commit=$(git rev-parse --short HEAD 2>/dev/null)
  rc=$?
  python3 test/gpui/soak/analyze.py $O/proof/samples.jsonl ${WARM:-900} --json $O/proof/analysis.json | tee $O/proof/analysis.txt
  exit $rc ;;
 crash)
  scen=$1; rounds=${2:-10}; ev=${3:-1000000}
  bun test/gpui/soak/crash.mjs bin=$HUB policy=$POL out=$O/crash scenario=$scen rounds=$rounds events=$ev port=3900 work=$HOME/crash-$scen ;;
esac
