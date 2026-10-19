"""Lossless crops only; never synthesize or enlarge UI pixels.
Run after capture.py: uv run --with pillow source/finish.py
"""
import hashlib,json
from pathlib import Path
from PIL import Image
OUT=Path(__file__).resolve().parent.parent
specs={
 'overview.png':('uberbot-overview.png',(0,0,2000,1050),'Uberbot, projects, threads and persistent bots; installed web client at 2x.'),
 'delegation.png':('subagent-panel.png',(528,0,2000,1020),'Actual expanded Subagents panel, Claude and Grok tasks under a Codex parent; installed web client at 2x.'),
 'space.png':('catbot-space.png',(528,0,2000,1050),'Persistent structured Space dashboard in the real bot tab; installed web client at 2x.'),
 'mailroom.png':('native-mail-conversation.png',(264,0,1600,440),'Native Mailroom: seeded conversations and one open body. Exact 1x pixels; no scaling.'),
 'webhooks.png':('catbot-webhooks.png',(528,0,2000,460),'Actual Webhooks tab, fictional localhost hook; installed web client at 2x.'),
 'space-webapp.png':('space-webapp-mobile.png',(0,0,960,1440),'Real HTML Space endpoint, responsive demo webapp at 480 CSS pixels and 2x device density.'),
 'delegation-mobile.png':('subagents-mobile.png',(0,0,960,1440),'Actual responsive thread/subagent UI at 480 CSS pixels and 2x device density.'),
 'space-mobile.png':('space-mobile.png',(0,0,960,1440),'Actual responsive structured Space tab at 480 CSS pixels and 2x device density.')
}
rows=[]
for name,(source,box,note) in specs.items():
 src=OUT/'raw'/source
 im=Image.open(src)
 assert box[2]<=im.width and box[3]<=im.height,(source,im.size,box)
 dest=OUT/name
 im.crop(box).save(dest,optimize=True)
 rows.append({'path':name,'raw':'raw/'+source,'raw_size':list(im.size),'crop_pixels':list(box),'size':list(Image.open(dest).size),'sha256':hashlib.sha256(dest.read_bytes()).hexdigest(),'note':note})
(OUT/'source/selection.json').write_text(json.dumps(rows,indent=2)+'\n')
print(json.dumps(rows,indent=2))
