# Altium Designer

Backplane opens Altium Designer projects next to KiCad ones: the viewers draw them, the BOM and history work on them, and agents
edit them through the person's running Altium.

## Which projects

A folder with no KiCad project and a `.PrjPcb` is an Altium project (`kicad/project.rs`: a `.PrjPcb` at the top, else the
shallowest; `.backplane.json` `project` may name one). Its board is the first `.PcbDoc` the project lists; its top sheet is the
first `.SchDoc` no other sheet places (`altium/prj.rs` `roots`). When a folder has both, KiCad's project wins. `.PcbDoc`, `.SchDoc`,
`.PrjPcb`, `.SchLib`, `.PcbLib`, `.IntLib` and `.OutJob` are design files (committed to main with each turn, one writer per document,
kept in history steps); Altium's own `History/` and `__Previews/` folders are not.

## Drawing (any platform)

The hub reads the files itself, in `native/host/src/altium/`, into the same items its KiCad readers make, so the Board, Schematic,
Split and History views, cross-probing, the phones and comparisons need nothing new.

- `mod.rs`: OLE compound files (the `cfb` crate), `|KEY=VALUE|` records, binary record cursors, units.
- `sch.rs`: a `.SchDoc` as schematic items: components (the current part of a multi-part one, its display mode), pins, wires,
  buses, junctions, net labels, power ports, ports, sheet symbols and entries, notes and text frames, harnesses, the page
  and title block; nets named as Altium names them. `sheets`, `sheet_boxes`, `parts`, `nets`.
- `pcb.rs`: a `.PcbDoc` as board items: outline, tracks, arcs, vias, pads (shapes, holes, slots), fills, regions and
  poured copper with its clearances, text, cutouts and keepouts; the layer stack's copper on In1..In6 in stack order
  (more inner layers share the six), mechanical layers by kind. `thickness_um`, `components`, `nets`, `layers`.
- `plotgen::of_bytes` sends an OLE file to these readers; the sheet tree, sheet boxes and reference lists read Altium sheets too.
  A sheet symbol may name its file without `.SchDoc`. A flat design's sheets (none placed by a sheet symbol) are reached
  from the top sheet, so the Sheets menu, the BOM and the names list see every sheet.
- 3D (`board3d.rs`, no KiCad): the .PcbDoc becomes the native 3D path's board model (`kicad/pcb3d`, which sends a compound file
  here): the outline and cutouts, the stack's thickness, outer copper, pours, pads, vias, holes, mask openings and silk from the
  2D reader's items and pad/via records. Parts are the components, placed as KiCad's importer places them; each body is a model:
  an embedded STEP (`Models/N`, zlib) written once into the models cache by content and placed with its body's transform
  (position, Z offset, X/Y/Z turns, projection side), an extruded body as a prism (a VRML file in the part's frame, one per
  shape), a linked STEP by its name beside the board (a box and a note when it is not there). They mesh and stream as a KiCad
  board's parts do.
- BOM: Altium's parameters (`Manufacturer Part Number`, `Manufacturer`, `DNP`); a sheet placed by several sheet symbols
  counts once per placement, its parts named `$Component_$RoomName` (`IC1_CAN1`), as Altium names them on the board.

## Editing (Windows, with Altium running)

Altium has no out-of-process API. `altium/live.rs` runs a DelphiScript inside the person's running Altium
(`X2.EXE -RScriptingSystem:RunScriptFile`), one file per run, and reads back what it wrote. The agent's script is
wrapped: its `var` block global, its routines after the helpers, its statements in try/except (an error is a result, not
Altium's blocking "Continue execution?" box), `Emit(S)` lines written through to disk, every `while`/`repeat` loop cut at
the run's deadline (a script cannot be stopped from outside), undo groups a failed script left open closed, and any box
Altium still raises answered. Runs go in `%PUBLIC%\Backplane\altium\runs`.

- MCP tools: `altium_status` (Altium's build, open projects and documents, the focused one) and `altium_script`.
- The agents of an Altium project get `<backplane_altium>` instead of KiCad's editing and runtime blocks
  (`agents/prompt.rs` `altium`), and the bundled `altium` skill (`deploy/skills/altium`): recipes tested against Altium.
- The viewer bar offers Open in Altium (the hub marks the viewer `cad: altium`; `kicad/open.rs` opens what is on show in
  the running Altium, else starts the newest installed one with it).
- Off Windows the tools say Altium runs on Windows; the viewers still draw the files.

## Testing

- `cargo test altium` runs the unit tests (synthetic records only: the repo carries no Altium design files).
- `BP_ALTIUM_CORPUS=<folder of real projects>` adds corpus tests (every sheet and board parses; BOM; 3D; with `BP_ALTIUM_OUT=<dir>`
  they write debug SVGs). `BP_KICAD_CLI=<kicad-cli>` adds the 3D comparison with KiCad's importer (every embedded model the same
  file in the same place); `BP_ALTIUM_3D_OUT=<dir>` draws both to PNGs (parts meshed with FreeCAD or `BACKPLANE_STEP2GLB`).
- On Windows: `BP_ALTIUM_LIVE=1` runs `live_altium` against the running Altium (status, an error, a runaway loop, a
  compile error, Altium still answering); `BP_ALTIUM_SCRIPT=<file>` runs one script through the bridge and prints the result.
  The test binary must run in the desktop session (Altium's command forwarding does not cross into an ssh session).
- Cross-building for Windows from Linux: `docs/windows.md` (cargo-xwin).

## Not yet

- Altium's ERC/DRC in the Review tab, Gerber/BOM/PDF output through Output Jobs, Buy it for Altium boards.
- Image contents on sheets (drawn as frames), more than six inner layers apart, rule-based mask/paste expansion.
- 3D: cylinder and sphere bodies, the stack's own colours, a body's 3D colour on a STEP model.
- Editing without Altium (writing .SchDoc/.PcbDoc directly).
