"""Boat-only scoped check receipt; canonical proof keeps its separate verdict."""
import hashlib
import json
from pathlib import Path
import subprocess

out = Path('build/issue927-native-fix')
out.mkdir(parents=True, exist_ok=True)
group = Path('/sys/fs/cgroup') / Path('/proc/self/cgroup').read_text().split('0::', 1)[1].strip().lstrip('/')
memory = int((group / 'memory.max').read_text())
swap = int((group / 'memory.swap.max').read_text())
assert 0 < memory <= 3_700_000_000 and swap == 0
record = {'state': 'incomplete', 'memoryMaxBytes': memory, 'swapMaxBytes': swap,
          'sources': {p: hashlib.sha256(Path(p).read_bytes()).hexdigest() for p in
                      ['src/server/host.bend', 'src/server/effects/host.js', 'src/server/kicad.bend']},
          'checks': []}
for command in [['bun', 'test/tools/atomic_text_test.cjs'],
                ['python3', 'test/tools/kicad_cache_test.py'],
                ['python3', 'test/tools/kicad_discovery_test.py'],
                ['python3', 'test/tools/kicad_install_test.py'],
                ['python3', 'scripts/laws-shape.py']]:
    result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=120)
    log = str(len(record['checks'])) + '.txt'
    (out / log).write_bytes(result.stdout)
    print(result.stdout.decode(errors='replace'), flush=True)
    record['checks'].append({'command': command, 'exit': result.returncode, 'log': log})
    (out / 'CHECKS.json').write_text(json.dumps(record, indent=2) + '\n')
    if result.returncode:
        raise SystemExit(result.returncode)
record['state'] = 'passed'
record['canonicalProofQualified'] = False
(out / 'CHECKS.json').write_text(json.dumps(record, indent=2) + '\n')
print('Scoped checks passed; canonical PROOF is a separate verdict.', flush=True)
