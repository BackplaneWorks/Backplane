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

## Writing files (any platform, no Altium)

`altium/write/` writes Altium's binary files itself, so agents make footprints, symbols and sheets with no Altium running
(the MCP tool `altium_write`, one JSON spec; `deploy/skills/altium/write.md` is the agents' reference).

- `mod.rs`: a compound file read into streams and written back (`cfb`, version 3: 512-byte sectors, as Altium writes);
  `|KEY=VALUE|` records in Altium's spelling, coordinates in 10-mil units and `_Frac`, Windows-1252 with `%UTF8%` twins,
  storage names cut to 31 characters with the full name in `SectionKeys`, unique ids from a seed.
- `pcblib.rs`: footprints (pads with rounded-rectangle corners and slots, tracks, arcs, extruded 3D bodies) as binary
  primitive records; the library's `Library/Data` (a master layer stack is required: without it Altium reports
  "Catastrophic failure" loading the Library section), `ComponentParamsTOC`, `LayerKindMapping`, `PadViaLibrary`, the
  model storages. A footprint of the same name is replaced; the others are kept byte for byte. `read` turns each
  footprint's primitives into the board reader's streams, so the round trip is checked by the same code that draws boards.
- `fp.rs`: footprints from package dimensions with IPC-7351B nominal lands (chip, gull-wing SOIC/SOP/TSSOP/MSOP/SOT-23,
  QFN/DFN, pin headers, custom pads), silk clear of copper, assembly outline, courtyard and 3D body, IPC names.
- `schlib.rs`: symbols (pins as Altium's binary pin records with `PinFrac` for fractions, graphics, parameters, the
  footprint links 44/45/46/48) and the library header's symbol list; pins by side laid out in a box sized to their names.
- `schdoc.rs`: a sheet made or edited: a library symbol placed with its records turned, mirrored and moved (a sheet stores
  them placed; pins become text records), parts of a multi-part symbol under one designator, wires, net labels, power
  ports, junctions, no-ERC marks, parameters, a pin hooked to a net (`connect`), parts and objects removed with owner
  indices closed up.
- `spec.rs`: the JSON spec (`pcblib`, `schlib`, `schdoc`, `project`), paths kept inside the project folder, the result
  read back through `sch.rs` and the board reader.

Altium's own behaviour with files changed under it (Altium 26.11): an open document with no unsaved edits was reloaded
silently; one with unsaved edits raises a modal "has been modified ... Reload?" box; an open project ignores a changed
.PrjPcb (and would overwrite it when saved); `IServerDocument.DoFileLoad` appends the file to the open document (every
object twice). So the tool (`live::before_write`/`after_write`, Windows with Altium running) refuses a target open with
unsaved edits, closes the other open targets before writing and reopens them after (the focused document last), and adds
new documents to an open project through Altium (`DM_AddSourceDocument`).

Formats and sources: Altium's own files; AltiumSharp (OriginalCircuit.Altium, Apache-2.0), whose writers the pad, track,
arc and body records are ported from; KiCad's importer and python-altium's format notes as references.

## Editing (Windows, with Altium running)

Altium has no out-of-process API. `altium/live.rs` runs a DelphiScript inside the person's running Altium
(`X2.EXE -RScriptingSystem:RunScriptFile`), one file per run, and reads back what it wrote. The agent's script is
wrapped: its `var` block global, its routines after the helpers, its statements in try/except (an error is a result, not
Altium's blocking "Continue execution?" box), `Emit(S)` lines written through to disk, every `while`/`repeat` loop cut at
the run's deadline (a script cannot be stopped from outside), undo groups a failed script left open closed, and any box
Altium still raises answered. Runs go in `%PUBLIC%\Backplane\altium\runs`. With no Altium running, a run starts the newest installed one first and waits for its main window.

- Dialogs a script means to open: `BpExpect(Title, Steps)` before the call that opens one (the ECO's Engineering Change
  Order, Situs's strategy dialog, the IPC footprint batch generator) tells the hub to work that window instead of
  cancelling it: steps `click <caption>` (a trailing `*` matches the caption's start), `wait <s>`, `text <n> = <value>`
  (the n-th edit box), `window <title>` (go on in another window it opened), `gone <s>`. The lines go to the run's own
  `.exp` file, which the hub reads only while a new Altium window is up (the script is then waiting on it; reading the
  output file while Emit rewrites it made Altium's save fail). The hub finds the new window whose title contains
  `Title` and clicks as the mouse does (posted button-down/up: Altium's buttons are drawn controls). What it did comes
  back as `driven`; a step it cannot do (no such enabled control within 30 s) closes the window and those it opened, as
  Cancel would, and says what the window offered.
- Boxes it answers unasked: Windows message boxes, Delphi message forms, and owned WPF windows (Altium's extensions,
  e.g. ActiveRoute's "Information"), which have no controls to read and are closed as Alt+F4 would.
  Which button (`pick_answer`): never one that throws work away (Don't Save, Save None, Discard, Reload, Revert); on
  a box about saving or losing changes, Cancel, else the answer that keeps the document (Yes to save, No to discard or
  reload); otherwise Cancel, No, OK, Close. `dialogs` says what was pressed (`<box> => pressed <button>`). Altium's own
  message forms draw their text as a label that cannot be read, so often only the title is known. A run the hub had to
  stop tells the agent to check and save the documents it was editing before anything else. Seen 2026-11-18: a one-button
  "Information" box came up while a placement script brought an unsaved board (fresh ECO parts) to the front. The hub
  pressed OK and stopped the script, and the board had closed with its unsaved parts. Hence the rule in the skill and
  the recipes: save right after the ECO and after placement.

- MCP tools: `altium_status` (Altium's build, open projects and documents, the focused one) and `altium_script`.
- The agents of an Altium project get `<backplane_altium>` instead of KiCad's editing and runtime blocks
  (`agents/prompt.rs` `altium`), the bundled `altium` skill (`deploy/skills/altium`): recipes tested against Altium, and
  KiStack's skills with a note that they are written for KiCad and are to be adapted at the agent's discretion (the
  KiCad-only tools kicad_edit, kicad_ipc and kicad-cli stay out).
- The viewer bar offers Open in Altium (the hub marks the viewer `cad: altium`; `kicad/open.rs` opens what is on show in
  the running Altium, else starts the newest installed one with it).
- Off Windows the tools say Altium runs on Windows; the viewers still draw the files.

## Testing

- `cargo test altium` runs the unit tests (synthetic records only: the repo carries no Altium design files).
- `BP_ALTIUM_WRITE_OUT=<dir> cargo test altium::write::spec::tests::demo_project` writes a demo project (every package,
  template and sheet feature, with SVGs drawn by the viewers' readers) to open in Altium; `BP_ALTIUM_SPEC=<spec.json>`
  with `BP_ALTIUM_WRITE_OUT` runs any spec there (`spec_file`).
- `BP_ALTIUM_CORPUS=<folder of real projects>` adds corpus tests (every sheet and board parses; BOM; 3D; with `BP_ALTIUM_OUT=<dir>`
  they write debug SVGs). `BP_KICAD_CLI=<kicad-cli>` adds the 3D comparison with KiCad's importer (every embedded model the same
  file in the same place); `BP_ALTIUM_3D_OUT=<dir>` draws both to PNGs (parts meshed with FreeCAD or `BACKPLANE_STEP2GLB`).
- On Windows: `BP_ALTIUM_LIVE=1` runs `live_altium` against the running Altium (status, an error, a runaway loop, a
  compile error, Altium still answering); `BP_ALTIUM_SCRIPT=<file>` runs one script through the bridge and prints the result.
  The test binary must run in the desktop session (Altium's command forwarding does not cross into an ssh session).
- Cross-building for Windows from Linux: `docs/windows.md` (cargo-xwin).

## Not yet

- Altium's ERC/DRC in the Review tab, Gerber/BOM/PDF output through Output Jobs, Buy it for Altium boards.
- ActiveRoute from a script (it sees only a selection made with the mouse); online parts (Manufacturer Part Search,
  workspace components), which have no script surface.
- Image contents on sheets (drawn as frames), more than six inner layers apart, rule-based mask/paste expansion.
- 3D: cylinder and sphere bodies, the stack's own colours, a body's 3D colour on a STEP model.
- Writing a .PcbDoc (boards are edited through Altium); footprint text and full pad stacks in written footprints.
