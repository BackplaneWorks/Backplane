# kicad_edit: a KiCad edit in one call

An agent's turn costs tokens per request: every request carries the whole context again (about 40 thousand tokens
before the first tool call). Over `kicad_ipc` a small edit took 15 to 38 requests: find the tool, hit the lock, open
the document, read items (a footprint is 15 to 90 KB of JSON; a 94-part board's footprints are 1.8 MB), begin a
commit, update, end it, save, then re-read the file to check. `kicad_edit` is the same edit as one request.

`kicad_edit {path, ops, message?, save?, show?}` (`native/host/src/kicad/kedit.rs`; the MCP glue is
`Agents::tool_kicad_edit` in `agents/design.rs`). The hub:

1. takes the document's lock for the thread when nobody holds it (as `lock_acquire` would; released at the turn's end);
2. opens the board or schematic `path` names if it is not open;
3. runs `ops` in order, the item changes inside one commit (`message` is its name in KiCad's undo history);
4. ends the commit, refills the zones (every zone when copper was added or moved, else the ones it changed), saves (the
   save is committed to main like any `SaveDocument`) and turns the person's viewer to the document (`show: false`
   leaves it);
5. answers with what KiCad's own check now says that it did not say before the edit (`check`, below).

A new symbol is first placed by KiCad from its library (`PlaceSymbolFromLibrary`), in a commit that is dropped, and the
real one is that item whole: its reference and value where the library has them, and KiCad's own definition, without
which a symbol that extends another (`AMS1117-3.3`) saves as a library mismatch. `Footprint`, `Datasheet` and
`Description` are a symbol's own fields, never user fields (KiCad answers OK to a Footprint user field and does not
write it).

An op that fails drops the commit: none of the call's item changes stay and nothing is saved. Net classes, rules and
raw `ipc` commands that ran before it are not undone; the error names them. KiCad shows a commit's changes only once it
ends, so a `find` after a change ends the commit so far (its rows are the document as changed); a later failure then
keeps and saves what came before that find, and says so.

The answer is `{results: [one per op], saved, check, nets, shown}`. A change answers with the items as they now are (24
at most, else their count), so nothing is read again to check it.

## The geometry is the hub's

An agent asked to route seven nets through a file wrote a clearance search in Python first; asked through the old
`kicad_edit` it did the same and then sent the tracks. These ops do that work (`native/host/src/kicad/kroute.rs`, pure
and tested without a KiCad; the driver in `kedit.rs` feeds it KiCad's own items):

- **`route`.** The board as shapes: pads (round-rect corners exact), tracks, arcs, vias, holes, copper drawn as
  graphics, the board's edge and keep-out areas, each with its layers and net. Each net's unconnected groups (copper that
  touches; a pour's outline joins what stands in it) are joined by an A* search on a grid a quarter of the track or
  finer (0.0125 to 0.025 mm), in 45 degree steps, with the clearance of the net's class and of each neighbour's, the
  edge clearance and the hole clearance. Vias are the kinds the board already has (a through via, or the micro via of
  each layer pair, made like one of the board's own: filled, capped, tented as they are), each at a price by how rare
  it is on the board, so a board of laser micro vias gets micro vias; a drilled via under a part of eight pads or
  more costs more, and a micro via may stand in a pad of its net. A layer the board hardly routes on costs a quarter
  more; a plane layer (an inner layer with a zone and next to no tracks) is open to a stack of micro vias passing
  through and, at two and a half times the length, to a track; a track through another net's pour on an inner layer
  costs three times its length. A track keeps a little room over its clearance where there is room, and the way found
  on the grid is pulled into straight runs at 0, 45 and 90 degrees with no corner sharper than a right angle. A new
  track that ends part way along another of its net breaks that one there, as KiCad's own router does. Short nets go first; a
  net with no way goes first in the next try (four at most). `ripup: true` takes the nets' old tracks up first. A part
  named in place of a net stands for the nets on its pads, and another net's track lying on one of those pads is taken
  up with its net and routed again, and an old track's end where a pad stood before the exchange is cut back to where it
  still joins something (what a footprint exchange leaves).
- **`place`.** A part goes next to a pin: its pad on the pin's net lands within `within` mm, at one of the four
  rotations square to the pin's part, at the nearest place where its courtyard overlaps no other and its pads keep
  their clearance. A courtyard that overlaps is pushed out the shortest way and centred in a slot between two parts; a
  pad a few hundredths too near a track moves off it by what is missing.
- **A footprint exchange** (`set: refs, footprint: "Lib:Name"` on a board). KiCad places the new footprint where the old
  one stands (in a dropped commit), pad 1 on the old pad 1, and the old part is updated with that definition: its id,
  reference, value, fields, symbol link and each pad's net (by number) stay.
- **A schematic group.** `pins: {pin: net}` on a new symbol joins each pin to its net with a 2.54 mm wire and the net's
  power symbol (the next free `#PWR` number), or a label where the power library has none. A point may be a pin
  (`"U7.2"`): in a wire (which then runs square between its points), a label's or a symbol's `at`. A group of new symbols that would lie
  over something the sheet has (a symbol, a wire, a label, a sheet symbol) is moved as one, on the 1.27 mm grid, to the
  nearest free room: inside `fit: [x0, y0, x1, y1]` when given, else within what the sheet has drawn; the answer says by
  how much and what it lay over (`fit: false` leaves it where it was asked and names what it lies over). A moved symbol
  takes its field texts with it.

What this model does not know (a custom rule, a rule area's condition) KiCad's check finds, in the same answer.

## The check

Before the first change and after the save the hub runs `kicad-cli pcb drc --schematic-parity` (a board) or `sch erc` (the root schematic)
and answers `check: {by, new: [the findings this edit added, 12 at most], more, elsewhere, left, gone}` (`kedit::check_delta`,
`checkrun::report_of`): each finding is its kind, its items and its place. Two findings are the same one when only a track's length, the place or the items' order differs (a refill changes those); and on a board, a new finding that names none of the nets or parts the call worked on is set apart as `elsewhere` (its count and the first three), since a refill or a file's first save moves such findings about on a large board. About two seconds each on the boards tried
(Hubble_DK, 8 layers: 2.0 s). `check: false` leaves it out; a call that changes nothing or does not save is not
checked. After a schematic edit, `nets` is the saved netlist's net of every pin of the parts the call added, moved or
wired by pin (`checkrun::netlist_of`), so no netlist is exported to see that a pin landed on its net.

## By the runtime's capabilities

`api-field-fixes` (the fork's batch 10: a symbol created by its library id is written flattened, text-only fields are
placed as the library has them, a user field named Footprint fills the symbol's own, a partial footprint update keeps a
field's layer, place and visibility). With it (`kedit::FIELD_FIXES`, read from the runtime's manifest): a new symbol is
created as its library id and its texts, Footprint among its fields by name, and a `set` of only a footprint's value
or reference sends that one field's text. KiCad's own placement of a new symbol is still read once in a dropped
commit: that is where the pins are, which `pins`, wires by pin and the move to free room need before anything is
created. Without the capability, the way described above: the whole placed item, `footprint_field`, the whole
footprint without its body. Both ways are tested (`a_runtime_with_the_fixed_item_api_gets_the_plain_requests`).

`sketch-route-width` (SketchRoute honours the net class or `track_width` and names each failure) is not used: see the
benchmark's routing section for the measurement.

## What it keeps

Every command goes through the same gate as a `kicad_ipc` call (`Agents::kicad_call`), so an edit in one call is held
to what an edit in twenty was:

- **The save gate** (`ipc::save_gate`). A save KiCad would not make (the document is unmodified; the board's only
  change is KiCad's own load-time migration) answers `saved: false` with `not_saved`; a real save carries
  `format_upgrade` and `kicad_load_changes` when it wrote those too. The commit to main follows only a real save.
- **Refusals.** The plan's approval (`ApprovePlan`, the plan view's actions) is refused before anything is opened or
  locked. An open dialog's `AS_BUSY` is the answer, naming the dialog. KiCad's task gate (`AS_BUSY: a task is
  running`, a check or a refill) is waited out, asked again every 250 ms for at most a minute, so a running check
  does not fail an edit.
- **Read-back.** What `set` puts on symbols (value, fields) is read back from the saved sheet files, a sub-sheet's
  most of all (`kedit::file_prop`): an edit KiCad answered OK but the file does not show is an error, never "done".
  Items KiCad deferred because the person is editing them (`deferred` in the answer) are not read back.
- **The person's KiCad.** With it open the edit lands in their live document, under the agent's name in the Agents
  panel, and waits while they have paused the agent. A document their KiCad has not opened is theirs to open: nothing
  is opened, shown or raised for an agent there (headless, the hub opens it; it never closes one).
- **The person first.** While the person is working in that KiCad (a selection or change in the last two minutes,
  `coedit::person_now`), a call that changes anything is answered with what they are doing and changes nothing. The
  agent asks them if its edit touches that, else repeats the call with `person_seen: true`. (Over `kicad_ipc` the agent
  saw `person` on a read before it wrote; one call must not skip that.)
- **The lock.** A free document is taken for the writer; one another thread holds is refused, naming the holder, and
  nobody is queued without asking.

## Ops

Every answer, and every error after KiCad was reached, carries `served_by`: `headless` (kicad-cli api-server), `embedded` (the person's KiCad), `both` (the route changed between two requests), or null (refused before any request). A commit open in one KiCad is never continued in the other: the request is refused and the edit dropped.

Parts are named by reference; lengths are millimetres, angles degrees, layers as KiCad shows them (`F.Cu`).

| Op | What it does |
|---|---|
| `{find: {kind, ...match}}` | The rows of a kind. Board: footprint, pad, track, arc, via, zone, text, shape, net. Schematic: symbol, wire, label, junction, no_connect, sheet. `fields: true` adds a part's fields, `raw: true` gives KiCad's own items, `limit` (200). |
| `{move: refs, by: [dx, dy]}` | Also `to: [x, y]` and `rot`. |
| `{set: refs, footprint: "Lib:Name"}` | A board: the footprint exchanged (above), answered with the pads' new places. A schematic: the symbols' Footprint. |
| `{set: refs or {kind, ...match}, ...row keys}` | Change what the row shows: `value`, `fields: {MPN: ...}`, `at`, `rot`, a zone's `clearance`, `min_width`, `name`, `priority`, a track's `width`, `layer`, `net`. `raw: {...}` merges into KiCad's own item; a symbol's `definition` is refused (its library symbol is not changed by an update: delete and add). |
| `{add: [...], sheet?, fit?}` | `{track: {from, to, width, layer, net}}`, `{via: {at, size, drill, net, layers}}`, `{wire: [[x, y] or "REF.PIN", ...]}`, `{label: {text, at, type}}`, `{junction: [x, y]}`, `{no_connect: [x, y]}`, `{symbol: {lib, ref, value, at, rot, footprint, fields, pins}}`, or KiCad's own item with `@type` (a `SchematicSymbolInstance` naming only `definition.id` is placed from its library first, KiCad's definition kept whole, its own keys over it). A new symbol answers with its pins' places. A symbol KiCad gives with no pins and nothing drawn, fields only, is refused unless `allow_empty: true` (#1192: an HTTP library part whose source symbol had not loaded in the headless KiCad up to bp.13.2; place the source symbol itself with the part's fields instead). |
| `{delete: ids or {kind, ...match}}` | A selector must say more than a kind. A selector first ends the call's open commit, so it finds what this call added. |
| `{netclass: {name, clearance, track_width, diff_pair_width, diff_pair_gap, via_diameter, via_drill, patterns}}` | Added or changed (merged), the patterns assigned to it. |
| `{rule: {name, condition, constraints: [{type, min, opt, max}], layer, severity}}` | Custom rules by name (`EditCustomRules`); `{rule: {remove: name}}`. |
| `{refill: true}` | Refill every zone. |
| `{route: nets or parts, width, layers, vias, via: {size, drill}, ripup}` | Route what is unconnected on those nets (above). Answers per net: width, length, vias, layers; `failed` with why; `already_connected`. |
| `{place: {ref: "U1.26", ...}, within, pad}` | Each part next to that pin (above). Answers its place, rotation, side and the pad's distance. |
| `{ipc: {type, message}}` | Any `kicad_ipc` command in the sequence; `"$doc"` and `"$project"` stand for the document and its project. |

A selector matches a row's keys: text takes `*` and `?`, a list is any-of, `near: [x, y, radius]` is a place (a track
or wire is near anywhere along it), `sheet` is a sheet's name or path. A reference named outright that is not there
is an error, not a smaller edit.

A row is what an agent reads and, with the same keys, what `set` writes:

| Kind | Row |
|---|---|
| footprint | `id, ref, value, at, rot, layer` (+ `fields`) |
| pad | `id, ref, pad, net, at, pin` |
| track, arc | `id, from, to, width, layer, net` |
| via | `id, at, size, drill, net` |
| zone | `id, name, net, layers, priority, clearance, min_width, corners, type` |
| symbol | `id, ref, value, at, rot, lib, unit, sheet` (+ `fields`) |
| wire | `id, from, to, type` |
| label | `id, text, at, type` |

How an item is written back: the hub reads KiCad's own item, changes what the op names and sends it whole, so a field's
text style, a zone's outline and every setting the row does not show stay as they were. A zone goes without its fills
(KiCad refills it), a footprint without its body unless a field in it changed, a symbol without its library body; a
symbol is updated on its own sheet, so one `set` reaches every sheet of a hierarchy.

## kicad_ipc, made forgiving

`kicad_ipc` stays the raw way in, for what `kicad_edit` lacks. Three things no longer cost a request each
(`Agents::tool_kicad_ipc`):

- a write to a document nobody holds takes its lock (`kicad_gate_take`); a document another thread holds is refused as before;
- a command that finds its document closed opens it and runs again (headless only: never in the person's KiCad);
- a short or misplaced command name is completed (`GetItems`, `kiapi.board.commands.CreateItems`), and `"$doc"` /
  `"$project"` in a message stand for the open document and its project. Only the name is completed: a misspelt field
  in the message is still KiCad's error.

## For Claude

`kicad_edit` is listed with `_meta["anthropic/alwaysLoad"]` to a thread whose project has a board or schematic
(`Agents::has_design`), so its definition is in the first request instead of behind a tool search.

## Tests

`cargo test kedit` (the rules and the driver against a stand-in KiCad) and `cargo test kroute` (the shapes, the router
and the placement, pure). Against a real KiCad: `python3 test/kipc_bench/bench.py e2e WORK` runs each benchmark task as
one `kicad_edit` call and compares the saved files with the reference item for item; with
`KIPC_BENCH_TASKS=test/kipc_bench/tasks2.json` it runs the design tasks (route, place, a schematic group, a footprint
exchange, a real 8-layer board) as the fewest calls and holds each to its rules: DRC or ERC with nothing new, every net
connected, the spec met ([the benchmark](kicad-ipc-bench.md)).
