#!/usr/bin/env python3
"""Save the README's curated PNG screenshots from actual firmware PPM previews."""
import argparse
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]


def generate(previews: Path) -> None:
    images = ROOT / "images"
    sources = {
        "settings-en": "settings",
        "languages-en": "languages",
        "guide-networks": "wifi-networks",
        "guide-password": "wifi-password",
        "location-input-en": "location-input",
        "location-results-en": "location-results",
        "sleep-timeout-en": "sleep-timeout",
        "settings-uk": "settings-uk",
        "location-cyrillic-lower-uk": "location-cyrillic-uk",
    }
    for language in ("en", "uk", "ru"):
        for view in ("Clock", "Week", "Hours"):
            suffix = "" if language == "en" else f"-{language}"
            sources[f"weather-{view}-ready-{language}"] = f"weather-{view.lower()}{suffix}"
    # Verify all inputs before replacing any documentation screenshots.
    for source in sources:
        if not (previews / f"{source}.ppm").is_file():
            raise FileNotFoundError(previews / f"{source}.ppm")
    for source, destination in sources.items():
        with Image.open(previews / f"{source}.ppm") as image:
            if image.size != (320, 480):
                raise ValueError(f"Unexpected preview dimensions: {source}: {image.size}")
            image.save(images / f"{destination}.png")
    # Keep the overview in sync with the three English weather pages.
    overview = Image.new("RGB", (976, 480), "white")
    for index, view in enumerate(("clock", "week", "hours")):
        with Image.open(images / f"weather-{view}.png") as page:
            overview.paste(page, (index * 328, 0))
    overview.save(images / "weather-overview.png")
    # These UI screenshots are superseded by the guide's English/Ukrainian examples.
    for obsolete in ("settings-ru", "languages-ru", "location-cyrillic-ru", "location-cyrillic-upper-ru"):
        (images / f"{obsolete}.png").unlink(missing_ok=True)
    print(f"Saved {len(sources)} README screenshots and weather overview to {images}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("previews", type=Path, nargs="?", default=Path("/tmp/weather-wifi-preview"))
    generate(parser.parse_args().previews)
