#!/bin/sh
# inside Boat: cargo test then release build of native/host (the hub crate)
set -u
# 'core' strips the other streams' modules from this sandbox copy only (the lead's gates must not wait for them)
if [ "${1:-}" = core ]; then
  (cd native/host && rm -rf src/agents src/files src/kicad && sed -i 's/mod agents; //; s/mod files; //; s/mod kicad; //; /agents::Agents::start/d; /files::start/d' src/main.rs)
fi
cd native/host
echo "machine: $(nproc) vCPU, $(free -m | awk '/Mem/{print $2}') MiB, $(rustc --version)"
/usr/bin/time -f "cargo test: %e s, peak %M KiB" cargo test --release --bin backplane-hub > /tmp/test.out 2>&1; grep -A14 "^error" /tmp/test.out | head -60; grep -E "test result|FAILED|panicked" /tmp/test.out | head -20
/usr/bin/time -f "cargo build --release: %e s, peak %M KiB" cargo build --release --bin backplane-hub > /tmp/build.out 2>&1; grep -A14 "^error" /tmp/build.out | head -90; tail -3 /tmp/build.out
ls -l target/release/backplane-hub
mkdir -p ../../out/cargo && cp target/release/backplane-hub ../../out/cargo/ 2>/dev/null
