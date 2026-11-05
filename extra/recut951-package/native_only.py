"""Only the missing native package phase; current linked executable is immutable."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

p = argparse.ArgumentParser(description=__doc__)
for n in ('source', 'tools', 'retained', 'out'):
    p.add_argument('--'+n, type=Path, required=True)
a = p.parse_args()
here = Path(__file__).resolve().parent
build = a.source/'build/test-service'
def sha(f):
    with f.open('rb') as stream: return hashlib.file_digest(stream, 'sha256').hexdigest()
d = json.loads((build/'BUILD.json').read_text())
auth = json.loads((build/'BUILD-AUTHORIZATION.json').read_text())
assert d['source_commit'] == auth['sourceCommit'] == 'be904cd4cb21ad6febe17e674918c96945113d07'
assert d['source_tree'] == auth['sourceTree'] == '1034e9dc35fb517f1186ebcdb2d8f3351beb7a59'
assert d['testOnly'] is True and d['releaseQualified'] is False and auth['releaseUseAuthorized'] is True
assert sha(build/'BUILD.json') == auth['buildReceipt']['sha256']
binary = sha(build/'backplane-serve.exe')
assert binary == d['binary_sha256'] == auth['binary']['sha256']
subprocess.run([sys.executable, here/'prepare-tooling.py', '--source', a.source, '--output', a.tools], check=True)
subprocess.run([sys.executable, here/'native_driver.py', '--source', a.source, '--tools', a.tools,
    '--retained', a.retained, '--target', 'windows-x64', '--out', a.out], check=True)
assert sha(build/'backplane-serve.exe') == binary
