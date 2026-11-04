#!/bin/sh
set -eu
cd extra/recut951-intake
mkdir -p ../../build/intake-controls
systemd-run --user --scope -q -p MemoryAccounting=yes -p MemoryMax=536870912 -p MemorySwapMax=0 -- /usr/bin/time -v -o ../../build/intake-controls/resources.txt python3 test_seed_intake.py > ../../build/intake-controls/tests.log 2>&1
cat ../../build/intake-controls/tests.log
