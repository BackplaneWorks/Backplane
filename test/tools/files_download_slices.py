"""Exact declaration slices for #931 under the person's 3.7 GB Boat cap.

This validates the changed boundaries; it is explicitly not a full build or
proof gate. proof-slice independently verifies every kept declaration/effect
against the canonical source bytes, including the law and proof statements.
"""
from pathlib import Path
import importlib.util
import json
import re
ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('slices', ROOT/'scripts/proof-slice.py')
slices = importlib.util.module_from_spec(spec); spec.loader.exec_module(slices)
s = slices.Slicer(ROOT)
scopes = {
    'route': [('src/server/server.bend', n) for n in ['FilesHttp.serve', 'Hub.files.root', 'Conn.allowed', 'Env.get']] + [('src/server/host.bend', 'Sock.recv_bytes'), ('src/core/http.bend', 'Http.request'), ('src/core/http.bend', 'Http.route')],
    'files-ui': [('src/web/view.bend', 'View.files'), ('src/mobile/view.bend', 'Files.json')],
    'proof': [('PROOF.bend', n) for n in re.findall(r'^def (Laws.fm_zip_[\w.]+|Laws.fm_get_[\w.]+|Laws.fm_files_[\w.]+|Laws.composer_bar_[\w.]+)\(', (ROOT/'PROOF.bend').read_text(), re.M)],
}
# Every registration in this one shared effect file needs its canonical
# declaration at emission time, even when the fixture calls only sockets.
registrations = sorted(set(re.findall(r'CID\(([^)]+)\)', (ROOT/'src/server/effects/host.js').read_text())))
scopes['route'] += [('src/server/host.bend', n) for n in registrations
                    if ('src/server/host.bend', n) not in scopes['route']]
for name, roots in scopes.items():
    seeds = [b for rel, n in roots for b in s.blocks(rel, n, 'def')]
    assert len(seeds) == len(roots), (name, roots)
    kept = s.write_shard(seeds, ROOT/'build/files-download'/name)
    print(name, len(kept), 'byte-verified canonical declarations')
