---
name: sendcutsend
description: Design flat and bent parts for SendCutSend (laser, waterjet and CNC-router cut sheet metal, MDF, plywood, acrylic, composites; bending, tapping, countersinking, PEM hardware, deburring, powder coat, anodize, plating) and prepare them for ordering. Picks material and thickness from SendCutSend's live catalog, checks the DXF or STEP against its live specs (holes, slots, webs, bends, flanges, size) with each rule's source, and writes the part's Backplane fab file.
---

# SendCutSend parts

Everything about SendCutSend's materials, limits and services comes from three files it publishes for agents. Read
them through the helper, never from memory and never from a copy in a project: they change (stock, new materials, new
limits), and the helper says how old its copy is.

```sh
SCS="<this SKILL.md's folder>/scripts/scs.py"       # Backplane installs it in <Backplane home>/bundled/skills/sendcutsend/
python3 -I "$SCS" fetch                              # fetch or revalidate the three files; prints each copy's age
python3 -I "$SCS" guide                              # the ordering guide: read it once per task
python3 -I "$SCS" materials --q 5052 --service bending   # or --group "Wood and MDF", --category Acrylic, --thickness 3mm, --process waterjet
python3 -I "$SCS" material ALU-063                   # one SKU: catalog row + every spec (cutting, bending, tapping, ...), inches
python3 -I "$SCS" finishes --type powder             # anodizing | powder | plating
python3 -I "$SCS" hardware --q M3                    # PEM nuts, studs, standoffs: hole size, edge distance, sheet range
python3 -I "$SCS" dfm mech/bracket.step --flat mech/bracket.flat.dxf --sku ALU-063 --finish powder:matte_black --hardware F-M3-1
python3 -I "$SCS" fab mech/bracket.step --flat mech/bracket.flat.dxf --sku ALU-063 --finish powder:matte_black --hardware F-M3-1 --qty 4 --name "L bracket"
python3 -I "$SCS" dfm mech/bracket.fab.json         # re-check a part from its fab file alone
```

The cache is `~/.cache/backplane/sendcutsend` (`SCS_CACHE`); a copy younger than `SCS_TTL` seconds (6 h) is used as
is, an older one is revalidated with its ETag or Last-Modified. Offline (or `SCS_OFFLINE=1`) the cached copy is used and
every report's first lines say `OFFLINE` and its age: tell the person when you relied on an old copy. With no copy at
all the helper stops; say so rather than guessing a limit. Space your own requests to sendcutsend.com by a second, and
never scrape its pages: the three files are what it asks agents to use.

## How to work

1. **Material first.** Ask what the part is for, then pick group → category → grade → thickness with `materials`
   (`out_of_stock` ones cannot be ordered; `available_services` says what can be done to it, `cutting_process` how it
   is cut: fiber laser, CO2 laser, waterjet or CNC router). Name the SKU in the part script's parameters. Never build a
   SKU from a pattern: look it up.
2. **Model it in FreeCAD** (`mech/<part>.py`, Backplane's mechanical rules). Read the SKU's limits from `material`
   into the script's parameters (thickness, inside bend radius, K-factor, minimums) instead of typing numbers.
   `examples/bracket.py` is a bent bracket done this way: it builds the folded STEP and the flat-pattern DXF with its
   bend line from the same parameters. `scripts/scs_sheet.py` writes clean DXFs (`write_dxf`, `slot`, `rounded_rect`,
   `bend_allowance`, `flat_length`; inside FreeCAD `face_dxf(face, path)` exports a planar face).
3. **Check it:** `dfm`. Fix every FAIL in the model, not in the report; a WARN is the person's call (custom quote size,
   splines, part-in-part). Run it again after each change.
4. **Write the fab file:** `fab` (same options as `dfm`) writes `<part>.fab.json` beside the part with Backplane's kind,
   options and measured features, and a `sendcutsend` block (SKU, services, finish, hardware, the DFM result and the
   hash of the file it checked). Run it again whenever the part changes: Backplane flags a fab file whose part changed
   since its check, or whose check failed.
5. **Show it:** `viewer_show` the STEP (kind `mech`) and, for a bent part, say the flat size and the bends.

Ordering is the person's. Backplane orders through its Buy it button (the buy thread checks and quotes the fab file);
the person may also upload the STEP/DXF to sendcutsend.com themselves. Never upload to sendcutsend.com, fill its cart,
or open its checkout unless the person asks for exactly that in their message, and never handle payment.

## What a file must be (the guide's rules; numbers from the specs)

- **Formats:** STEP for anything bent (SendCutSend unfolds it itself), DXF for flat parts; also DWG, SVG, AI, EPS.
  One part per file. On upload the person confirms INCH or MM: set DXF `$INSUNITS` (1 inches, 4 mm).
- **Geometry:** closed contours only (one outline, holes and cutouts as closed loops); lines, arcs and polylines, no
  splines where you can avoid them; no text as a font (convert to outlines, or engrave/etch it as a finishing service);
  no blocks, dimensions, title blocks or construction lines on cut layers; no doubled lines (they cut twice).
- **Kerf:** draw the finished size. SendCutSend's tolerance (`cutting_specs.cut_tolerance`) is held on the drawn
  geometry; do not offset for the beam. Interlocking tabs and slots need `cutting_specs.tab_slot_tolerance` of play.
- **Minimums** (all in `cutting_specs`): `min_hole_size` (also the narrowest slot), `min_bridge_size` (the web between
  two cutouts), `min_hole_to_edge`; the part between `min_part_size` and `max_instant_pricing_part_size` (bigger, up to
  `max_custom_quote_part_size`, is a custom quote). Thickness tolerance and flatness are in `general_specs` /
  `cutting_specs`: design fits around them.
- **Part-in-part:** a shape inside a cutout falls out as a loose piece. Send it as its own part if it is wanted.
- **Waterjet and router** parts: router-cut materials have inside corners no sharper than the cutter (ask the person
  or keep inside radii generous) and larger `min_part_size`; check each SKU's own numbers.

## Bending

Only SKUs with `bending` in `available_services` (and `bending_specs` in the specs) bend.

- The **inside radius** is SendCutSend's tooling, `bending_specs.effective_bend_radius`; model the bend at it. The flat
  pattern follows from `k_factor` (bend allowance = angle·(r + K·t)) or `bend_deduction`; `scs_sheet.flat_length`
  does this. A STEP modelled at another radius forms to a different size: the check warns.
- **Flanges:** at least `min_flange_length_before_bend` in the flat from the bend line, and
  `min_flange_length_after_bend` formed (outside). Angles between `min_bend_angle` and `max_bend_angle` (no hems);
  bend length up to `max_bend_length`; Z-bends between `min_joggle` and `max_joggle`.
- **Holes near bends** distort: keep every hole and cutout at least the minimum flange from the bend line, or cut a
  relief through the bend. **Bend reliefs** at the ends of a bend that meets material: `bend_relief_depth` deep, at
  least the thickness wide.
- **DXF bend lines** sit alone on their own layer (any name; the helper reads layers matching `bend`, `--bend-layer`
  for others), run fully across what bends, and are drawn dashed in SolidWorks/AutoCAD, solid in Fusion/Illustrator.
  Give the angles with `--bend-angle` (default 90).
- Tolerances: `bend_angle_tolerance_up_to_24` / `_over_24`.

## Secondary operations and finishes

Each needs its name in the SKU's `available_services`, and the flat part within that service's
`min_flat_part_size`..`max_flat_part_size` (`<service>_specs`); the check does both.

- **Tapping** (`tapping_specs`): thread between `smallest_tap` and `largest_tap`; hole centre to edge at least
  `min_distance_cl_hole_to_edge`. Draw the tap drill hole; tell the check `--tap DIA:THREAD` (e.g. `3.3mm:M4x0.7`).
- **Countersinking** (`countersinking_specs`): minor ≥ `countersink_min_minor`, major ≤ `countersink_max_major`,
  centre to edge ≥ `min_hole_center_to_material_edge`. `--countersink DIA:MAJOR`.
- **Hardware insertion (PEM)**: draw the item's `hole_diameter` (catalog `hardware`), keep the centre
  `min_cl_to_edge_distance` from the edge, and the sheet within its `min/max_sheet_thickness`. `--hardware SKU`.
- **Dimple forming**: between `smallest_dimple` and `largest_dimple`.
- **Deburring, tumbling**: services, not finishes; size limits per SKU.
- **Finishes** (`finishes`): anodizing (aluminium only; Type II colours, Type III hard), powder coat (metals; note
  its minimum flat size), plating (zinc colours, nickel). `--finish TYPE:VALUE` with the catalog's `value`.
  Engraving and etching are finishing options too (`engraving_specs`).

## The fab file

`fab` maps what has a Backplane equivalent (order/fabsrv/catalog.py): metals become `kind: sheetmetal` (5052 →
`aluminum_5052`, 304 → `stainless_304`, mild/cold rolled → `steel_cold_rolled`, G90 → `steel_galvanized`; powder →
`powder_coat`, anodize Type II/III → `anodize`/`hard_anodize`) with `flat_x_mm`, `flat_y_mm`, `cut_length_mm`,
`holes`, `bends`, `countersinks` measured from the flat pattern; router and laser-cut plastics become `kind: cnc`
(acrylic → `pmma`, polycarbonate, ABS, Delrin → `pom_*`, G-10 → `fr4`) with volume, area and sizes. What has no
equivalent (MDF, plywood, rubber, 6061/7075 sheet, plating...) is left out of the options and listed in
`sendcutsend.unmapped`; Backplane then shows that the estimate is incomplete. Say so to the person. Never invent an
option to make the price appear.

For a bent STEP always pass `--flat` with the flat DXF: without it the flat size is estimated from the volume.

## A report (an example: your numbers come from the live specs)

```
SendCutSend DFM · bracket.dxf · ALU-063 5052 H32 Aluminum 0.063" (Fiber Laser)
sendcutsend-catalog.json: fetched 2026-11-09 16:24 UTC (12 min ago, cached), generated 2026-11-05 21:51Z
sendcutsend-specs.json: fetched 2026-11-09 16:24 UTC (12 min ago, cached), generated 2026-11-05 21:51Z
PASS  min hole               smallest hole Ø0.172" (4.37 mm) ≥ 0.014" (0.36 mm) (3 holes)
                             source: specs ALU-063 cutting_specs.min_hole_size
FAIL  bend 1 hole distance   closest hole edge 0.079" (2.00 mm) from the bend line < 0.254" (6.45 mm): ...
                             source: specs ALU-063 bending_specs.min_flange_length_before_bend (...)
...
25 pass · 1 fail · 0 warn · 0 not checked
```

Exit status 0 means no rule failed, 1 a rule failed, 2 the check could not run. `--json` gives the same rows.
Sources are `specs <SKU> <section.field>`, `catalog ...`, `ordering guide, <section>`, or `skill (<why>)` for the few
rules that are file hygiene rather than SendCutSend numbers. What the check cannot see (narrow necks in the outline
itself, inside corners of router parts, the exact shape of an L-shaped cutout) is yours to look at in the drawing.
