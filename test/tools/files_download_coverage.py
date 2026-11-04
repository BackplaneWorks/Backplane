from pathlib import Path
import importlib.util
import json
import subprocess
ROOT = Path(__file__).resolve().parents[2]
base = '397cb47edeb9a5eb4f3eabf44b3be7d56b936591'
subprocess.run(['git','fetch','--no-tags','--filter=blob:none','https://github.com/BackplaneWorks/Backplane.git',base], check=True)
spec = importlib.util.spec_from_file_location('coverage', ROOT/'scripts/ahead-validation/coverage.py')
c = importlib.util.module_from_spec(spec); spec.loader.exec_module(c)
before, after = c.inventory(c.Source(base)), c.inventory(c.Source())
def missing(report): return sorted((r['file'], r['action'], r['event']) for r in report['missing'])
assert missing(before) == missing(after), {'before':missing(before),'after':missing(after)}
Path('build/files-download/coverage.json').write_text(json.dumps({'baseline':base,'existing_missing':missing(before),'new_missing':[]}, indent=2))
print('Handler coverage: no new unmatched actions; baseline has',len(before['missing']), 'known unmatched bindings')
