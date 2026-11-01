"""Lossless crops and vertical stack of real 2x UI frames. No rescaling."""
import json
from pathlib import Path
from PIL import Image

src=Path(__file__).resolve().parent
features=src.parents[1]
media=features.parent
panels=json.loads((src/'panels.json').read_text())
for name,spec in panels.items():
    r=spec['cropCSS']; d=spec['deviceScaleFactor']
    rect=(r['x']*d,r['y']*d,(r['x']+r['width'])*d,(r['y']+r['height'])*d)
    frame=Image.open(src/spec['raw'])
    panel=frame.crop(rect)
    panel.save(src/(name+'.png'))
    panel.save(features/(name+'-2x.png'))
names=['thread','room','project','bot']
ims=[Image.open(src/('reference-'+n+'.png')).convert('RGB') for n in names]
gap=16
stack=Image.new('RGB',(ims[0].width,sum(im.height for im in ims)+gap*3),ims[0].getpixel((0,0)))
y=0
for im in ims:
    assert im.width==stack.width
    stack.paste(im,(0,y)); y+=im.height+gap
stack.save(media/'reference-menus-200.png')
manifest={}
for name in ['features/link-cards-2x.png','reference-menus-200.png','features/inline-terminal-2x.png','features/btw-autocomplete-2x.png']:
    manifest[name]=list(Image.open(media/name).size)
(src/'outputs.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps(manifest,indent=2))
