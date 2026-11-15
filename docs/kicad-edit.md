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
4. ends the commit, refills the zones it changed, saves (the save is committed to main like any `SaveDocument`) and
   turns the person's viewer to the document (`show: false` leaves it).

An op that fails drops the commit: none of the call's item changes stay and nothing is saved. Net classes, rules and
raw `ipc` commands that ran before it are not undone; the error names them.

Every command goes through the same gate as a `kicad_ipc` call (`Agents::kicad_call`): the approval refusals, the
one-writer lock, the co-editing actor name, the pause from KiCad's Agents panel, the save gate. With the person's
KiCad open the edit lands in their live document.

## Ops

Parts are named by reference; lengths are millimetres, angles degrees, layers as KiCad shows them (`F.Cu`).

| Op | What it does |
|---|---|
| `{find: {kind, ...match}}` | The rows of a kind. Board: footprint, pad, track, arc, via, zone, text, shape, net. Schematic: symbol, wire, label, junction, no_connect, sheet. `fields: true` adds a part's fields, `raw: true` gives KiCad's own items, `limit` (200). |
| `{move: refs, by: [dx, dy]}` | Also `to: [x, y]` and `rot`. |
| `{set: refs or {kind, ...match}, ...row keys}` | Change what the row shows: `value`, `fields: {MPN: ...}`, `at`, `rot`, a zone's `clearance`, `min_width`, `name`, `priority`, a track's `width`, `layer`, `net`. `raw: {...}` merges into KiCad's own item. |
| `{add: [...], sheet?}` | `{track: {from, to, width, layer, net}}`, `{via: {at, size, drill, net}}`, `{wire: [[x, y], ...]}`, `{label: {text, at, type}}`, `{junction: [x, y]}`, `{no_connect: [x, y]}`, `{symbol: {lib, ref, value, at, rot, fields}}`, or KiCad's own item with `@type`. |
| `{delete: ids or {kind, ...match}}` | A selector must say more than a kind. |
| `{netclass: {name, clearance, track_width, diff_pair_width, diff_pair_gap, via_diameter, via_drill, patterns}}` | Added or changed (merged), the patterns assigned to it. |
| `{rule: {name, condition, constraints: [{type, min, opt, max}], layer, severity}}` | Custom rules by name (`EditCustomRules`); `{rule: {remove: name}}`. |
| `{refill: true}` | Refill every zone. |
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
- a command that finds its document closed opens it and runs again;
- a short or misplaced command name is completed (`GetItems`, `kiapi.board.commands.CreateItems`), and `"$doc"` /
  `"$project"` in a message stand for the open document and its project.

## For Claude

`kicad_edit` is listed with `_meta["anthropic/alwaysLoad"]` to a thread whose project has a board or schematic
(`Agents::has_design`), so its definition is in the first request instead of behind a tool search.

## Tests

`cargo test kedit` (the rules and the driver against a stand-in KiCad). Against a real KiCad:
`python3 test/kipc_bench/bench.py e2e WORK` runs each benchmark task as one `kicad_edit` call and compares the saved
files with the reference item for item ([the benchmark](kicad-ipc-bench.md)).
