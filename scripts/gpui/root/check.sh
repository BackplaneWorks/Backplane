#!/bin/sh
# Run only through bp-build.sh: focused root tests, sidebar parity, then the runtime binary.
export PATH="$HOME/.bun/bin:$PATH"
if [ "${ROOT_CHECK_GATE:-}" = 1 ]; then sh "$(dirname "$0")/gate-check.sh" || exit "$?"; fi
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
cargo build --release --bin backplane-hub --bin sidebar-check > /tmp/bp-root-build.log 2>&1
rc=$?
grep -A10 '^error' /tmp/bp-root-build.log
tail -3 /tmp/bp-root-build.log
[ "$rc" = 0 ] || exit "$rc"
cd ../..
python3 - <<'PYJSON'
import json
j=json.load(open('native/host/src/present/fixtures/session-gpui-recent.json'))
with open('/tmp/bp-root-sidebar-events.jsonl','w') as f:
    for e in j['frames'][4]['json']['items']: f.write(json.dumps(e)+'\n')
PYJSON
bun test/gpui/parity/sidebar/compare_projects.mjs build/gpui/root/bp-policy-pager "$CARGO_TARGET_DIR/release/sidebar-check" /tmp/bp-root-sidebar-events.jsonl 3897 > /tmp/bp-root-sidebar-projects.log 2>&1
rc=$?
cat /tmp/bp-root-sidebar-projects.log
[ "$rc" = 0 ] || exit "$rc"
python3 - <<'PYJSON'
import json
rows=[json.loads(l) for l in open('/tmp/bp-root-sidebar-projects.log') if l.startswith('{')]
assert rows and rows[0]['differences']==0, rows
PYJSON
[ "$?" = 0 ] || exit 1
bun test/gpui/parity/sidebar/compare_status.mjs build/gpui/root/bp-policy-pager "$CARGO_TARGET_DIR/release/sidebar-check" /tmp/bp-root-sidebar-events.jsonl 3897 > /tmp/bp-root-sidebar-status.log 2>&1
rc=$?
cat /tmp/bp-root-sidebar-status.log
[ "$rc" = 0 ] || exit "$rc"
python3 - <<'PYJSON'
import json
rows=[json.loads(l) for l in open('/tmp/bp-root-sidebar-status.log') if l.startswith('{')]
assert rows and rows[0]['different']==0 and rows[0]['onlyInBend']==0, rows
PYJSON
