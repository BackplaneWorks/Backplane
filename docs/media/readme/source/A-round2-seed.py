"""Run once on a fresh A-round2 folder, then A-round2-hub.sh start / desk.
The neutral /tmp paths exist only inside that runner's private mount namespace.
"""
from pathlib import Path
import json, shutil
R=Path('<capture-root>')
M=R/'repo/docs/media/readme';D=R/'run/A-round2'
if (D/'home').exists(): raise RuntimeError('Use a fresh run folder; never reseed a running hub.')
for n in ['home','fakehome','agents']: (D/n).mkdir(parents=True,exist_ok=True)
# Byte-identical public CAD, excluding only the separate NFC antenna project.
# This build resets Split sub-sheet navigation when multiple boards are indexed.
shutil.copytree(R/'projects/royalblue54L_feather',D/'royalblue54L_feather',ignore=shutil.ignore_patterns('nfc_antenna'))
ev=[json.loads(x) for x in (M/'source/A-demo-events.jsonl').read_text().splitlines()]
for e in ev:
 if e['$']=='ProjectCreated' and e['id']=='royalblue': e['root']='/tmp/readme-demo/royalblue54L_feather'
s=''.join(json.dumps(e)+'\n' for e in ev)
(D/'home/events.jsonl').write_text(s);(M/'source/A-round2-demo-events.jsonl').write_text(s)
