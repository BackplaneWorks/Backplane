"""Lossless crops of actual native pixels, plus explicitly scaled 2x exports.

Run after capture-features.py. No UI is drawn or reconstructed here.
"""
import json
from pathlib import Path
from PIL import Image

OUT = Path(__file__).resolve().parents[1]
panels = {
    'reference-thread': (530, 772, 1334, 990),
    'reference-room': (530, 772, 1334, 990),
    'reference-project': (530, 772, 1334, 990),
    'reference-bot': (530, 772, 1334, 990),
    'btw-autocomplete': (530, 830, 1334, 990),
    'btw-draft': (530, 865, 1334, 990),
    'inline-terminal': (530, 348, 1285, 677),
    'model-picker': (530, 748, 1334, 990),
}
manifest = {}
for name, box in panels.items():
    im = Image.open(OUT / 'raw' / (name + '.png')).crop(box)
    im.save(OUT / (name + '.png'))
    # An enlarged pixel export for documentation composition, not extra detail.
    im.resize((im.width * 2, im.height * 2), Image.Resampling.NEAREST).save(OUT / (name + '-2x.png'))
    manifest[name] = {'source': 'raw/' + name + '.png', 'crop': box, 'native_size': im.size, '2x': 'Nearest-neighbor enlargement of actual native pixels; not a higher-resolution rerender.'}
candidate = OUT / 'links/raw/link-cards.png'
if candidate.exists():
    box = (530, 136, 1030, 289)
    im = Image.open(candidate).crop(box)
    im.save(OUT / 'link-cards.png')
    im.resize((im.width * 2, im.height * 2), Image.Resampling.NEAREST).save(OUT / 'link-cards-2x.png')
    manifest['link-cards'] = {'source': 'links/raw/link-cards.png', 'crop': box, 'native_size': im.size, 'runtime': 'Existing run3 candidate binary; see links/source/capture.json. Render appearance only; activation not tested.', '2x': 'Nearest-neighbor enlargement of actual native pixels; not a higher-resolution rerender.'}
(OUT / 'source/panels.json').write_text(json.dumps(manifest, indent=2) + '\n')
