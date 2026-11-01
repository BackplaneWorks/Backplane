"""Select lossless current-build PNG crops. No scaling or drawn UI.
The delegation header and content are joined with their original pixels.
"""
import hashlib,json,shutil,time
from pathlib import Path
from PIL import Image
OUT=Path(__file__).resolve().parent.parent
specs={
 'overview.png':('current-overview.png',(0,0,2000,1050),'Mochi Status, projects and threads, Tofu and Pixel; Scratch threads use the Conversations in progress shelf.'),
 'webhooks.png':('current-webhooks.png',(528,72,2000,532),'Tofu Webhooks; fictional demo-review-hook, 0 calls. Current UI displays a relative endpoint.'),
 'space.png':('current-space.png',(528,72,2000,1122),'Pixel Space, persistent structured Rev C control desk.'),
 'space-webapp.png':('current-space-webapp.png',(0,0,960,1440),'Real /bots/pixel/space.html response at 480 CSS pixels, deviceScaleFactor 2.'),
 'mailroom.png':('current-mailroom.png',(528,72,3200,1200),'Tofu Mailroom, three fictional conversations and the open Gateway handoff message; actual 2x UI pixels.')
}
rows=[]
def record(name,raw,rects,note):
 p=OUT/name
 rows.append({'path':name,'raw':'raw/'+raw,'raw_size':list(Image.open(OUT/'raw'/raw).size),'crop_rectangles':rects,'size':list(Image.open(p).size),'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'note':note})
for name,(raw,box,note) in specs.items():
 im=Image.open(OUT/'raw'/raw)
 assert box[2]<=im.width and box[3]<=im.height
 im.crop(box).save(OUT/name,optimize=True)
 record(name,raw,[{'source':box,'destination':[0,0]}],note)
im=Image.open(OUT/'raw/current-delegation.png')
result=Image.new(im.mode,(1472,1020))
rects=[{'source':[0,0,1472,72],'destination':[0,0]},{'source':[528,72,2000,1020],'destination':[0,72]}]
for r in rects: result.paste(im.crop(r['source']),tuple(r['destination']))
result.save(OUT/'delegation.png',optimize=True)
record('delegation.png','current-delegation.png',rects,'Lossless header/content composition: full current thread title and Subagents header above the expanded done rows and conversation; no sidebar or right rail.')
shutil.copyfile(OUT/'mailroom.png',OUT.parent/'mailroom-200.png')
rows.append({'path':'../mailroom-200.png','source':'mailroom.png','size':list(Image.open(OUT/'mailroom.png').size),'sha256':hashlib.sha256((OUT.parent/'mailroom-200.png').read_bytes()).hexdigest(),'note':'Byte-identical copy of the real 2x Mailroom crop. No enlargement.'})
(OUT/'source/current-selection.json').write_text(json.dumps(rows,indent=2)+'\n')
print(json.dumps([{'path':r['path'],'size':r['size']} for r in rows],indent=2))
