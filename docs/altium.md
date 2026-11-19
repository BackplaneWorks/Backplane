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
  board's parts do. A board saved with no body on any part says so in the view (a body Altium holds only in the component's
  group is not saved: see the skill's rule).
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
(`X2.EXE -RScriptingSystem:RunScriptFile(FileName=<file>|ProcName=Run)`, one file per run), and reads back what it wrote.
The agent's script is wrapped: its `var` block global, its routines after the helpers, its statements in try/except (an error
is a result, not Altium's blocking "Continue execution?" box), `Emit(S)` lines written through to disk, every `while`/`repeat`
loop cut at the run's deadline (a script cannot be stopped from outside), undo groups a failed script left open closed.
Runs go in `%LOCALAPPDATA%\Backplane\altium\runs`. With no Altium running, a run starts the newest installed one first and
waits for its main window.

- One run at a time: status, scripts, the write tool's close/write/reopen and Open in Altium hold one session (a mutex in the
  hub, a lock file in the runs folder across hubs). A run that timed out after its script had started is pending: no other
  script is sent until it reports back, its Altium is gone, or Altium is idle again (answering, enabled, the run's windows
  closed); the next call says what became of it. A run that timed out before its script started leaves `<run>.cancel`: if
  Altium starts it later, it does nothing. With a modal window already up (the person's), nothing is sent and the call says so.
- The run file is ASCII: Altium reads a script in the ANSI code page whatever its byte-order mark (a UTF-8 `'Ü'` came back
  as two characters, 26.11), so characters past ASCII in string literals become character codes (`'a'#220'b'`, a UTF-16
  pair past the BMP). The run's own files are ASCII too (`BpEsc`: `\\`, `\uXXXX`), because `TStringList.SaveToFile`
  writes the ANSI code page and DelphiScript has no `TEncoding`; the hub turns them back. Tested: `Ünïcode µ Ω 中` both ways.
- Numbers: a comma-decimal locale (German, French...) makes `StrToFloat('1.5')` fail (tested with Windows' decimal
  separator set to `,`); scripts use `BpNum(S)` and `BpFmt(Fmt, X)`, which read and write a decimal point whatever the locale.
- `BpFail(Msg)` ends the run not ok with that message, the lines emitted kept (a partial result, a failed check).
- Dialogs a script means to open: `BpExpect(Title, Steps)` before the call that opens one (the ECO's Engineering Change
  Order, Situs's strategy dialog, the IPC footprint batch generator) tells the hub to work that window: steps
  `click <caption>` (a trailing `*` matches the caption's start), `wait <s>`, `text <n> = <value>` (the n-th edit box),
  `window <title>` (go on in another window it opened), `gone <s>`. `BpMayExpect`: a window that comes only sometimes
  (Component Links). The hub works only a new window of Altium's, owned (a dialog), whose title matches, and checks it
  again before every step; it clicks as the mouse does (posted button-down/up: Altium's buttons are drawn controls). A
  box with a single button that the window raises while it is worked (the ECO's "Information: Component not found in
  available libraries") is pressed. What it did comes back as `driven`. A step it cannot do (no such enabled control within
  30 s, the window gone or blocked) closes that window and the windows it owns, as Cancel would, and the run is not ok;
  so is an expected window that never came.
- Titles and captions in another language: Altium ships translations of its forms (`System\Resources\<language>\forms.fdb`:
  German, French, Japanese, Simplified and Traditional Chinese, Korean, Russian in 26.11), and with "Use localized resources"
  its dialogs show them ("Änderungen bestätigen (&V)" for Validate Changes, "Änderungsaufträge (ECO)" for the ECO). The hub
  reads the installed Altium's files and matches a script's English words or any of their translations (`fdb_captions`).
  Windows' own file box (Open, Save As) is found by its controls (the file-name box, IDOK), in any Windows language.
- Boxes the hub answers unasked, and only these: the script engine's own box (a Windows box titled with the program's name,
  "X2", with Cancel and the error's line and column: Cancel, which stops the script; then the engine's one-button "Error"
  box after a compile error), a box of the X2.EXE the hub started to hand the script over, and a box with a single button that
  came up while the script was running (Altium's UI thread was running the script, so the person could not have opened it;
  one button decides nothing). Everything else is left for the person and named in `dialogs` (`left for the person: "<title>"
  [buttons]`): a box with a choice, a WPF window (Altium's extensions: "Unsaved Changes", ActiveRoute's "Information"), any
  window of the person's. The hub never closes a window it did not open. When a pressed box ended the script (an access
  violation inside Altium ends a script outright, its `finally` too), the run says so as soon as Altium is idle again. A
  script that did not compile says that nothing of it ran.
- Which Altium: the X2.EXE in the hub's own Windows session that has its main window (a second X2.EXE that only hands a
  command over has none). In another session only (another user, Remote Desktop, a service): refused with a message. Run as
  administrator while the hub is not: refused with a message (Windows' UIPI drops a lower-integrity program's clicks and
  may drop the command; the hub compares the processes' integrity levels). Installs: `BACKPLANE_ALTIUM_EXE` if set, else
  the newest version Altium registered (`HKLM\SOFTWARE\Altium\Builds\*`, `ProgramsInstallPath`, `Version`: the folder name
  is not the version, 26.11 installs into `AD23` when updated in place), else the folders under `Program Files\Altium`.
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
- `BP_ALTIUM_LIVE=2` runs `live_left_box` (a box no one expected: left for the person, the run pending, the next refused until
  it is gone); `BP_ALTIUM_IL_PID=<pid>` runs `live_integrity` (integrity levels as the hub reads them, the Altium it picks, the
  install it would start, the translations it reads).
- Cross-building for Windows from Linux: `docs/windows.md` (cargo-xwin).

## Where it works

Tested on Windows 10 22H2 (19045) with Altium Designer 26.11.1 (2026-11-19, a VM); the rest is from Windows' and Altium's
documentation and reasoning, not tested.

| | Status |
|---|---|
| Windows 10 and 11, x64 | The Win32 calls used (Toolhelp, EnumWindows, posted mouse messages, WM_COMMAND, tokens) are the same on both; tested on 10. Altium supports 11, and 10 "not recommended" |
| Altium versions | `RunScriptFile` with `FileName|ProcName` is documented (Running Scripts); the `-R` command line is not: tested on 26.11, seen in Altium's own AD19 CI article (`RunScript`). The program is X2.EXE (DXP.EXE in old releases: not supported) |
| Both programs unelevated (a normal install) | tested: status, scripts, errors, loops, two runs at once, the ECO |
| Backplane elevated, Altium not | tested: works (a Backplane run as administrator starts Altium as administrator too) |
| Altium elevated, Backplane not | refused with a message before anything is sent (integrity levels compared; the read of an elevated process's level tested) |
| Altium in another Windows session (other user, Remote Desktop, a service, ssh) | refused with a message; a hub started over ssh cannot reach the desktop's Altium |
| Several Altium instances | the first with a main window in the hub's session; which instance Altium hands a `-R` command to is Altium's choice (not tested) |
| Paths with spaces, any script (`Ünïcode`, `Ωμέγα 中文`) | tested, for the run file (one quoted argument) and for paths inside scripts (character codes) |
| User names with spaces or past ASCII | the runs folder is under `%LOCALAPPDATA%`: the same two mechanisms; not tested with a real account |
| Text past ASCII in scripts and their output | tested both ways (`Ünïcode µ Ω 中`) |
| Comma-decimal locales | tested (Windows' decimal separator set to `,`): `StrToFloat('1.5')` fails, `BpNum`/`BpFmt` work |
| Localized Altium UI (7 languages) | its translations read from the install (tested: all 7 load); not tested with a localized UI running |
| Windows in another language | the script engine's box and Windows' file box found by controls and numbers, not words; not tested |
| Display scaling | clicks go to a control's own client centre, from a per-monitor-aware thread; not tested at 150 % |
| A dialog the person has open | tested: before a run, nothing is sent and the call says so; during a run, it is left and named |
| Antivirus, AppLocker, Smart App Control | AppLocker's script rules do not cover `.pas`; Altium does not support Windows 11 Smart App Control; endpoint tools may block X2.EXE (Altium's own knowledge base); not tested |

Known limits: the text of Altium's own message boxes (drawn labels) cannot be read, only their title and buttons; a WPF box
(Altium's extensions) cannot be worked; a box with a choice that comes up during a run waits for the person; an Altium
Workspace (365) project must be checked out to a local folder for Backplane to open it, and a folder with both a KiCad and an
Altium project opens the KiCad one unless `.backplane.json` names the `.PrjPcb`; a viewer licence or an Altium with no
scripting server makes the script never start (the run says "did not start"); a footprint's mechanical layers in
`pcblib_footprint.pas` and the DRC recipe's rule kinds and search radius are fixed values to check against the project.

## Not yet

- Altium's ERC/DRC in the Review tab, Gerber/BOM/PDF output through Output Jobs, Buy it for Altium boards.
- ActiveRoute from a script (it sees only a selection made with the mouse); online parts (Manufacturer Part Search,
  workspace components), which have no script surface.
- Image contents on sheets (drawn as frames), more than six inner layers apart, rule-based mask/paste expansion.
- 3D: cylinder and sphere bodies, the stack's own colours, a body's 3D colour on a STEP model.
- Writing a .PcbDoc (boards are edited through Altium); footprint text and full pad stacks in written footprints.
