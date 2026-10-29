#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = ["pillow==12.1.1"]
# ///
"""Reject a flat screenshot body even when accessibility reports visible views.

Run with Pillow 12.1+ and a measured content rectangle in screenshot pixels, excluding
system status and tab bars. This detects the reproduced tab-background cover;
it does not prove useful content, freshness, or accessibility. Check real view
frames and review the screenshot as well. No image content is printed.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path

from PIL import Image


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("screenshot", type=Path)
    parser.add_argument("--rect", nargs=4, type=int, required=True,
                        metavar=("X", "Y", "WIDTH", "HEIGHT"))
    args = parser.parse_args()
    x, y, width, height = args.rect
    with Image.open(args.screenshot) as image:
        if (x < 0 or y < 0 or width <= 0 or height <= 0 or
                x + width > image.width or y + height > image.height):
            parser.error("content rectangle must be inside the screenshot")
        body = image.convert("RGB").crop((x, y, x + width, y + height))
        counts = Counter(body.get_flattened_data())
    pixels = width * height
    # The failure paints only the selected and unselected tab backgrounds over
    # the entire body. A tiny allowance covers screenshot color rounding.
    background_pixels = sum(count for _, count in counts.most_common(2))
    non_background_fraction = (pixels - background_pixels) / pixels
    flat = non_background_fraction < 0.002
    print(json.dumps({
        "screenshotSha256": hashlib.sha256(args.screenshot.read_bytes()).hexdigest(),
        "contentRectPixels": args.rect,
        "pixels": pixels,
        "distinctColors": len(counts),
        "nonBackgroundFraction": round(non_background_fraction, 6),
        "flatBody": flat,
        "scope": "blank-body rejection only; frames, visual and freshness checks remain required",
    }, sort_keys=True))
    return 1 if flat else 0


if __name__ == "__main__":
    raise SystemExit(main())
