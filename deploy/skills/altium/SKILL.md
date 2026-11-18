---
name: altium
description: Make Altium Designer footprints (.PcbLib), symbols (.SchLib) and schematic sheets (.SchDoc) directly with the altium_write tool (no Altium needed, any platform), and inspect and edit Altium projects (.PrjPcb, .SchDoc, .PcbDoc, .SchLib, .OutJob) live in the person's running Altium through the altium_status and altium_script tools - list components, nets and ERC messages, change parameters, place wires, labels, power ports and parts, move board components, add tracks and vias, check design rules, diff schematic against board, run OutJob outputs (Gerber, drill, ODB). Use whenever a task touches an Altium design on Windows; verified recipes are in recipes/.
---

# Altium Designer

## Writing files directly: altium_write (any platform, no Altium)

New footprints, new symbols, new sheets, and parts, wires, net labels and power ports added to a sheet: use
`altium_write`. It takes one JSON spec, writes Altium's own binary files, reads them back, and reports what they hold.
Read `write.md` before the first call: it has the spec, the package table and the rules. Prefer it to scripting for
these jobs: it needs no Altium, a spec cannot leave a dialog up, and every part is checked when it is read back.

Typical flow for a new part: footprint from the datasheet's package drawing (`pcblib`, a `package` or `custom` pads),
symbol with pins by side (`schlib`, `footprint` naming it), then place and `connect` it on a sheet (`schdoc`), and add the
files to the project (`project`), all in one call. On Windows with Altium running, compile with
`recipes/project_netlist.pas` afterwards to see Altium's own nets and ERC.

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

## Recipes (recipes/*.pas, each verified on Altium Designer 26.11)

| Recipe | What |
|---|---|
| open_project | open a .PrjPcb by path; list documents (kind, path, loaded, modified) |
| sch_list_components | designator, comment, library ref + library, footprint, location, rotation, parameters |
| sch_list_objects | wires (vertices), net labels, power ports, ports, junctions |
| project_netlist | compile; nets with their pins; compiler/ERC messages with their objects |
| sch_edit_component | change comment and a parameter, move a component |
| sch_place_wire_label_power | wire between two points, net label, power port |
| sch_place_part | place a part from a .SchLib/.IntLib (`SchServer.LoadComponentFromLibrary`) |
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

## Rules that bite (all hit while verifying; details in reference.md)

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
- Moving a schematic component does not drag its wires: its pins leave the wires and the nets change (the diff and the
  netlist show it).
- Units: schematic coordinates in mils (`MilsToCoord`/`CoordToMils`); board in mm from the board origin
  (`Board.XOrigin + MMsToCoord(x)`, `CoordToMMs(c - Board.XOrigin)`).

## What needs the person

- **Update PCB / Import Changes (ECO).** `WorkspaceManager:Compare` (`ObjectKind=Project|Action=UpdateMe` with the
  board in front) opens Altium's modal "Component Links" / "Engineering Change Order" dialogs; the script waits until
  they are closed. Show the person `sch_pcb_diff.pas`'s list, then ask them to run Design > Update PCB Document
  (Validate, Execute) themselves.
- **Altium's batch DRC.** `PCB:DesignRuleCheck` opens the modal Design Rule Checker. Use `pcb_drc.pas`; when the full
  rule set matters (un-routed nets, silk, mask...), ask the person to run Tools > Design Rule Check, then read the
  markers with `pcb_violations.pas`.
- **PDF output.** The OutJob PDF container (`WorkspaceManager:Print`, `Action=PublishToPDF|ObjectKind=OutputBatch|
  DisableDialog=True`) raised an Error box and wrote no PDF in testing; Gerber/drill/ODB through `outjob_run.pas` work.

## How results look

A result has `ok`, `output` (your `Emit` lines), `error` and `dialogs`. `ok: false` with an `error` is an exception in
your script (the lines emitted before it are kept). Anything in `dialogs` is a box Altium raised and the hub closed:
look at the screen state with `altium_status` and do not retry blindly. Loops stop at the deadline
(`BP-LOOP-GUARD` in the output): narrow the work instead of raising the timeout.
