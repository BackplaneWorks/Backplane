"""Unpack only the pinned canonical Actions seed; no URL or emitter path."""
import argparse
import hashlib
import importlib.util
from pathlib import Path
import tarfile

p = argparse.ArgumentParser(description=__doc__)
for n in ('source', 'artifact', 'out'):
    p.add_argument('--'+n, type=Path, required=True)
p.add_argument('--operation', choices=['js', 'android'], required=True)
a = p.parse_args()
wanted = {'js': ('4708b3cc7c6f69d8e4fba0bd7e0651a6e58f7486c778960805b8ea46541dbf72', 11728502),
          'android': ('bf2576829d84d213990b6d352b6b068c10fdf75f3af7273ae7870f1526bd2c14', 31730640)}
packet = a.artifact/'seed-inputs.tar.gz'
assert packet.is_file() and not packet.is_symlink() and not a.out.exists()
with packet.open('rb') as stream:
    assert (hashlib.file_digest(stream, 'sha256').hexdigest(), packet.stat().st_size) == wanted[a.operation]
with tarfile.open(packet) as archive:
    members = archive.getmembers()
    assert sum(x.size for x in members) < 512*1024**2
    assert all((x.isfile() or x.isdir()) and not x.name.startswith('/')
               and '..' not in Path(x.name).parts for x in members)
    archive.extractall(a.out, filter='data')
spec = importlib.util.spec_from_file_location('source_bootstrap', a.source/'scripts/release-bootstrap.py')
b = importlib.util.module_from_spec(spec)
spec.loader.exec_module(b)
c = 'be904cd4cb21ad6febe17e674918c96945113d07'
t = b.source_identity(c, a.source)
assert t == '1034e9dc35fb517f1186ebcdb2d8f3351beb7a59'
b.verify(a.out, c, t, a.operation)
print('ACTUAL_CANONICAL_ACTIONS_SEED_SOURCE_HASH_BOUND', a.operation)
