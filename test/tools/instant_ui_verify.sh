#!/bin/sh
# Run serially on Boat; preserve receipts even when main's proof fails.
set -eu
mkdir -p build/instant-ui/verify
bend guide > build/instant-ui/verify/bend-guide.txt
node --test desktop/test/thin-host.test.cjs desktop/test/thin-send-preview.test.cjs desktop/test/thin-view-hold.test.cjs desktop/test/local-polish.test.cjs desktop/test/theme-retained.test.cjs desktop/test/recent-views.test.cjs desktop/test/view-prefetch.test.cjs > build/instant-ui/verify/node.log 2>&1
if [ "${1:-}" = --web-only ]; then
  printf '%s\n' 'deferred: prior scripts/check.sh admission 75; full proof must run with 18 GiB available' > build/instant-ui/verify/check.log
  printf '%s\n' deferred > build/instant-ui/verify/check.rc
else
  rc=0
  scripts/check.sh > build/instant-ui/verify/check.log 2>&1 || rc=$?
  printf '%s\n' "$rc" > build/instant-ui/verify/check.rc
fi
bend src/web/index.html -o build/instant-ui/web > build/instant-ui/verify/web.log 2>&1
python3 scripts/js-unmarshal.py ../core/client.Ui build/instant-ui/web/chunk-*.js >> build/instant-ui/verify/web.log
python3 scripts/js-native.py build/instant-ui/web/chunk-*.js >> build/instant-ui/verify/web.log
cp src/web/sw.js build/instant-ui/web/sw.js
python3 - <<'PY'
import pathlib,hashlib,json
root=pathlib.Path('build/instant-ui/web')
(pathlib.Path('build/instant-ui/verify')/'web-manifest.json').write_text(json.dumps({str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in root.rglob('*') if p.is_file()},indent=2)+'\n')
PY
tar -czf build/instant-ui/verify/web.tar.gz -C build/instant-ui web
cat build/instant-ui/verify/check.rc
tail -8 build/instant-ui/verify/check.log
tail -8 build/instant-ui/verify/node.log
