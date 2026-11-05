#!/bin/sh
# Builds backplane-step2glb on Boat (Bun, never on the desktop; Boat's Bun cannot read bun.lock's version, so the sandbox installs from package.json's pin) and returns it in out/step2glb/, as the oracle for STEP parity: the JS hub and the Rust hub meshing with the same tool.
exec scripts/boat-job.sh --size default --timeout 900 --idle 3600 --artifact out/step2glb \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/gpui/hub/policy-bin/**' -- sh -c 'mkdir -p out/step2glb; mkdir -p dist; (cd tools/step2glb && rm -f bun.lock && bun install 2>&1 && grep -m1 version node_modules/occt-import-js/package.json; cd ../.. && bun build tools/step2glb/step2glb.ts --compile --minify --outfile dist/backplane-step2glb 2>&1) > out/step2glb/build.log 2>&1; c=$?; tail -30 out/step2glb/build.log > out/step2glb/build.txt; [ $c -eq 0 ] && { cp dist/backplane-step2glb out/step2glb/ || c=1; }; tail -20 out/step2glb/build.txt; ls -l out/step2glb; exit $c'
