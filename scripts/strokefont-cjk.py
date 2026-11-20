#!/usr/bin/env python3
# strokefont-cjk.py NEWSTROKE_FONT_CPP: the CJK Unified Ideographs (U+4E00..U+9FFF) of KiCad's newstroke stroke font, for the hub's stroke text
# (native/host/src/kicad/strokefont.rs), so Chinese and Japanese text in Altium (and KiCad) designs draws as KiCad draws it instead of as boxes.
# Writes native/host/src/kicad/strokefont_cjk.z: zlib of one line per code point from U+4E00 to U+9FFF, each KiCad's glyph string as it is
# (two bounds, then coordinate pairs as characters offset by 'R', " R" lifting the pen); an empty line where the font has no glyph.
# The glyphs were made from Source Han Sans (SIL OFL 1.1) by Lingdong Huang's code (MIT); both notices are in strokefont_cjk.LICENSE.
# Source: KiCad common/newstroke_font.cpp (any revision since 2019; the table is indexed by code point - 32).
import re, sys, zlib
src = open(sys.argv[1], encoding="utf-8").read()
body = src[src.index("newstroke_font[] ="):]
body = body[body.index("{") + 1:body.index("};")]
body = re.sub(r"/\*.*?\*/", "", body, flags=re.S)
glyphs = [m.group(1).encode("utf-8").decode("unicode_escape") for m in re.finditer(r'"((?:[^"\\]|\\.)*)"', body)]
box = "F^K[KFYFY[K["   # the font's own "no glyph" box
lines = []
for cp in range(0x4E00, 0xA000):
    g = glyphs[cp - 32] if cp - 32 < len(glyphs) else ""
    lines.append("" if g == box or len(g) < 4 else g)
assert len(lines) == 0xA000 - 0x4E00 and all("\n" not in l for l in lines)
out = sys.argv[2] if len(sys.argv) > 2 else "native/host/src/kicad/strokefont_cjk.z"
open(out, "wb").write(zlib.compress("\n".join(lines).encode("ascii"), 9))
print(f"{out}: {sum(1 for l in lines if l)} glyphs")
