#!/bin/sh
# on h@backplane (through scripts/bp-build.sh): the crate's tests, the failures and the totals only. Provisional, never gating.
cd native/host || exit 2
cargo test --all-targets > ../../.bp-test.log 2>&1
rc=$?
grep -E "^error(\[|:)" -A12 ../../.bp-test.log | head -80
grep -E "^test result|FAILED|^failures:|panicked at" -A6 ../../.bp-test.log | head -120
exit $rc
