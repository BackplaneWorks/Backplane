# broken-toplevel

KiCad's `complex_hierarchy` demo (GPL-3.0-or-later, from the KiCad source tree) in the shape that made the
embedded schematic editor hang (Hubble_DK, Backplane KiCad bp.11/bp.12 before 8e57d691cd): one top-level
sheet whose UUID in the project file (`schematic.top_level_sheets`, `6a7b3c1e-…`) differs from the root
file's own (`5b9623a5-…`), with every instance path still rooted at the file's UUID, which is what a bp.11
headless save wrote. KiCad repairs the paths on every load. Before the fix the embedded editor raised a
modal "automatically fixed" dialog on its hidden pre-warm and never drew the schematic.

Use: `KEMBED_PROJ_SRC=$PWD/test/files/fixtures/broken-toplevel` for test/kicad_embed suites. Load-time
repair must never block (no modal), never mark the project modified, and Board -> Schematic must draw.
Do not save over this copy: a fixed save would repair it.
