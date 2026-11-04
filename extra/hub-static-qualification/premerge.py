"""Boat-only merged-law shape and exact scoped-input preservation; no Bend run."""
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess

ROOT=Path.cwd()
OUT=ROOT/'build/hub-static-premerge'
PACKET=ROOT/'extra/hub-static-merge-packet'
group=Path('/sys/fs/cgroup')/Path('/proc/self/cgroup').read_text().split('0::',1)[1].strip().lstrip('/')
assert group.name.startswith('bp-static-premerge-')
assert 0<int((group/'memory.max').read_text())<=536870912
assert (group/'memory.swap.max').read_text().strip()=='0'
assert not OUT.exists(), 'inspect the existing attempt'
OUT.mkdir(parents=True)
summary=json.loads((ROOT/'extra/hub-static-qualification/SUMMARY.json').read_text())
identity=json.loads((PACKET/'identity.json').read_text())
sha=lambda b:hashlib.sha256(b).hexdigest()
compiled=[n for n in summary['source_sha256'] if n.startswith(('src/core/','src/server/')) and n!='src/server/server.bend']
boundaries=['test/tools/static_file_test.mjs','test/tools/asset_file_test.mjs','test/tools/preview_file_test.mjs','test/fixtures/static_http_reference.json']
for name in compiled+boundaries:
    assert sha((ROOT/name).read_bytes())==summary['source_sha256'][name], name
for name in ['LAWS.bend','PROOF.bend']:
    assert sha((PACKET/name).read_bytes())==identity['sha256'][name], name
r=subprocess.run(['python3','scripts/laws-shape.py',str(PACKET/'LAWS.bend'),str(PACKET/'PROOF.bend')],text=True,capture_output=True)
(OUT/'laws-shape.txt').write_text(r.stdout+r.stderr)
assert r.returncode==0
spec=importlib.util.spec_from_file_location('proof_slice',ROOT/'scripts/proof-slice.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
laws=m.Module(ROOT,'extra/hub-static-merge-packet/LAWS.bend')
proof=m.Module(ROOT,'extra/hub-static-merge-packet/PROOF.bend')
for name,digests in summary['canonical_blocks_sha256'].items():
    assert [sha(laws.by_name[name][0]['text'].encode()),sha(proof.by_name['Laws.'+name][0]['text'].encode())]==digests,name
server=m.Module(ROOT,'src/server/server.bend')
audit=json.loads((ROOT/'extra/hub-static-qualification/source-audit.json').read_text())
for name,body in audit['delegates'].items():
    lines=server.by_name[name][0]['text'].splitlines()
    while lines and (not lines[-1].strip() or lines[-1].startswith('#')):lines.pop()
    assert len(lines)==2 and lines[1].strip()==body,name
end={k:int(v) for k,v in (line.split() for line in (group/'memory.events').read_text().splitlines())}
assert end['oom']==end['oom_kill']==0
out={'state':'passed','kind':'fresh merged-law shape + exact scoped input/delegate preservation, no new proof execution',
     'tested_source_head':summary['source_head'],'merge_identity':identity,'compiled_modules_unchanged':compiled,
     'boundary_inputs_unchanged':boundaries,'canonical_obligations_unchanged':list(summary['canonical_blocks_sha256']),
     'server_delegates_unchanged':list(audit['delegates']),'memory_max_bytes':int((group/'memory.max').read_text()),
     'swap_max_bytes':0,'memory_peak_bytes':int((group/'memory.peak').read_text()),'events':end,
     'full_server_qualified':False,'proof_executions':0}
(OUT/'SUMMARY.json').write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps(out))
