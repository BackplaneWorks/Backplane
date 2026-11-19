---
name: altium
description: Make and edit Altium Designer designs. Without Altium (any platform), the altium_write tool writes footprints (.PcbLib), symbols (.SchLib) and schematic sheets (.SchDoc). Live in the person's running Altium (Windows), altium_status and altium_script inspect and edit projects (.PrjPcb, .SchDoc, .PcbDoc, .SchLib, .PcbLib, .OutJob) - make symbols and footprints (also IPC-7351 ones with Altium's generator), new sheets and boards and the board outline, list components, nets and ERC messages, change parameters, place wires, labels, power ports and parts, update the board from the schematic (Altium's ECO, no person), move components, autoroute (whole board, nets, connections), pour polygons, add tracks and vias, check design rules, diff schematic against board, run OutJob outputs (Gerber, drill, ODB). Use whenever a task touches an Altium design; verified recipes are in recipes/.
---

# Altium Designer

## Read KiStack's skill for the job first

This skill covers only how to reach Altium (its files, its scripting, the tools below). How to design well is in KiStack's
KiCad skills, which every Backplane agent has: before the work, read the one that covers it completely (as a skill:
`kistack:<name>`; the `<kistack_skills>` block in your instructions gives their folder).

| Work | KiStack skill |
|---|---|
| a schematic sheet: placing, wiring, power and ground, labels, readability | `kicad-schematic` |
| a symbol | `kicad-symbol` |
| a footprint | `kicad-footprint` |
| board placement and routing | `layout-placement`, `kicad-layout`, `kicad-pcb` |
| BOM, outputs, Gerbers, panels, renders | `kicad-bom`, `kicad-export`, `kicad-gerbers`, `kicad-panelize`, `pcb-product-render` |

They are written for KiCad: keep their design rules and conventions (signal left to right, power symbols up and ground symbols
down rather than net labels for supplies, real wires between nearby pins and labels only for long or cross-sheet runs, standard
symbol shapes, spacing, reference designators, checks) and do the steps with Altium's means here instead of KiCad's tools
(kicad-cli, kicad_edit and kicad_ipc do not apply). Where a KiCad-only step has no Altium equivalent, skip it and say so.
When the sheet or board is drawn, look at it (`viewer_show` it, or read it back) and fix what KiStack's checks would flag.

## Writing files directly: altium_write (any platform, no Altium)

New footprints, new symbols, new sheets, and parts, wires, net labels and power ports added to a sheet: use
`altium_write`. It takes one JSON spec, writes Altium's own binary files, reads them back, and reports what they hold.
Read `write.md` before the first call: it has the spec, the package table and the rules. It needs no Altium, a spec cannot
leave a dialog up, and every part is checked when it is read back; but it is plain: `connect` puts a stub and a net label on a pin,
so for a sheet that reads well draw wires between nearby pins and use power ports for supplies and ground (`wires`,
`power` in the spec, or `connect` with `power`), as KiStack's `kicad-schematic` asks. A real Altium library part, where one is installed, beats a
template symbol.

Typical flow for a new part: footprint from the datasheet's package drawing (`pcblib`, a `package` or `custom` pads),
symbol with pins by side (`schlib`, `footprint` naming it), then place and `connect` it on a sheet (`schdoc`), and add the
files to the project (`project`), all in one call. On Windows with Altium running, compile with
`recipes/project_netlist.pas` afterwards to see Altium's own nets and ERC.

# Altium Designer, live

If Altium is not running, `altium_status` and `altium_script` start the newest installed Altium and wait until it is up (about a
minute); the result says so. Then open the project by path (`recipes/open_project.pas`). Never ask the person to open it.

`altium_script` runs a DelphiScript inside the person's running Altium (Windows only). Altium has their real work open:
every script addresses documents **by full path**, and never the focused/current document.

## Workflow

1. `altium_status` first: Altium's build, the open projects and their documents, the focused one. A project that is
   not open: `recipes/open_project.pas` opens it by path (it loads the sheets and board in the background, the
   person's view does not change).
2. Read before you write: `sch_list_components.pas`, `sch_list_objects.pas`, `project_netlist.pas`, `pcb_list.pas`,
   `pcb_list_area.pas`. Coordinates come out the way the edit recipes take them (schematic mils, board mm from the
   board origin).
3. Edit with the recipe for it: copy the file, change only its `const` block (placeholder paths, designators,
   coordinates), send it as `script`. Keep each script to one change: a short script fails clearly.
4. Verify by re-reading (the list recipes), not by trusting the edit's own message.
5. **Save each document right after a change that matters** (the ECO, placement, the outline, routing, a new part),
   before any other step: `SD.DoFileSave('')`, or `save_document.pas`. Until then the change lives only in Altium's
   memory, and a box Altium raises or a document that closes loses it (this happened: an ECO's parts were lost when the
   board was brought to the front unsaved). Backplane's viewers also read only the saved files.
6. Close only what you opened (`close_document.pas`), never the person's documents.

A new design, all by script (verified end to end): `new_documents` (sheet and board) -> `pcblib_footprint` /
`pcblib_ipc_batch` and `schlib_symbol` (libraries, `PrjPath` set so the project knows them) -> `sch_place_part` and wires
-> `project_netlist` (check the nets) -> `pcb_board_outline` -> `pcb_update_from_sch` -> `sch_pcb_diff` (differences 0)
-> `pcb_move_component` -> `pcb_autoroute` -> `pcb_polygon` -> `pcb_drc` -> `save_document` for each document.

## Recipes (recipes/*.pas, each verified on Altium Designer 26.11)

| Recipe | What |
|---|---|
| open_project | open a .PrjPcb by path; list documents (kind, path, loaded, modified) |
| sch_list_components | designator, comment, library ref + library, footprint, location, rotation, parameters |
| sch_list_objects | wires (vertices), net labels, power ports, ports, junctions |
| project_netlist | compile; nets with their pins; compiler/ERC messages with their objects |
| sch_edit_component | change comment and a parameter, move a component |
| sch_place_wire_label_power | wire between two points, net label, power port |
| sch_place_part | place a part from a .SchLib/.IntLib (`SchServer.LoadComponentFromLibrary`), with a unique id of its own |
| sch_delete | delete a component, net labels, power ports and wires (collected first, then removed) |
| save_document / close_document | save or close by path |
| pcb_list | components (designator, footprint, x/y mm, rotation, layer); nets with their pads |
| pcb_list_area | tracks, vias, arcs, pads, components inside a rectangle |
| pcb_move_component | move and rotate a component |
| pcb_add_track_via | track on a layer with a net, via |
| pcb_delete | delete objects of chosen kinds in an area (optionally one net) |
| pcb_drc | rule check from the script (clearance, short circuit, width, hole size, hole to hole); no dialog |
| pcb_violations | read the violation markers Altium's own DRC left on the board |
| sch_pcb_diff | what Update PCB would change (components, footprints, pin nets), read-only |
| outjob_run | run an OutJob's Folder Structure container (Gerber, NC drill, ODB...) headlessly |
| new_documents | new empty .SchDoc / .PcbDoc, saved and added to the project |
| schlib_symbol | make a symbol in a .SchLib (made if missing): body, pins (number, name, type, side), parameters, footprint link |
| schlib_list | a .SchLib's symbols: pins (number, name, type, location), parameters, models |
| pcblib_footprint | make a footprint in a .PcbLib (made if missing): SMD/TH pads, silk, assembly, courtyard, 3D body (extruded block; its STEP option is untested) |
| pcblib_ipc_batch | IPC-7351 footprints from datasheet dimensions with Altium's IPC batch generator (columns: ipc_batch_columns.txt) |
| pcblib_list | a .PcbLib's footprints: pads (mm from the footprint origin), lines per layer, 3D bodies |
| libraries_find | the libraries Altium offers the project; symbols in its .SchLibs matching a word |
| pcb_board_outline | board shape: a rectangle (optional chamfers) from the origin, also drawn on a mechanical layer |
| pcb_update_from_sch | Update PCB (Altium's ECO): the hub works its dialogs; parts and nets onto the board |
| pcb_from_schematic | the same without the ECO: footprints from named .PcbLibs placed, linked and given their pad nets |
| pcb_autoroute | Situs: whole board, connections between / on named components, only named nets; unrouted count back |
| pcb_polygon | polygon pour on a net over a rectangle, poured; or repour every polygon |

## Rules that bite (all hit while verifying; details in reference.md)

- **Never ask the person to run Update PCB (Design > Update PCB Document) or any other step a recipe does.** Run
  `pcb_update_from_sch.pas` (the hub works the ECO's dialogs), or `pcb_from_schematic.pas` when the ECO cannot run;
  both save the board. If one fails, read the error, fix the script, and run it again. Do not hand the step to the
  person. If work was lost, redo it the same way.
- **A 3D body on a placed board part is there only if the board counts it.** A body added with `C.AddPCBObject(Body)`
  (even with `PCBM_BoardRegisteration` sent) sits in the component's group, but a board iterator over
  `eComponentBodyObject` does not see it and the save leaves it out: 11 parts each showed a body through their group
  iterators, the saved .PcbDoc had none, and Backplane's 3D view drew no parts. After adding bodies, save, then count
  them with a board iterator (`B.BoardIterator_Create`, `eComponentBodyObject`, `AllLayers`); say the models are on the
  board only when that count matches. Calling `B.AddPCBObject(Body)` as well is the likely fix (not yet verified).

- **Altium's own dialogs: `BpExpect(Title, Steps)`.** A process that opens a modal window (the ECO, Situs's strategy
  dialog, the IPC batch generator) blocks the script until the window closes. Call `BpExpect` just before it and the hub
  works the window: `click <caption>` (trailing `*` = caption starts with), `wait <s>`, `text <n> = <value>` (n-th edit
  box), `window <title>` (go on in a window it opened, e.g. a file box), `gone <s>`. `BpMayExpect` for a window that comes
  only sometimes. Write titles and captions in English: the hub also matches Altium's own translations of them, and finds
  Windows' file box (`window Open`, `click Open`/`Save`) in any language. What it did comes back as `driven`; a step it
  cannot do closes that window (as Cancel), lists what the window offered, and makes the run not ok (so does an expected
  window that never came). Without `BpExpect` the hub leaves the window for the person and the run waits out its timeout.
- **Numbers: `BpNum(S)` and `BpFmt(Fmt, X)`, never `StrToFloat`/`FloatToStr`/`FormatFloat` on text.** On a machine whose
  Windows uses a decimal comma (German, French, ...), `StrToFloat('1.5')` fails and `FloatToStr(1.5)` gives `1,5`. `BpNum`
  reads a decimal point and `BpFmt` writes one, whatever the locale. Read and check every number before the first edit, so a
  bad one stops the script with nothing changed.
- **`BpFail(Msg)`** ends the run not ok with your message (the lines emitted are kept): use it for a partial result or a check
  that failed, never `Emit` alone.
- **`var` before `const`.** The hub turns your `var` section into globals; a `const` section placed before it makes the
  variables "typed constants" and the script does not compile. Every recipe has `var` first, then `const`.
- **Never send a script that does not compile.** A compile error leaves Altium with an Error box the hub does not close,
  and every later run then times out. Do not name a routine after a reserved word (`Register` is one). Calling a method
  on `nil` is a run-time error that reads "Undeclared identifier: <method>".
- **Save with `SD.DoFileSave('')`.** `DoFileSave(SD.Kind)`, `DoSafeChangeFileNameAndSave` and the
  `WorkspaceManager:SaveObject` process all return success and write nothing on a document opened in the background.
  Check the file's time after saving if it matters.
- **Board edits: the board must be the current one.** `SendMessageToRobots(..., PCBM_BoardRegisteration, ...)` registers
  the change with the undo of whichever board is in front, even when the object is added to another board: the
  person's board gets your undo entries and turns modified. The PCB edit recipes bring their board to the front
  (`Client.ShowDocument`) first and stop if that fails. Schematic edits address their own document and need no focus.
- **Place, then move.** A component from `LoadComponentFromLibrary` must be registered on the sheet before it is moved
  (`MoveToXY`); setting `Location` before leaves its pins and graphics at 0,0.
- **Collect, then delete.** Removing objects while iterating skips some.
- **A part placed from a library brings the symbol's UniqueId**, and the compiler merges parts that share one:
  `sch_place_part.pas` gives each placed part a fresh one (set inside `SCHM_BeginModify`/`EndModify`, or it is not kept).
- **A pad's net set by script is lost on save unless the net lists the pad:** `Pad.Net := Net; Net.AddPCBObject(Pad);`
  (new tracks and vias made with a net keep it).
- **Library footprints are stored around their own origin**; only the one shown in the library editor sits at the
  library's origin. Write pad coordinates as the footprint's own; read them as `P.x - FP.x` (`pcblib_list.pas`).
- **A new document has no file until renamed and saved** (`OpenNewDocument(Kind, '', Name, False)`, then
  `DoSafeChangeFileNameAndSave(Path, Kind)`); `DoFileSave` alone on it opens a Save As box.
- Moving a schematic component does not drag its wires: its pins leave the wires and the nets change (the diff and the
  netlist show it).
- Units: schematic coordinates in mils (`MilsToCoord`/`CoordToMils`); board in mm from the board origin
  (`Board.XOrigin + MMsToCoord(x)`, `CoordToMMs(c - Board.XOrigin)`).

## What needs the person

Only what no script can reach (below). Everything else, Update PCB included, is yours to do by script.

- **Online parts.** Manufacturer Part Search and workspace (Altium 365) components live in web panels with no script
  surface: a script cannot search them or place from them. Do it yourself where you can (find the part and its datasheet
  on the web; make the symbol with `schlib_symbol.pas` and the footprint with `pcblib_ipc_batch.pas` or
  `pcblib_footprint.pas`, MPN and manufacturer as parameters). Ask the person only when they want Altium's own part:
  "In Altium, open Manufacturer Part Search (or the Components panel for your workspace), find <MPN>, right-click it and
  choose Download (or Place, then save the part to a library), and tell me the library file it went to." Then
  `libraries_find.pas` and `sch_place_part.pas` take it from there.
- **ActiveRoute.** It reads only a selection made with the mouse (a script's selection, even through Altium's query,
  gives "select some objects first", and its WPF box stops the run until the hub closes it). Use `pcb_autoroute.pas`
  (Situs) for nets and connections.
- **Altium's batch DRC.** `PCB:DesignRuleCheck` opens the modal Design Rule Checker. Use `pcb_drc.pas`; when the full
  rule set matters (un-routed nets, silk, mask...), say that Tools > Design Rule Check covers the rest and read its
  markers with `pcb_violations.pas` if the person runs it; do not stop the work for it.
- **PDF output.** The OutJob PDF container (`WorkspaceManager:Print`, `Action=PublishToPDF|ObjectKind=OutputBatch|
  DisableDialog=True`) raised an Error box and wrote no PDF in testing; Gerber/drill/ODB through `outjob_run.pas` work.

## How results look

A result has `ok`, `output` (your `Emit` lines), `error`, `dialogs` and, after `BpExpect`, `driven`. `ok: false` with an `error` is an exception in
your script or a `BpFail` (the lines emitted before it are kept). `dialogs` says what the hub did with Altium's boxes. It
answers only the script engine's own box (a compile or run error: Cancel, which stops the script; "did not compile" means
nothing of it ran) and a box with a single button that came up while your script ran (`=> pressed OK`: it decided nothing).
Every other box is the person's to answer and is named as `left for the person: "<title>" [buttons]`; the hub never presses
a button that decides something and never closes a window it did not open. A run that times out after its script started
blocks the next ones until it ends (the next call says what became of it): do not retry in a loop; look at what it was
doing. When a box ended the run ("Altium ended the script"), first check that every document you were editing is still open
and save it; if one closed, redo its change by script. Do not retry blindly. Loops stop at the deadline
(`BP-LOOP-GUARD` in the output): narrow the work instead of raising the timeout.
