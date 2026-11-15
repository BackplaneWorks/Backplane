# Repros for the KiCad fork (found by the token benchmark)

Runtime: Backplane KiCad 10.99.0-bp.12.1 (source 7fb2d1bf67), headless `kicad-cli api-server`, Linux x86-64. Not tried on
bp.12.4 or batch 7. Each script talks to a benchmark hub's `kicad_ipc` (README.md one folder up): start a hub, copy the
fixture, hold a turn open and run the script with the turn's MCP url:

```sh
W=$BP_TMP/kipc-bench/a; cp -r $BP_TMP/kipc-bench/fixtures/stickhub $W/runs/r5        # pic_programmer for repro 4
bun test/kipc_bench/turn.mjs $W $W/runs/r5 "HOLD $W/r5.url" nowait
python3 test/kipc_bench/repros/fp_value_partial.py $W/r5.url $W/runs/r5              # touch $W/r5.url.stop ends the turn
```

The same messages go to KiCad unchanged through any client; the hub adds nothing to them.

## (5) A partial FootprintInstance update resets the value field's layer, place and visibility: `fp_value_partial.py`

Board: the runtime's `demos/stickhub/StickHub.kicad_pcb` (upgraded by `kicad-cli pcb upgrade`), footprint R2
(`57f2a42b-f94b-4bd7-917e-cb76fae439df`, on B.Cu). Message, inside a commit:

```json
{"header": {"document": DOC}, "items": [{"@type": "type.googleapis.com/kiapi.board.types.FootprintInstance",
  "id": {"value": "57f2a42b-f94b-4bd7-917e-cb76fae439df"}, "value_field": {"text": {"text": {"text": "10k1"}}}}]}
```

Answer: `ISC_OK`. Expected: the Value property keeps its layer (B.Fab), size (0.3 mm), place and visibility, with only the
text changed (as the footprint's position, orientation and body are kept when left out). Actual, in the saved file: the
property is on `F.SilkS`, `(hide yes)`, at a place far from the part (`(at 170.246365 -34.403436 135)`); its size and
justification are kept. Sending the whole `value_field` as GetItems gave it, with only the text changed, keeps all of it (the script does both
and prints the two properties).

## (4) A new symbol's reference and value land hidden at the sheet origin: `symbol_fields.py`

Schematic: the runtime's `demos/pic_programmer/pic_programmer.kicad_sch` (upgraded), root sheet. Three CreateItems of
`Device:R` in one commit, which is dropped at the end:

1. with `reference_field: {name: "Reference", text: {text: "R92"}}` and the same for the value: `ISC_OK`; the returned
   item's reference and value have `position: {}` (the sheet's origin) and no field-level `visible`. Saved, the property
   reads `(at 0 0 0) (hide yes)`.
2. the same with `visible: true`, `allow_auto_place: true` on both fields and `fields_autoplaced: true` on the symbol:
   visible, but still `position: {}`: nothing is autoplaced.
3. with no fields at all: placed from the library (`R` at x + 2.032 mm, visible).

Expected for 1 and 2: a field given without a position is placed as the library places it (as in 3), keeping the given
text; `fields_autoplaced: true` autoplaces.

## (8) "did not answer Ping: KiCad is not ready to reply"

Seen 6 times in about 190 server starts, always as a turn's first command for a project, with two KiCad servers of the
same hub starting or stopping at that moment (the hub keeps two and asks the least recently used to shut down for a
third). The tool's answer, whole:

    KiCad IPC unavailable: the headless KiCad for StickHub stopped: it did not answer Ping: KiCad is not ready to reply.
    Fallback: edit the file directly or use `<kicad-cli> pcb …`; re-read the file after.

A second call five seconds later got the same answer (the server stays "failed" for the hub's retry schedule). The hub
logs a line only when a server comes up (`kicad ipc: <pro> up, KiCad says …`), so there is no log line for the failure:
the project is simply missing from the "up" lines. KiCad's own stdout is not kept by the hub.

The hub no longer fails on it: a server that answers Ping with not-ready is asked again for up to 30 s
(`native/host/src/kicad/ipc.rs`, `READY_WAIT`). In the 48 runs after that change it was not seen again. For the fork, the
question is only whether a server should listen before it can answer.

Both scripts were run again on bp.12.1 on 2026-11-14 with the outputs described above.

## (6) A symbol that `extends` another, created by its library id, is a library mismatch: `symbol_extends.py`

Schematic: `pic_programmer.kicad_sch` (upgraded), root sheet, bp.12.1 (run 2026-11-15). CreateItems, in a commit that is
kept and saved:

```json
{"@type": "type.googleapis.com/kiapi.schematic.types.SchematicSymbolInstance", "position": {"x_nm": "241300000", "y_nm": "33020000"},
 "definition": {"id": {"library_nickname": "Regulator_Linear", "entry_name": "AMS1117-3.3"}},
 "lib_id": {"library_nickname": "Regulator_Linear", "entry_name": "AMS1117-3.3"}, "unit": {"unit": 1},
 "reference_field": {"name": "Reference", "text": {"text": "U91"}}}
```

Answer: `ISC_OK`. `Regulator_Linear:AMS1117-3.3` extends `AP1117-15`. Expected: the sheet's `lib_symbols` copy is the
flattened symbol, as `PlaceSymbolFromLibrary` writes it. Actual: `kicad-cli sch erc` on the saved file reports
`lib_symbol_mismatch: Symbol U91 [AMS1117-3.3]`. A symbol that extends nothing (`Device:C`) is right. What the hub does
now: `PlaceSymbolFromLibrary` in a commit that is dropped, then CreateItems of that item whole (its `definition` as KiCad
gave it); that saves without the finding.

## (7) A Footprint sent as a user field is answered OK and not written: same script

The same CreateItems with `Device:C` and
`"user_fields": [{"name": "Footprint", "text": {"text": "Capacitor_SMD:C_0805_2012Metric", ...}}]`: `ISC_OK`, and the saved
symbol has `(property "Footprint" ""`. UpdateItems with the same user field does the same. Expected: the field named
Footprint is the symbol's footprint field, or the item is refused. `footprint_field: {text: {text: "..."}}` works (what
the hub sends now).

## (8) SketchRoute, headless, on StickHub with the seven `/LED*` nets taken up (2026-11-15, bp.12.1)

`kiapi.backplane.SketchRoute {board, items: [the 14 pads]}` after a net class `LED` (0.2 mm track, pattern `/LED*`) was set
and saved in the same server: `connections: 7, routed: 6, failed: 1` (`/LED6`), 0.07 s. The tracks are 0.15 mm wide (the
board's default), not the net class's 0.2 mm; total length 28.2 mm where 13.5 mm does it; and after a refill DRC has one
unconnected GND item (a pour cut off). A message with no `board` answers `AS_UNHANDLED`, not a bad request. Asked for:
a `width` (or the net class's), and a per-connection reason for a failure. The hub routes with its own grid router
(`native/host/src/kicad/kroute.rs`) until then.
