"""Protected target linking only from the canonical authenticated raw packet."""
import argparse
from pathlib import Path
import subprocess
import sys

p = argparse.ArgumentParser(description=__doc__)
for n in ('source', 'artifact', 'tools-out', 'inputs-out', 'output'):
    p.add_argument('--'+n, type=Path, required=True)
p.add_argument('--target', required=True)
a = p.parse_args()
here = Path(__file__).resolve().parent
for args in [
    [here/'unpack_seed.py', '--source', a.source, '--artifact', a.artifact,
     '--operation', 'js', '--out', a.inputs_out],
    [here/'prepare-tooling.py', '--source', a.source, '--output', a.tools_out],
    [here/'run-test-producer.py', '--source', a.source, '--tools', a.tools_out,
     '--inputs', a.inputs_out, '--output', a.output, '--target', a.target],
]:
    subprocess.run([sys.executable, *map(str, args)], check=True)
