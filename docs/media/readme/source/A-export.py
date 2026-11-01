from pathlib import Path
from PIL import Image
import json
import sys
M=Path(__file__).resolve().parents[1]
recipes=[]
def source(p): return Image.open(M/p).convert('RGB')
def crop(im,box): return im.crop(tuple(v*2 for v in box))
def save(im,path,parts):
 if '--round2' not in sys.argv or path in ['cad/kicad-freecad-setup-200.png','cad/pad-workbar-detail-200.png']:
  im.save(M/path,optimize=True)
 recipes.append({'path':path,'size':list(im.size),'parts_css_2x':parts})
# Two upper viewer panes, one wide lower schematic pane. Crops remain at native 2x.
a=crop(source('source/A-3d-frame.png'),(825,36,1600,600))
b=crop(source('source/A-board-frame.png'),(825,36,1600,600))
c=crop(source('source/A-schematic-frame.png'),(638,36,2200,755))
out=Image.new('RGB',(3172,2638),(11,13,16))
out.paste(a,(24,24));out.paste(b,(1598,24));out.paste(c,(24,1176))
save(out,'royalblue-viewers-200.png',[['source/A-3d-frame.png',[825,36,1600,600],[24,24]],['source/A-board-frame.png',[825,36,1600,600],[1598,24]],['source/A-schematic-frame.png',[638,36,2200,755],[24,1176]]])
save(crop(source('source/A-picker-frame.png'),(422,644,1366,994)),'agent-setup-200.png',[['source/A-picker-frame.png',[422,644,1366,994]]])
save(source('cad/source/A-mech-frame.png'),'cad/cm5-mech-200.png',[['cad/source/A-mech-frame.png','full frame']])
im=source('cad/source/A-settings-frame.png')
# Actual heading and hardware rows, removing the intervening navigation gap.
out=Image.new('RGB',(1676,582),(12,15,18));out.paste(crop(im,(288,48,1126,80)),(0,0));out.paste(crop(im,(288,100,1126,359)),(0,64))
save(out,'cad/kicad-freecad-setup-200.png',[['cad/source/A-settings-frame.png',[288,48,1126,80],[0,0]],['cad/source/A-settings-frame.png',[288,100,1126,359],[0,64]]])
im=source('cad/source/A-pad-frame.png')
# Round 2: one contiguous crop of the actual Split frame. Both panes cross-probe +BATT.
save(crop(im,(625,36,2424,650)),'cad/pad-workbar-detail-200.png',[['cad/source/A-pad-frame.png',[625,36,2424,650]]])
(M/'source/A-exports.json').write_text(json.dumps(recipes,indent=2)+'\n')
for r in recipes: print(r['path'],r['size'])
