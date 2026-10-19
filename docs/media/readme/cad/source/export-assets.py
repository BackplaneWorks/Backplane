"""Crop/scale actual screenshots and the existing cat sheet; no UI painting.
Run: uv run --no-project --python /usr/bin/python3 python export-assets.py
"""
from pathlib import Path
import hashlib
import json
from PIL import Image
ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / 'source'
exports = []

def export(name, source, box=None, size=None):
    src = ROOT / source
    im = Image.open(src).convert('RGB')
    native = im.size
    if box:
        im = im.crop(box)
    if size is None:
        size = (im.width * 2, im.height * 2)
    im = im.resize(size, Image.Resampling.LANCZOS)
    dest = ROOT / name
    im.save(dest)
    exports.append({'file': name, 'source': source, 'source_sha256': hashlib.sha256(src.read_bytes()).hexdigest(), 'native_size': native, 'crop': box, 'output_size': im.size, 'method': 'crop and Lanczos resample only; no content edits', 'sha256': hashlib.sha256(dest.read_bytes()).hexdigest()})

def join_crops(name, source, boxes, direction):
    """Join native screenshot rectangles; remove gaps without painting UI."""
    src = ROOT / source
    original = Image.open(src).convert('RGB')
    pieces = [original.crop(box) for box in boxes]
    if direction == 'horizontal':
        assert len({p.height for p in pieces}) == 1
        joined = Image.new('RGB', (sum(p.width for p in pieces), pieces[0].height))
    else:
        assert len({p.width for p in pieces}) == 1
        joined = Image.new('RGB', (pieces[0].width, sum(p.height for p in pieces)))
    offset = 0
    for piece in pieces:
        joined.paste(piece, (offset, 0) if direction == 'horizontal' else (0, offset))
        offset += piece.width if direction == 'horizontal' else piece.height
    # Every output pixel before scaling belongs to one of the source crops.
    joined = joined.resize((joined.width * 2, joined.height * 2), Image.Resampling.LANCZOS)
    dest = ROOT / name
    joined.save(dest)
    exports.append({'file': name, 'source': source, 'source_sha256': hashlib.sha256(src.read_bytes()).hexdigest(), 'native_size': original.size, 'crop_rectangles': boxes, 'join': direction, 'output_size': joined.size, 'method': 'same-frame native crops joined to remove empty space, then 200% Lanczos; no UI drawing or text changes', 'sha256': hashlib.sha256(dest.read_bytes()).hexdigest()})

if (ROOT / 'source/pad-mentioned-native.png').exists():
    export('pad-mentioned-200.png', 'source/pad-mentioned-native.png')
    export('pad-selected-200.png', 'source/pad-selected-native.png')
    export('pad-workbar-detail-200.png', 'source/pad-mentioned-native.png', (295,660,1368,980))
    join_crops('pad-mention-compact-200.png', 'source/pad-mentioned-native.png',
               [(296,759,900,980), (948,668,1368,889)], 'horizontal')
export('kicad-freecad-setup-200.png', 'source/kicad-freecad-settings-native.png', (295,305,901,638))
if (ROOT / 'source/enclosure-mech-native.png').exists():
    export('enclosure-mech-full-200.png', 'source/enclosure-mech-native.png')
    export('enclosure-mech-200.png', 'source/enclosure-mech-native.png', (933, 0, 1600, 1000))
    join_crops('enclosure-mech-landscape-200.png', 'source/enclosure-mech-native.png',
               [(933,0,1600,95), (933,312,1600,713)], 'vertical')
export('cm5-mech-200.png', 'source/web-mech-existing.png', size=(4320,2700))
export('cm5-mech-detail-200.png', 'source/web-mech-existing.png', (1237,0,2160,1350), (1846,2700))
# Three actual cells from docs/media/cats.png: idle, work, think.
# The original solid background remains; no invented strokes or characters.
cats = Image.open(SOURCE / 'cats-original.png').convert('RGB')
footer = Image.new('RGB', (432,144), cats.getpixel((0,0)))
boxes = [(0,0,230,234), (460,0,690,234), (690,0,920,234)]
for i, box in enumerate(boxes):
    im = cats.crop(box).resize((132,134), Image.Resampling.LANCZOS)
    footer.paste(im, (i*144+6,5))
footer.save(ROOT / 'cats-footer-200.png')
exports.append({'file': 'cats-footer-200.png', 'source': 'source/cats-original.png', 'source_sha256': hashlib.sha256((SOURCE/'cats-original.png').read_bytes()).hexdigest(), 'crop_cells': boxes, 'output_size': [432,144], 'display_size': [216,72], 'background': '#111111', 'method': 'three original cat cells, cropped and downsampled; no redrawing'})
(SOURCE / 'exports.json').write_text(json.dumps(exports,indent=2)+'\n')
