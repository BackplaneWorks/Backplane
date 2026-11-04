"""Run the canonical sidebar regression through a byte-verified exact slice."""
from pathlib import Path
import importlib.util
import subprocess
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('slice', ROOT/'scripts/proof-slice.py')
slice=importlib.util.module_from_spec(spec);spec.loader.exec_module(slice)
s=slice.Slicer(ROOT)
folder=ROOT/'build/files-download/sidebar'
kept=s.write_shard(s.blocks('test/sidebar_draft_image_test.bend','main','def'), folder)
print('sidebar regression:',len(kept),'byte-verified declarations',flush=True)
entry=folder/'run.bend'
entry.write_text('import Base\nimport ./t/test/sidebar_draft_image_test.bend as F\n\ndef main() -> IO(Unit):\n  F.main()\n')
r=subprocess.run(['bend',str(entry)],capture_output=True,text=True,timeout=180)
text=r.stdout+r.stderr
(ROOT/'build/files-download/sidebar.txt').write_text(text)
print(text,flush=True)
if r.returncode or 'FAIL' in text or len([l for l in text.splitlines() if l.startswith('ok')])!=24:
    raise SystemExit(1)
