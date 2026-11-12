# Management panels

Panels about the open design, beside it: BOM, Checks, Lifecycle (later Plan and Review). They sit in the viewer pane under the design
(the embedded KiCad, or Backplane's Board, Schematic, 3D and Split viewers): a slim tab strip along the bottom of the pane, and the open
panel at the pane's right side or bottom. Everything a panel shows is hub data, so the phones can draw the same panels.

## State

Per project, the hub keeps which panel is open, its side (`right`|`bottom`) and size in px (`<home>/mgmt/panels.json`, at most 200
projects, the oldest dropped). A press on a tab is drawn at once and confirmed by the hub's state.

- `mgmt.watch {project}` → `{digest, project, state, data}` (`data`: each panel's value as published so far, by id): a client shows this project's design. Every panel starts (or refreshes) what it
  publishes for the project. `project` is any absolute path of the project (its `.kicad_pro`, board, schematic or folder).
- `mgmt.panel {project, toggle?, open?, side?, size?}` → `{digest, state}`: `toggle` opens a panel or shuts it when it is the open one;
  `open` names one or `""`; a new `side` takes that side's default size unless `size` is given; `size` is clamped to 160–1600.

## Info

- `mgmt.state.<digest>`: `{project, open, side, size, panels: [{id, label}]}`; `panels` are the tabs, in order.
- `mgmt.<panel>.<digest>`: `{project, badge: {n, tone}, ...}`; the tab shows `n` (when above 0) in `tone` (`warn` or `bad`).

`<digest>` is the project's digest (`kicad/kserve.rs digest` of the `.kicad_pro` path), as in `editor.kicad.<digest>`.

## BOM (`mgmt.bom.<digest>`)

`{badge, source, at, parts, missing_mpn, dnp, more, lines: [{id, refs, qty, value, footprint, mpn, mfr, fab, dnp}]}`

- `source`: `kicad` while the project's GUI KiCad has its schematic open (GetItems over its API once a second: an edit in KiCad, or an
  agent's through it, shows before it is saved), else `file` (the saved schematic and every sheet it reaches, read again within a second of
  a save).
- Lines group parts with the same value, footprint, MPN and DNP (`kicad/bom.rs group`), in natural order of their first reference; `id` is
  that reference. `fab` is the fab's part number (the symbol's `LCSC` field). At most 400 lines; `more` counts the rest.
- The badge counts lines without an MPN (DNP lines excepted) once some line has an MPN or a fab number.
- A press on a line selects its parts in the open KiCad and lights them in the viewers (`editor.kicad.select`).
- Each line that has an MPN or a fab number carries `key` (`mpn:<MPN>` or `fab:<number>`) and `stock`: `{status, stock, class, price,
  need, fab, at}`, `status` one of `in`, `low` (under five builds' worth), `out` (fewer than `need`: the line's quantity on every board),
  `unknown` (with `why`), `checking`. `boards` is the board fab file's `qty`; `cost` is one board's parts at the prices for `need`, over
  `priced` of `lookups` lines. The badge counts lines without an MPN and lines out of stock (`bad` when any is out).
- What a line says of its part, each with `checked` (unix seconds of the reading; the design review reads these, never the parts search;
  a field is absent while the part was never checked):
  - `stock: {status, stock, need, price, class, fab, at, checked}`: the fab's stock at the project's quantity (`need`);
  - `library: {class, checked}`: the fab's library class, `basic`, `preferred` or `extended`;
  - `lifecycle: {status, words, source, checked}`: `status` is `active`, `nrnd`, `eol` (EOL, obsolete, discontinued) or `unknown`;
    `words` are the distributor's own ("Obsolete", "Not Recommended for New Designs", "" when it states none); `source` is `mouser`.

## BOM edits

`mgmt.bom.edit {project, refs, field, value}` → `{via, changed}` sets `mpn` (the MPN field, or the MPN alias the symbol has), `fab`
(`LCSC`), `value`, `footprint` (`Library:Name`) or `dnp` (`yes`|`no`) on every symbol (each unit) of a line. Only standard symbol fields and
the DNP attribute are written; a new MPN or LCSC field is hidden.

- `via: "kicad"`: the project's GUI KiCad has the schematic open. One commit (BeginCommit, UpdateItems, EndCommit with the words
  "BOM: MPN of R1, R2 = …"), not saved: Ctrl+Z in KiCad takes it back, and the person saves it with the rest.
- `via: "file"`: no KiCad has it open. The headless KiCad writes and saves it while the hub holds the schematic's named lock
  (`agents/design.rs`, one writer); an agent holding the lock gets the edit refused with its name.

## Stock

The fab's stock comes from the `pcbparts` MCP server (Settings > MCP servers can turn it off: `mcp.pcbparts`; the tests' stand-in is
`BACKPLANE_PCBPARTS_URL`), through the hub (`kicad/parts.rs`): a fab number by `jlc_get_part`; an MPN by `jlc_get_part`, then
`jlc_stock_check` (it also finds parts out of stock), then the fab number's price breaks; a part found before (within 7 days) is asked
by its fab number only, one call. One worker asks, one call every 3 s at most and 300 an hour (the jlc tools take one part a call, so
there is nothing to batch); answers are cached 6 h (a failure 15 min), outlive a restart through the history's last sample, and are
refreshed on that schedule while a BOM shows them. A 429 pauses every lookup for the `retry_after` it names (at least a minute, doubled
for each limit in a row, at most 30 minutes; the key goes back to the head of the queue, never retried in a loop), and the BOM says
"Stock check paused" (`paused`). A typed MPN is looked up once it is on the line, never per keystroke. `mgmt.bom.alts {project, fab, need}` → `{alternatives: [{fab, model, mfr, stock, class, price,
package}]}` asks `jlc_find_alternatives` on demand.

## Lifecycle (`mgmt.life.<digest>`)

`{badge, counts: {active, nrnd, eol, unknown}, samples, since, boards, parts: [{key, id, refs, mpn, lifecycle, stock, price, n, first,
last}], bom: {cost: [[day, $ a board]], instock: [[day, lines in stock]], lines}}`; `mgmt.life.part {project, key}` → `{key, samples:
[{at, stock, price, lifecycle, class}]}`.

- History: `<home>/parts/history/<key digest>.jsonl` (`kicad/parthist.rs`), one sample a line, appended at every fresh stock answer and
  lifecycle reading, and the fab's own recorded stock changes (`jlc_stock_history`, once a week a part, through the same limits;
  `source: "fab_history"`, stock only: the service records changes that leave 1,000 or fewer, so the graphs have a past from the first
  day for parts that ran low); at most 2000 a part (past 2500 the oldest go). It starts when Backplane first looks a part up; with one reading the
  panel says so and shows the values it has.
- Lifecycle readings: `mouser_get_part`, ten parts a call, each part once a day, at most 40 calls a day (the distributors' allowance is
  shared), mapped to active, NRND, EOL or unknown (Mouser states none for most parts: unknown). The badge counts NRND and EOL parts (`bad`
  with any EOL).

## Checks (`mgmt.checks.<digest>`)

One list of everything that checks the design: DRC and ERC (`kicad/checkrun.rs`, kicad-cli on the saved files) and the sources other parts
of the hub add (the design review's fab rules, BOM, outputs, assembly, mech, agent-judged datasheet checks). The value is the checks
summary (`kicad.checks.<digest>`: `drc`, `erc`, `sources`, `order`, `chip`) plus `badge` (errors and warnings; red with an error), `total`
and `groups` (by source and rule, worst first, each with its count `n` and its first rows within the info's size; `kicad.checks.list` has
the rest). A row: `{id, source, severity, rule, title, names, doc}`.

- Each check's `state`: `none`, `running`, `deferred` (waiting for memory or another heavy KiCad job: `reason`), `fresh`, `stale` (the design
  changed since), `failed` (`reason`). Background runs leave KiCad's library checks out (`note`); `kicad.checks.run {full: true}` runs them too.
- A row pressed (or reached with up/down): `kicad.checks.focus {project, id}` selects and zooms its items in KiCad's editor for it (board for
  DRC, schematic for ERC) and lights them in the viewers. "Fix with agent": `kicad.checks.fix {project, id}` -> `{text}`, sent as the
  thread's next message. The chip in the editor's row ("DRC 12 · ERC 3") opens the panel.
- Another source: `checkrun::set_findings(hub, pro, source, findings: Vec<findings::Finding>, meta) -> Result<(), String>` (or the RPC
  `kicad.checks.set {project, source, findings, meta}`) replaces that source's findings for the project. `meta.label` names it ("Fab rules"),
  `meta.judged: true` marks every row of it "agent-judged" (with `meta.confidence`; put the datasheet page in a finding's `detail`). An empty
  finding `id` is made from its anchors (stable across runs when they carry KIIDs or names). It reads `stale` once the design on disk
  changes after the set, like DRC; set it again to refresh. `summary.sources.<source>` = `{label, state, counts, at, rev, meta, judged}`;
  the panel filters by source and severity and searches words over all of them.

## Adding a panel

Hub side (`native/host/src/kicad/<yours>.rs`): implement `mgmt::Panel` and call `mgmt::register(Arc::new(YourPanel))` from your `start`:

```rust
pub trait Panel: Send + Sync {
    fn id(&self) -> &'static str;          // "checks": the tab's id, the info key's middle part
    fn label(&self) -> &'static str;       // "Checks": the tab's words (the client translates)
    fn watch(&self, hub: &Arc<Hub>, project: &str);   // a client shows the project: publish (or refresh) for it
    fn opened(&self, hub: &Arc<Hub>, project: &str) { self.watch(hub, project) }   // the person opened the panel
}
```

Publish with `mgmt::publish(hub, "checks", project, json!({"badge": {"n": 3, "tone": "bad"}, ...}))` whenever the data changes (it is
bounded by the hub's info: 64 KiB). `mgmt::is_open(project, "checks")` says whether the panel is open there now.

Client side (`native/gpui/src/screens/mgmt_<yours>.rs`): `pub fn body(app: &mut Backplane, data: &Value, light: bool, cx: &mut Context<Backplane>) -> AnyElement`
draws the panel from the hub's value (`Value::Null` until it comes), and one line in `screens/mgmt.rs` `VIEWS`:
`View { id: "checks", body: super::mgmt_checks::body }`. The host draws the tab, the badge, the edge and the dock buttons. Its key is in
`keys.rs` (`mgmt-checks`, Alt+Shift+C).

## Keys

Alt+Shift+B (BOM), Alt+Shift+C (Checks), Alt+Shift+L (Lifecycle) open or shut a panel; they are in the key sheet and can be rebound.
While KiCad has the keyboard, its keys are KiCad's.

## Tests

`kicad::{mgmt, bompanel, bomedit, parts, parthist, lifepanel}`, `mcpx::health` (hub); `screens::{mgmt, mgmt_bom, mgmt_life}` (client);
`test/kicad_embed/embed_mgmt_e2e.sh` (stand-in parts server: `test/kicad_embed/lib/pcbparts_stub.py`).
