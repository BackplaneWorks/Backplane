#!/bin/sh
# Focused synthetic policy/proof gate; no application build or provider call.
set -eu
mkdir -p build/account-priority
export BEND_NO_TELEMETRY=1 BUN_JSC_forceRAMSize=4294967296 TEST_JOBS=1 CHECK_JOBS=1 CHECK_SHARDS=1
python3 - <<'PY'
from pathlib import Path
import re
name = 'provider_fallback_only_when_accounts_exhausted'
laws = Path('LAWS.bend').read_text()
proof = Path('PROOF.bend').read_text()
law = re.search(r'^law ' + name + r':\n(?:  .*\n|\n)*', laws, re.M).group(0)
body = re.search(r'^def Laws\.' + name + r'\([^\n]*\n(?:  .*\n|\n)*', proof, re.M).group(0)
imports = 'import Base\nimport ../../src/core/account-fallback.bend as ACCOUNTRECOVERY\nimport ../../src/core/model.bend as M\nimport ../../src/core/json.bend as J\n'
Path('build/account-priority/policy.bend').write_text(imports + '\n' + law + '\n' + body.replace('def Laws.', 'def ', 1))
PY
rc=0
scripts/heavy bend build/account-priority/policy.bend > build/account-priority/policy.log 2>&1 || rc=1
cat build/account-priority/policy.log
TEST_FILES='test/account_failover_test.bend test/fallback_test.bend test/accounts_test.bend' scripts/test.sh > build/account-priority/focused.log 2>&1 || rc=1
cat build/account-priority/focused.log
scripts/heavy bend test/sidebar_draft_image_test.bend > build/account-priority/sidebar-full.log 2>&1 || rc=1
cat build/account-priority/sidebar-full.log
python3 - <<'PY'
from pathlib import Path
import hashlib, json
p = Path('build/account-priority/sidebar-full.log')
text = p.read_text()
count = sum(line.startswith('ok ') for line in text.splitlines())
bad = any(line.startswith(('FAIL', 'Error:', 'SOME PROOFS FAIL')) for line in text.splitlines())
receipt = {'assertions': count, 'passed': count == 24 and not bad, 'logSHA256': hashlib.sha256(p.read_bytes()).hexdigest(),
           'accountFallbackSHA256': hashlib.sha256(Path('src/core/account-fallback.bend').read_bytes()).hexdigest()}
Path('build/account-priority/sidebar-result.json').write_text(json.dumps(receipt, indent=2) + '\n')
if not receipt['passed']: raise SystemExit('Full-output sidebar24 did not pass')
PY
exit "$rc"
