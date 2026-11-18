# altium_write: Altium files without Altium

`altium_write` makes and edits Altium Designer's binary files directly, on any platform, from one JSON object. Altium
does not need to be running. Verified on Altium Designer 26.11: the written libraries open with no message, and a
project made this way compiles with no ERC message.

Use it for:
- new footprints (.PcbLib) and symbols (.SchLib), or replacing one by name;
- new schematic sheets, and parts, wires, labels and power ports added to a sheet;
- adding the files to the .PrjPcb.

Everything else goes through `altium_script`: board edits, ERC/DRC, outputs, and changes to objects already on a sheet
(moving a part, editing a wire). The tool never writes a .PcbDoc.

## Shape of a call

The arguments are the spec. Its parts run in this order: `pcblib`, then `schlib`, then `schdoc`, then `project`. So one
call can make a footprint, the symbol that uses it, and a sheet that places it.

- **Paths** are relative to the project folder.
- **Schematic coordinates** are mils, Y up. Put pin ends, parts and wires on the 100 mil grid.
- **Footprint coordinates** are mm, Y up. Pin 1 is at the top left (IPC's zero orientation).
- **Replacing:** a symbol or footprint with the same name as one already in the library is replaced. Everything else in
  the file is kept byte for byte.
- **Result:** what each file now holds, read back through Backplane's own readers:
  - pads, with their size, position and copper extent;
  - each symbol's pins, with `end_mils`, where a wire connects;
  - the sheet's parts and nets.

  Check the result before you report done.

```json
{
  "pcblib": {"path": "lib/Power.PcbLib", "footprints": [{"package": "sot23", "pins": 5}]},
  "schlib": {"path": "lib/Power.SchLib", "symbols": [{
    "name": "TLV75533PDBV", "description": "500 mA LDO, 3.3 V", "designator": "U?", "footprint": "SOT95P280X145-5N",
    "parameters": {"Manufacturer": "Texas Instruments", "Manufacturer Part Number": "TLV75533PDBVR"},
    "pins": [
      {"number": 1, "name": "IN", "side": "left", "type": "power"},
      {"number": 3, "name": "EN", "side": "left", "type": "input"},
      {"number": 5, "name": "OUT", "side": "right", "type": "power"},
      {"number": 4, "name": "NC", "side": "right"},
      {"number": 2, "name": "GND", "side": "bottom", "type": "power"}]}]},
  "schdoc": {"path": "Power.SchDoc", "size": "A4",
    "place": [{"symbol": "TLV75533PDBV", "library": "lib/Power.SchLib", "designator": "U1", "at": [2000, 4000]}],
    "connect": [{"pin": "U1.IN", "net": "VBAT"}, {"pin": "U1.EN", "net": "VBAT"}, {"pin": "U1.GND", "net": "GND"},
                {"pin": "U1.OUT", "net": "3V3", "power": "bar"}, {"pin": "U1.4", "net": "NC"}]},
  "project": "Power.PrjPcb"
}
```

## pcblib

`{"path", "footprints": [...], "remove": [names]}`

Each footprint gets:
- IPC-7351B lands at nominal density;
- a silkscreen outline kept 0.2 mm clear of copper, and a pin 1 dot;
- an assembly outline (Mechanical 13), a courtyard (Mechanical 15) and an extruded 3D body of the given `height`
  (Mechanical 13);
- the IPC-7351 name, unless `name` is given.

| package | fields (mm) |
|---|---|
| `chip` | `size` "0201"/"0402"/"0603"/"0805"/"1206"/"1210"/"1812"/"2010"/"2512" (or `body` [l, w] + `terminal`), `kind` resistor/capacitor/inductor |
| `soic`, `sop`, `tssop`, `msop`, `ssop` | `pins` (even), `pitch`, `span` (lead tip to tip), `body` [w, l], `foot` (lead length), `lead` (lead width), `height` |
| `sot23` | `pins` 3, 5 or 6 (JEDEC sizes built in; any field overrides) |
| `qfn`, `dfn` | `pins` (or `nx`, `ny` per side), `pitch`, `body` [x, y], `term` (terminal length), `lead` (width), `ep` [w, h] exposed pad (numbered last) |
| `header` | `rows`, `pins` or `cols`, `pitch` (2.54), `drill`, `pad`; pin 1 square, numbered along each row |
| `custom` | `pads`: [{`number`, `x`, `y`, `w`, `h`, `shape` rect/round/roundrect/oct, `rotation`, `hole`, `slot`, `plated`, `layer` top/bottom}], `body` [w, l] for the outlines |

Optional on any footprint:
- `description`, `height`;
- `lines`: [{`layer`, `from`, `to`, `width`}], on any layer by name: `top_overlay`, `mech1`..`mech16`, `top`, `keepout`, ...

Read the datasheet's land pattern or package drawing for the dimensions. A package not in the table is `custom` with its
recommended lands.

## schlib

`{"path", "symbols": [...], "remove": [names]}`

A symbol has:
- `name` (its library reference), `description`;
- `designator`: "U?", "R?", "J?"...;
- `comment`: shown on the sheet; defaults to the name;
- `footprint`: a name, or a list (the first is the current one). The footprint links to the pcblib of the same call;
  `footprint_library` names another library;
- `parameters`: {"Manufacturer": "..."}.

A symbol is drawn in one of three ways:

- **Pins by side** (an IC): `pins`: [{`number`, `name`, `side` left/right/top/bottom, `type`, `part`, `inverted`,
  `clock`, `hidden`, `description`}].
  - The pins go 100 mil apart: left and right top to bottom, top and bottom left to right.
  - `{"gap": true, "side": "left"}` leaves a gap.
  - The box is sized to the names.
  - Pins with `part` 2, 3... make a multi-part symbol: one box per part.
- **Template:** `"template"`: `resistor`, `capacitor`, `inductor`, `diode` (pins A, K) or `led`. Two pins, 1 and 2.
- **Pins at points:** each pin has `at` [x, y] (its wire end) and `points` left/right/up/down (from the body outwards),
  with an optional `length` (200). Then `shapes`: [{`kind` rect `from` `to` / line `points` / polygon `points` /
  arc `center` `radius` `start` `end` / ellipse `center` `rx` `ry` / text `at` `text`, `filled`, `part`}].

`type` is one of: `input`, `output`, `io`, `passive` (the default), `power`, `open_collector`, `open_emitter`, `hiz`.

## schdoc

`{"path", "size", ...}`. A missing file is a new sheet of `size`: A4, A3, A2, A1, A0, A, B, C, D, E, Letter, Legal or
Tabloid. An existing one is edited.

The operations run in this order:

1. `remove`: {`parts`: [designators], `nets`: [net label / power port texts], `at`: [[x, y]]}. A part goes with
   everything it owns. Wires, labels, ports, junctions and no-ERC marks are removed by text or by a point they touch.
2. `place`: [{`symbol`, `library` (.SchLib path), `designator`, `at` [x, y], `rotation` 0/90/180/270, `mirror`, `part`,
   `comment`, `parameters` {}}].
   - The symbol's origin goes to `at`.
   - Another part of a multi-part symbol takes the same designator and a different `part` (U2A, U2B).
3. `set`: [{`part` designator, `name`, `value`, `visible`}]. Sets a parameter, adding it if missing. `name` "Comment"
   sets the comment.
4. `wires`: [[[x, y], [x, y], ...]], `labels`: [{`net`, `at`, `rotation`}],
   `power`: [{`net`, `at`, `style`, `rotation`}], `junctions`: [[x, y]], `no_erc`: [[x, y]].
   - `style` is one of: `bar`, `arrow`, `circle`, `wave`, `ground`, `signal_ground`, `earth`.
   - A port without `style`: ground for GND nets, else bar.
5. `connect`: [{`pin` "U1.3" or "U1.VIN", `net`, `power`, `stub`}]. This is the easy way to wire a sheet.
   - It draws a short wire out from the pin's end (100 mil, longer for a long label), then a net label at its end, or a
     power port when `power` is a style or `true`.
   - Nets with GND in the name get a ground port unless `"power": false`.
   - `"net": "NC"` puts a no-ERC mark on the pin.
   - Pins on the same net name are connected (labels and ports join nets across the sheet).

Lay parts out on the 100 mil grid with room between them (500 mil or more). A pin's end must land on another pin, a
wire end, or a wire's vertex: anything else is not connected. `connect` avoids all of that.

## project

`"project": "Board.PrjPcb"`: the written files are added to it, and it is made when missing.

## When Altium has the files open

The tool runs on the machine with the person's Altium, when that Altium is running:

- **Unsaved edits:** a target document open in Altium with unsaved edits stops the call, and nothing is written.
  Overwriting it would raise Altium's modal "has been modified ... Reload?" box. Save or close it first, or ask the person.
- **Open without edits:** target documents open with no edits are closed before the write, then reopened and shown after
  it. The person's focused document is shown again last.
- **Open project:** a project open in Altium gets the new documents added there as well. Altium does not reload a changed
  .PrjPcb, and saving the project there would drop them. Tell the person to save the project in Altium.

The result's `altium` lines say what happened.

Never ask Altium to reload a document with `IServerDocument.DoFileLoad`: it appends the file to the open document, and
every object ends up twice.

## Limits

- No .PcbDoc writing: place footprints on the board with Altium's Update PCB (the person), then edit with `altium_script`.
- A placed part's text (designator, comment, a symbol's text) reads left to right after a turn or mirror, as Altium
  draws it. Check crowded spots on the picture: `viewer_show` the sheet.
- No footprint text (the `.Designator` special string) and no pad stacks beyond top/middle/bottom: add them in Altium if
  they matter.
- Library files of a type Altium cannot hold (IntLib, DbLib) are not written.
