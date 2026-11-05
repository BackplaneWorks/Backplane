#!/bin/sh
# chain.sh HOPS SECS_PER_HOP [START_HOP] : the hub-only soak as a chain of Boat jobs (a job lives at most 2 h). Run from the repo root,
# in the background (work_run). Each hop: upload carry.tar.gz (hub home + chain/ with samples and ledgers), run SECS, SIGKILL the hub,
# verify the whole log against every acked write, tar the home forward. Final artifacts: out/chain/<n>/ and build results dir.
HOPS=${1:-15}; SECS=${2:-5760}; N=${3:-1}
RES=${RES:-/home/h/Documents/i2cjak/backplane-gpui/build/gpui/results/soak/m7-hubonly-24h}
mkdir -p $RES test/gpui/soak/carry
while [ $N -le $HOPS ]; do
  tries=0
  while :; do
    BOAT_T=$(( SECS + 800 )) sh test/gpui/soak/boat-job.sh hop $N $SECS > $RES/hop-$N.out 2>&1; rc=$?
    J=$(grep -o 'logs /[^ ]*' $RES/hop-$N.out | tail -1 | cut -d' ' -f2)
    if [ $rc -eq 75 ] && [ $tries -lt 20 ]; then tries=$((tries+1)); echo "hop $N refused, retry $tries"; sleep 300; continue; fi
    break
  done
  echo "hop $N exit $rc job $J"
  [ -n "$J" ] && [ -f $J/artifacts/soak/carry.tar.gz ] || { echo "hop $N: no carry artifact; stopping the chain"; exit 1; }
  mv $J/artifacts/soak/carry.tar.gz test/gpui/soak/carry/carry.tar.gz
  rm -rf $RES/chain && cp -r $J/artifacts/soak/chain $RES/chain && cp $J/artifacts/soak/machine.txt $RES/ 2>/dev/null
  cp $J/job.log $RES/hop-$N.job.log
  [ $rc -ne 0 ] && echo "hop $N: nonzero exit $rc (kept going; see $RES/hop-$N.out)"
  N=$((N+1))
done
echo "chain done"; rm -f test/gpui/soak/carry/carry.tar.gz; cat $RES/chain/analysis.txt | head -60
