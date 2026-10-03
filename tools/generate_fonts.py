#!/usr/bin/env python3
"""Convert pinned Montserrat/LVGL glyphs into OFL-licensed embedded font assets.

Pillow (version pinned in sources.json) rasterises non-ASCII glyphs and forecast fonts.
Network is used only on a cache miss.
Pinned inputs make glyph metrics, 4-bit coverage and kerning reproducible.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import struct
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "firmware/assets/fonts"
FORECAST_SIZES = (19, 22, 31)
# Keep the UI's full Latin and symbol coverage independently of translation text.
UI_EXTRA_CHARACTERS = "°¿ÄÅÉÊÍÎÓÖÜßàáâäåçèéêëìíîïñòóôöùúûüÿĞğİıœŞş‐–’→"


def array(text, name):
    body = re.search(r"\b" + name + r"\[\]\s*=\s*\{(.*?)\};", text, re.S).group(1)
    return re.sub(r"/\*.*?\*/", "", body, flags=re.S)


def numbers(body):
    return [int(value, 0) for value in re.findall(r"-?0x[0-9a-fA-F]+|-?\d+", body)]


def field(text, name):
    return int(re.search(r"\." + name + r"\s*=\s*(-?\d+)", text).group(1))


def parse(text):
    assert field(text, "bpp") == 4 and field(text, "bitmap_format") == 0
    bitmap = bytes(numbers(array(text, "glyph_bitmap")))
    glyphs = []
    for record in re.findall(r"\{([^{}]+)\}", array(text, "glyph_dsc")):
        glyphs.append({name: field(record, name) for name in
                       ("bitmap_index", "adv_w", "box_w", "box_h", "ofs_x", "ofs_y")})
    mapping = {cp: cp - 31 for cp in range(32, 127)}
    left = numbers(array(text, "kern_left_class_mapping"))
    right = numbers(array(text, "kern_right_class_mapping"))
    values = numbers(array(text, "kern_class_values"))
    count = field(text, "right_class_cnt")

    def kern(a, b):
        return values[(left[a] - 1) * count + right[b] - 1] if left[a] and right[b] else 0
    scale = field(text, "kern_scale")
    return mapping, glyphs, bitmap, lambda a, b: (kern(a, b) * scale) >> 4


def generate(cache, check):
    sources = json.loads((ASSETS / "sources.json").read_text())
    parsed = {}
    extra = None
    forecast = None
    cache.mkdir(parents=True, exist_ok=True)
    for source in sources:
        path = cache / source["file"]
        if not path.exists():
            path.write_bytes(urllib.request.urlopen(source["url"]).read())
        data = path.read_bytes()
        if hashlib.sha256(data).hexdigest() != source["sha256"]:
            raise ValueError(f"Source checksum mismatch: {path}")
        if source["kind"] in ("extra", "forecast"):
            import PIL
            from PIL import features
            if PIL.__version__ != source["pillow_version"] or features.version_module("freetype2") != source["freetype_version"]:
                raise ValueError("Pillow/FreeType version mismatch; use the versions in sources.json")
            if source["kind"] == "extra":
                extra = path
            else:
                forecast = path
        else:
            parsed[source["size"], source["kind"]] = parse(data.decode())
    for size in (12, 14, 19, 20, 22, 31, 42):
        merged = {}
        if size in FORECAST_SIZES:
            kinds = ()  # Rasterize the complete subset from the pinned Medium TTF.
        else:
            kinds = ("medium",)
        for kind in kinds:
            mapping, glyphs, bitmap, kern = parsed[size, kind]
            for cp, glyph_id in mapping.items():
                merged[cp] = kind, glyph_id, glyphs[glyph_id], bitmap, kern
        # Include every catalog character and every accented city-keyboard key.
        catalog = (ROOT / "firmware/src/i18n.rs").read_text()
        characters = set(catalog + "°→ÉäöüÄÖÜßåÅéèêëàâçñìíòóùúœğĞşŞıİ")
        characters.update(chr(cp) for cp in range(0x410, 0x450))
        characters.update("ЁёҐґЄєІіЇї")
        if size in FORECAST_SIZES:
            characters.update(chr(cp) for cp in range(32, 127))
        elif size != 42:
            characters.update(UI_EXTRA_CHARACTERS)
        if size == 42:
            merged = {cp: value for cp, value in merged.items() if chr(cp) in "0123456789:-? "}
            characters = set("0123456789:-? ")
        additions = sorted(ord(ch) for ch in characters if ord(ch) >= 32 and ord(ch) not in merged)
        if additions:
            from PIL import ImageFont
            font = ImageFont.truetype(str(forecast if size in FORECAST_SIZES else extra), size)
            glyphs, bitmap, mapping = [], bytearray(), {}
            for cp in additions:
                mask, offset = font.getmask2(chr(cp), mode="L", anchor="ls")
                width, height = mask.size
                glyphs.append(dict(bitmap_index=len(bitmap), adv_w=round(font.getlength(chr(cp)) * 16),
                                   box_w=width, box_h=height, ofs_x=offset[0], ofs_y=-offset[1]-height))
                coverage = [min(15, (value + 8)//17) for value in bytes(mask)]
                for i in range(0, len(coverage), 2):
                    bitmap.append((coverage[i] << 4) | (coverage[i+1] if i+1 < len(coverage) else 0))
                mapping[cp] = len(glyphs) - 1
            def kern(a, b):
                return round((font.getlength(chr(additions[a])+chr(additions[b]))
                              - font.getlength(chr(additions[a])) - font.getlength(chr(additions[b]))) * 16)
            for cp, glyph_id in mapping.items():
                merged[cp] = "extra", glyph_id, glyphs[glyph_id], bitmap, kern
        assert len(merged) < 256
        pixels, records, pairs = bytearray(), bytearray(), bytearray()
        entries = sorted(merged.items())
        for cp, (_, _, g, bitmap, _) in entries:
            records.extend(struct.pack("<IIHBBbb2x", cp, len(pixels), g["adv_w"],
                                       g["box_w"], g["box_h"], g["ofs_x"], g["ofs_y"]))
            start = g["bitmap_index"]
            length = (g["box_w"] * g["box_h"] + 1) // 2
            assert start + length <= len(bitmap)
            pixels.extend(bitmap[start:start + length])
        for a, (_, (kind_a, id_a, _, _, kern)) in enumerate(entries):
            for b, (_, (kind_b, id_b, _, _, _)) in enumerate(entries):
                value = kern(id_a, id_b) if kind_a == kind_b else 0
                if value:
                    pairs.extend(struct.pack("<BBb", a, b, value))
        for suffix, data in (("glyphs", records), ("bitmap", pixels), ("kerning", pairs)):
            path = ASSETS / f"montserrat-{size}.{suffix}"
            if check:
                if path.read_bytes() != data:
                    raise ValueError(f"Generated asset differs: {path}")
            else:
                path.write_bytes(data)
        print(f"{size}px: {len(entries)} glyphs, {len(pixels)} bitmap bytes, {len(pairs)//3} kerning pairs")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-dir", type=Path, default=ROOT / "firmware/target/font-sources")
    parser.add_argument("--check", action="store_true", help="Verify checked-in assets without rewriting them")
    args = parser.parse_args()
    generate(args.source_dir, args.check)
