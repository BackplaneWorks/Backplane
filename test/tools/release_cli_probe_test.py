#!/usr/bin/env python3
"""Exercise the exact generated synthetic CLI without a compiler or CAD tool."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--source-root', type=Path, default=Path(__file__).resolve().parents[2])
args = parser.parse_args()
source = (args.source_root/'test/tools/release_exports_e2e.ts').read_text()
match = re.search(r'writeFileSync\(join\(fake, "kicad-cli"\), (`.*?`)\);', source, re.S)
assert match, 'the production fixture generator must be present'
# Let JavaScript evaluate the literal exactly as the TypeScript fixture does,
# including its shell parameter and newline escapes.
result = subprocess.run(['node', '-e', 'let s="";process.stdin.on("data",b=>s+=b);process.stdin.on("end",()=>process.stdout.write(Function("return "+s)()));'],
                        input=match[1], text=True, capture_output=True, check=True, timeout=10)
assert result.stdout.startswith('#!/bin/sh\n')
with tempfile.TemporaryDirectory(prefix='bp-release-cli-control-') as directory:
    root = Path(directory)
    cli = root/'kicad-cli'; cli.write_text(result.stdout); cli.chmod(0o755)
    for fail_at in (1, 4, 7, 0):
        calls = root/f'calls-{fail_at}'
        env = dict(os.environ, RELEASE_CALLS=str(calls), RELEASE_FAIL_AT=str(fail_at))
        count = lambda: len(calls.read_text().splitlines()) if calls.exists() else 0
        def invoke(*parts):
            return subprocess.run([str(cli), *map(str, parts)], env=env, text=True,
                                  capture_output=True, timeout=10)
        def probe():
            before = count()
            for spelling in ('version', '--version'):
                r = invoke(spelling)
                assert r.returncode == 0 and r.stdout.strip() == '10.0.6', (spelling, r)
            r = invoke('api-server', '--describe')
            assert r.returncode == 2 and not r.stdout, r
            assert count() == before, 'resolver probes must not count as exports'
        probe()
        for number in range(1, 8):
            probe()
            out = root/f'output-{fail_at}-{number}.out'
            r = invoke('pcb', 'export', 'bom', '-o', out, 'synthetic.kicad_pcb')
            assert count() == number, 'exactly one counter record per export'
            if number == fail_at:
                assert r.returncode == 23 and not out.exists()
                assert f'synthetic export failure at {number}' in r.stderr
                break
            assert r.returncode == 0 and out.read_text().strip() == f'synthetic output {number}'
        assert count() == (fail_at or 7)
        probe()
        print(f'ok metadata probes preserve export counter and failAt={fail_at}')
# The transaction fixture has an actual generated Node shebang and must obey
# the same resolver metadata protocol without creating its invocation log.
source = (args.source_root/'test/tools/release_transaction_e2e.ts').read_text()
match = re.search(r'writeFileSync\(join\(fake, "kicad-cli"\), (`.*?`)\);', source, re.S)
assert match
result = subprocess.run(['node', '-e', 'let s="";process.stdin.on("data",b=>s+=b);process.stdin.on("end",()=>process.stdout.write(Function("node","return "+s)(process.execPath)));'],
                        input=match[1], text=True, capture_output=True, check=True, timeout=10)
with tempfile.TemporaryDirectory(prefix='bp-release-transaction-cli-control-') as directory:
    root = Path(directory); cli = root/'kicad-cli'; cli.write_text(result.stdout); cli.chmod(0o755)
    calls = root/'calls'; env = dict(os.environ, RELEASE_CALLS=str(calls), RELEASE_MODE='ok')
    for parts, code in [(('version',), 0), (('--version',), 0), (('api-server', '--describe'), 2)]:
        r = subprocess.run([str(cli), *parts], env=env, text=True, capture_output=True, timeout=10)
        assert r.returncode == code and (r.stdout.strip() == '10.0.6' if code == 0 else not r.stdout)
        assert not calls.exists(), 'transaction metadata probes must not create an export/check log'
    design = root/'input.kicad_pcb'; design.write_text('synthetic committed board')
    out = root/'output.out'
    r = subprocess.run([str(cli), 'pcb', 'export', 'bom', '-o', str(out), str(design)],
                       env=env, text=True, capture_output=True, timeout=10)
    assert r.returncode == 0 and out.read_text() == design.read_text()
    invoked = [json.loads(line) for line in calls.read_text().splitlines()]
    assert len(invoked) == 1 and invoked[0]['checking'] is False
print('ok transaction metadata probes create no check/export log; actual export remains intact')
print('PASS exact generated release CLIs: metadata is separate, export counts and failures remain exact')
