#!/bin/sh
# Boat only, fixtures only: the hub binary's `--render MODEL` flag draws a GLB's views as PNGs (out/render/).
exec scripts/boat-job.sh --size default --timeout 1500 --idle 3600 --artifact out/render \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'stage/**' --exclude 'stage-edge/**' --exclude 'test/gpui/hub/policy-bin/**' -- sh -c '
mkdir -p out/render; (cd native/host && cargo build --release --bin backplane-hub 2>&1 | tail -2) > out/render/build.txt
G=$(ls test/gpui/files/fixtures/mesh/*.glb | head -1); mkdir -p out/render/work; cp "$G" out/render/work/part.glb
(cd out/render/work && BACKPLANE_HOME=$PWD/h ../../../native/host/target/release/backplane-hub --render part.glb; echo "exit $?"; ls -la . renders 2>/dev/null) 2>&1 | tee out/render/run.txt | tail -15
BACKPLANE_HOME=$PWD/out/render/h native/host/target/release/backplane-hub --bogus > out/render/bogus.txt 2>&1; echo "bogus flag exit $?" >> out/render/run.txt; tail -1 out/render/run.txt'
