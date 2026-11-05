#!/bin/sh
# chain.sh HOPS SECS_PER_HOP [START_HOP] : the hub-only soak as a chain of Boat jobs (a job lives at most 2 h). Run from the repo root,
# in the background (work_run). Each hop: upload carry.tar.gz (hub home + chain/ with samples and ledgers), run SECS, SIGKILL the hub,
# verify the whole log against every acked write, tar the home forward. Stops at the first hop that fails, with the failure kept.
HOPS=${1:-16}; SECS=${2:-5400}; N=${3:-1}
RES=${RES:-/home/h/Documents/i2cjak/backplane-gpui/build/gpui/results/soak/m7-hubonly-24h}
CARRY=test/gpui/soak/carry/carry.tar.gz
mkdir -p $RES test/gpui/soak/carry; FAIL=0
while [ $N -le $HOPS ]; do
  tries=0
  while :; do
    BOAT_T=$(( SECS + 800 )) sh test/gpui/soak/boat-job.sh hop $N $SECS > $RES/hop-$N.out 2>&1; rc=$?
    J=$(grep -o 'logs /[^ ]*' $RES/hop-$N.out | tail -1 | cut -d' ' -f2)
    if [ $rc -eq 75 ] && [ $tries -lt 30 ]; then tries=$((tries+1)); echo "hop $N refused (try $tries), waiting"; sleep 300; continue; fi
    break
  done
  echo "hop $N exit $rc job $J"
  [ -n "$J" ] && [ -s $J/artifacts/soak/carry.tar.gz ] || { echo "hop $N: no carry artifact; stopping"; exit 1; }
  gzip -t $J/artifacts/soak/carry.tar.gz || { echo "hop $N: carry is not a valid gzip; stopping"; exit 1; }
  # keep the chain's small files (samples, ledgers, summaries, verify) for every hop; promote the carry only after the checks
  rm -rf $RES/chain.new && cp -r $J/artifacts/soak/chain $RES/chain.new && rm -rf $RES/chain && mv $RES/chain.new $RES/chain || { echo "hop $N: copy failed; stopping"; exit 1; }
  cp $J/artifacts/soak/machine.txt $RES/ 2>/dev/null; cp $J/job.log $RES/hop-$N.job.log
  cp $J/artifacts/soak/carry.tar.gz $CARRY.new && mv $CARRY.new $CARRY || { echo "hop $N: promote failed; stopping"; exit 1; }
  if [ $rc -ne 0 ]; then echo "hop $N FAILED (exit $rc): $RES/hop-$N.out"; FAIL=$N; break; fi
  N=$((N+1))
done
cp $CARRY $RES/final-carry.tar.gz 2>/dev/null   # the last home and everything needed to resume or inspect, kept outside the repo
rm -f $CARRY
echo "chain ended: last hop run $N, failed hop $FAIL"; head -70 $RES/chain/analysis.txt 2>/dev/null
[ $FAIL -eq 0 ]
