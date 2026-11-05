cd native/host
cargo test --release --bin backplane-hub -- present:: > /tmp/bp-root-tests.log 2>&1
rc=$?
grep -A10 '^error' /tmp/bp-root-tests.log
tail -8 /tmp/bp-root-tests.log
[ "$rc" = 0 ] || exit "$rc"
cargo test --release --bin backplane-hub -- sidebar:: > /tmp/bp-root-sidebar.log 2>&1
rc=$?
grep -A10 '^error' /tmp/bp-root-sidebar.log
tail -8 /tmp/bp-root-sidebar.log
[ "$rc" = 0 ] || exit "$rc"
cargo build --release --bin backplane-hub > /tmp/bp-root-build.log 2>&1
rc=$?
grep -A10 '^error' /tmp/bp-root-build.log
tail -3 /tmp/bp-root-build.log
exit "$rc"
