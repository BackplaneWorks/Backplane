#!/bin/sh
# Builds bp-plot (the Rust plot reader as a command) and runs the kicad tests of the files harness on Boat, never on the desktop.
# Output: out/files-plot/{tests.txt,build.txt,bp-plot}
exec scripts/boat-job.sh --size default --timeout 1800 --idle 3600 --artifact out/files-plot \
  --exclude 'mobile/**' --exclude 'desktop/**' --exclude 'build/**' --exclude 'docs/**' --exclude 'tools/**' \
  --exclude 'shots-snap/**' --exclude 'assets/**' --exclude 'i18n/**' --exclude 'deploy/**' --exclude 'order/**' \
  --exclude 'skills/**' --exclude 'extra/**' --exclude 'test/gpui/hub/policy-bin/**' -- sh -c 'mkdir -p out/files-plot; cd test/gpui/files/harness; (cargo build --release --bin bp-plot 2>&1 | tail -80) > ../../../../out/files-plot/build.txt 2>&1; cp target/release/bp-plot ../../../../out/files-plot/ 2>/dev/null; (cargo test kicad 2>&1 | tail -80) > ../../../../out/files-plot/tests.txt 2>&1; cd ../../../..; tail -50 out/files-plot/build.txt; tail -50 out/files-plot/tests.txt; ls -l out/files-plot'
