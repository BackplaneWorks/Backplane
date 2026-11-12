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

- `source`: `file` (the saved schematic and every sheet it reaches, read again within a second of a save).
- Lines group parts with the same value, footprint, MPN and DNP (`kicad/bom.rs group`), in natural order of their first reference; `id` is
  that reference. `fab` is the fab's part number (the symbol's `LCSC` field). At most 400 lines; `more` counts the rest.
- The badge counts lines without an MPN (DNP lines excepted) once some line has an MPN or a fab number.
- A press on a line selects its parts in the open KiCad and lights them in the viewers (`editor.kicad.select`).

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

`kicad::mgmt`, `kicad::bompanel` (hub), `screens::mgmt`, `screens::mgmt_bom` (client); `test/kicad_embed/embed_mgmt_e2e.sh`.
