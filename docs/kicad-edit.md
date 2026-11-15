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

A new symbol is first placed by KiCad from its library alone, in a commit that is dropped, and the real one keeps that
placement of its reference and value (fields sent without a place land hidden at the sheet's origin).

An op that fails drops the commit: none of the call's item changes stay and nothing is saved. Net classes, rules and
raw `ipc` commands that ran before it are not undone; the error names them. KiCad shows a commit's changes only once it
ends, so a `find` after a change ends the commit so far (its rows are the document as changed); a later failure then
keeps and saves what came before that find, and says so.

The answer is `{results: [one per op], saved, shown}`. A change answers with the items as they now are (24 at most,
else their count), so nothing is read again to check it.

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
- a command that finds its document closed opens it and runs again (headless only: never in the person's KiCad);
- a short or misplaced command name is completed (`GetItems`, `kiapi.board.commands.CreateItems`), and `"$doc"` /
  `"$project"` in a message stand for the open document and its project. Only the name is completed: a misspelt field
  in the message is still KiCad's error.

## For Claude

`kicad_edit` is listed with `_meta["anthropic/alwaysLoad"]` to a thread whose project has a board or schematic
(`Agents::has_design`), so its definition is in the first request instead of behind a tool search.

## Tests

`cargo test kedit` (the rules and the driver against a stand-in KiCad). Against a real KiCad:
`python3 test/kipc_bench/bench.py e2e WORK` runs each benchmark task as one `kicad_edit` call and compares the saved
files with the reference item for item ([the benchmark](kicad-ipc-bench.md)).
