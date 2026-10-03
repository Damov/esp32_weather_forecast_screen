#!/usr/bin/env python3
"""Convert pinned MIT-licensed Meteocons SVGs to RGB565 + 8-bit alpha.
Requires CairoSVG 2.7.1 and Pillow 9.0.1. No runtime icon dependencies.
"""
import argparse
import base64
import hashlib
import io
import json
from pathlib import Path
import tarfile
import urllib.request
import cairosvg
import cairocffi
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / 'firmware/assets/weather'
NAMES = ['clear-day', 'clear-night', 'partly-cloudy-day', 'partly-cloudy-night',
         'overcast', 'fog', 'drizzle', 'sleet', 'rain', 'snow',
         'thunderstorms', 'thunderstorms-hail', 'not-available']


def generate(check):
    import PIL
    if cairosvg.__version__ != '2.7.1' or PIL.__version__ != '9.0.1':
        raise ValueError('Use CairoSVG 2.7.1 and Pillow 9.0.1')
    source = json.loads((ASSETS / 'sources.json').read_text())
    if cairocffi.cairo_version_string() != source['cairo']:
        raise ValueError('Use Cairo ' + source['cairo'])
    cache = ROOT / 'firmware/target/weather-sources'
    cache.mkdir(parents=True, exist_ok=True)
    archive = cache / 'svg-static-0.1.0.tgz'
    if not archive.exists():
        archive.write_bytes(urllib.request.urlopen(source['url']).read())
    data = archive.read_bytes()
    integrity = 'sha512-' + base64.b64encode(hashlib.sha512(data).digest()).decode()
    if integrity != source['integrity']:
        raise ValueError('Upstream archive checksum mismatch')
    files = {}
    with tarfile.open(fileobj=io.BytesIO(data), mode='r:gz') as tar:
        files['MIT.txt'] = tar.extractfile('package/LICENSE').read()
        for name in NAMES:
            svg = tar.extractfile('package/flat/' + name + '.svg').read()
            files['svg/' + name + '.svg'] = svg
            for size in (32, 72):
                png = cairosvg.svg2png(bytestring=svg, output_width=size, output_height=size)
                image = Image.open(io.BytesIO(png)).convert('RGBA')
                bitmap = bytearray()
                for r, g, b, a in image.getdata():
                    color = ((r*31+127)//255 << 11) | ((g*63+127)//255 << 5) | ((b*31+127)//255)
                    bitmap.extend((color & 255, color >> 8, a))
                files[f'{name}-{size}.bitmap'] = bytes(bitmap)
    files["checksums.json"] = (json.dumps({name: hashlib.sha256(data).hexdigest() for name, data in sorted(files.items())}, indent=2) + "\n").encode()
    for name, data in files.items():
        path = ASSETS / name
        if check:
            if path.read_bytes() != data:
                raise ValueError('Generated asset differs: ' + str(path))
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
    print(f'Verified {len(NAMES)} symbols at 32px and 72px' if check else f'Generated {len(NAMES)} symbols at 32px and 72px')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    generate(parser.parse_args().check)
