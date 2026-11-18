---
name: altium
description: Inspect and edit Altium Designer projects (.PrjPcb, .SchDoc, .PcbDoc, .SchLib, .PcbLib, .OutJob) live in the person's running Altium through the altium_status and altium_script tools - make symbols and footprints (also IPC-7351 ones with Altium's generator), new sheets and boards and the board outline, list components, nets and ERC messages, change parameters, place wires, labels, power ports and parts, update the board from the schematic (Altium's ECO, no person), move components, autoroute (whole board, nets, connections), pour polygons, add tracks and vias, check design rules, diff schematic against board, run OutJob outputs (Gerber, drill, ODB). Use whenever a task touches an Altium design on Windows; verified recipes are in recipes/.
---

# Altium Designer, live

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
5. Save the edited documents with `save_document.pas`: Backplane's viewers read the files, so an unsaved edit is
   invisible there. Saving is the person's call when they are working in the same document; say what is unsaved.
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

- **Altium's own dialogs: `BpExpect(Title, Steps)`.** A process that opens a modal window (the ECO, Situs's strategy
  dialog, the IPC batch generator) blocks the script until the window closes. Call `BpExpect` just before it and the hub
  works the window: `click <caption>` (trailing `*` = caption starts with), `wait <s>`, `text <n> = <value>` (n-th edit
  box), `window <title>` (go on in a window it opened, e.g. a file box), `gone <s>`. What it did comes back as `driven`;
  a step it cannot do closes the window (as Cancel) and lists the buttons the window had. Without `BpExpect` the hub
  only answers message boxes, and the run waits out its timeout.
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
  rule set matters (un-routed nets, silk, mask...), ask the person to run Tools > Design Rule Check, then read the
  markers with `pcb_violations.pas`.
- **PDF output.** The OutJob PDF container (`WorkspaceManager:Print`, `Action=PublishToPDF|ObjectKind=OutputBatch|
  DisableDialog=True`) raised an Error box and wrote no PDF in testing; Gerber/drill/ODB through `outjob_run.pas` work.

## How results look

A result has `ok`, `output` (your `Emit` lines), `error`, `dialogs` and, after `BpExpect`, `driven`. `ok: false` with an `error` is an exception in
your script (the lines emitted before it are kept). Anything in `dialogs` is a box Altium raised and the hub closed:
look at the screen state with `altium_status` and do not retry blindly. Loops stop at the deadline
(`BP-LOOP-GUARD` in the output): narrow the work instead of raising the timeout.
