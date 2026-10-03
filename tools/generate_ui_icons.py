#!/usr/bin/env python3
"""Generate the pinned MIT Tabler settings icon as a white alpha mask."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import cairosvg
import cairocffi
import PIL
from PIL import Image

root = Path(__file__).resolve().parents[1] / 'firmware/assets/ui'
parser = argparse.ArgumentParser()
parser.add_argument('--check', action='store_true')
args = parser.parse_args()
assert cairosvg.__version__ == '2.7.1' and PIL.__version__ == '9.0.1'
assert cairocffi.cairo_version_string() == '1.16.0'
svg = (root / 'settings.svg').read_bytes()
assert hashlib.sha256(svg).hexdigest() == json.loads((root / 'sources.json').read_text())['svg_sha256']
png = cairosvg.svg2png(bytestring=svg.replace(b'currentColor', b'#ffffff'), output_width=32, output_height=32)
data = Image.open(io.BytesIO(png)).convert('RGBA').getchannel('A').tobytes()
path = root / 'settings-32.alpha'
if args.check:
    assert path.read_bytes() == data, 'UI icon differs from pinned source'
else:
    path.write_bytes(data)
print('Verified settings icon' if args.check else 'Generated settings icon')
