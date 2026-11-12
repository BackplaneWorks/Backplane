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
