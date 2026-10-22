import Base

# System font effects (effects/font.c): fontconfig finds a font file,
# FreeType rasterizes its glyphs. Both are dlopen'd at run time; without
# them Font.find / Font.open fail and text.bend keeps the bitmap font.
# Only words cross the boundary; text.bend decodes them.

# the best match for a fontconfig pattern ("Liberation Mono",
# "monospace:weight=bold"): (family, (file, face index))
def Font.find(pattern: String) -> IO(Result<&1, &1, U32 & String, String & String & U32>):
  import "./effects/font.js"

# a face of the file at px pixels per em; the answer is a face id
def Font.open(path: String, index: U32, px: U32) -> IO(Result<&1, &1, U32 & String, U32>):
  import "./effects/font.js"

# [ascender, descender, line height, max advance], 26.6 pixels; [] for a
# bad face
def Font.metrics(face: U32) -> IO(List<&2, U32>):
  import "./effects/font.js"

# [found, advance (26.6), left + 32768, top + 32768, w, h, coverage...]:
# w*h coverage bytes row-major, four per word, first pixel in the low
# byte; [] when the glyph cannot be loaded
def Font.glyph(face: U32, cp: U32) -> IO(List<&2, U32>):
  import "./effects/font.js"

# kerning between two code points, 26.6 pixels + 32768
def Font.kern(face: U32, a: U32, b: U32) -> IO(U32):
  import "./effects/font.js"
