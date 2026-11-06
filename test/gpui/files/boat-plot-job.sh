#!/bin/sh
# Builds bp-plot (the Rust plot reader as a command) and runs the kicad tests of the files harness on Boat, never on the desktop.
# Output: out/files-plot/{tests.txt,build.txt,bp-plot}
exec scripts/boat-job.sh --size default --timeout 1800 --idle 3600 --artifact out/files-plot \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/gpui/hub/policy-bin/**' -- sh -c 'mkdir -p out/files-plot; cd test/gpui/files/harness; cargo build --release --bin bp-plot > ../../../../out/files-plot/build.log 2>&1; b=$?; tail -80 ../../../../out/files-plot/build.log > ../../../../out/files-plot/build.txt; [ $b -eq 0 ] && { cp target/release/bp-plot ../../../../out/files-plot/ || b=1; }; cargo test kicad > ../../../../out/files-plot/tests.log 2>&1; t=$?; tail -80 ../../../../out/files-plot/tests.log > ../../../../out/files-plot/tests.txt; cd ../../../..; tail -50 out/files-plot/build.txt; tail -50 out/files-plot/tests.txt; ls -l out/files-plot; exit $(( b | t ))'
