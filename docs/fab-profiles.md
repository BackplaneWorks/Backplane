# Fab profiles

What each fab (and its assembly service) can make, kept by the hub so boards are designed against a real fab's limits, not remembered
numbers. Code: `native/host/src/fab/`.

## The data (`fab/model.rs`)

A profile is one fab's capabilities, `schema` 1, with a `rev` that goes up with each stored change and the day it was `updated`.

- `pages`: each page or API it is read from, `{url, fetched, etag?, sha256?}`.
- `kinds`: per board kind (`pcb`, `flex`, `hdi`, `aluminum`, `copper_core`, `rogers`, `ptfe`, catalog.py's words) a list of values. A
  kind whose page lists only what differs `inherits` the rest from another kind (the rigid page's), and those values say `inherited`.
  For the fab Backplane orders from, `rogers` and `ptfe` inherit every manufacturing limit from the rigid page on purpose: that page
  lists "RF PCB: 1 oz copper, 2-layer RF PCBs with Rogers and PTFE cores" among its board types and gives them only their own sizes
  (590 x 438 mm, 3 x 3 mm); its track, drill and ring tables cover them. Its `rf-pcb-capabilities` and
  `high-frequency-pcb-capabilities` addresses serve the same page (checked 2026-11-13), so there is no other page to read.
- every value: `{key, value, unit, cost, when, page, source, fetched, confidence, note, preferred?}`.
  - `cost`: `free` is what the fab makes at its base price, `extra` what it makes for more (the manufacturing review says "costs more",
    never "error", for a design between the two).
  - `when`: the option tier it holds for: `layers`, `copper_oz`, `thickness_mm` (inclusive ranges), `finish`, `color` (lists). The most
    specific matching value holds (`model::select`), so a 0.8 mm ENIG board gets the 0.1 mm via for more while others get 0.15 mm.
  - `confidence`: `published` (read off the page), `derived` (computed from it: mil to mm, a ring from a diameter), `inherited`.
  - `note`: where on the page it is, in the page's words.
- `stackups`: published stackups with their plies (copper, prepreg, core: material, thickness, Er) for impedance work.
- `assembly`: per assembly service (`economic`, `standard`) the same kind of values: sides, package, pitch, board size, rails, fiducials.

A value that cannot be read again keeps its old value and date: never a guess.

## Where it is kept (`fab/store.rs`)

- `native/host/src/fab/baseline/<fab>.json`: the bundled baseline, as read when this Backplane was built, so a fresh install has profiles
  offline.
- `<home>/fab/profiles/<fab>.json`: the current one as gathered; `<home>/fab/history/<fab>/<rev>.json`: every stored revision.
- The newer (by `updated`, then `rev`) of the bundled and the stored one is the profile.

## A project's fab (`fab/project.rs`)

Picked once per project, in `.backplane/fab-profile.json` of the design root: `{fab, kind, options, rev_seen}`. A sidecar of Backplane's;
nothing goes into a KiCad file. Until picked: the fab Backplane orders from, with the kind and options of the project's first board
`*.fab.json`, else a rigid board with the KiCad board's copper layers and thickness, else a 2-layer 1.6 mm board. `choice.by` says which.

## Reading it

- Agents: the MCP tool `fab_profile {action: get|pick|seen, fab?, kind?, options?}` (every thread).
- Clients: `fab.profile {thread | project, action?, ...}` on the hub's socket; phones: `fab.profile` on data/1 (feature `fabprofile`),
  answering `{profile}`.
- The answer (`Fab.view`): `{fab, rev, updated, choice:{fab, kind, options, by, rev_seen, from}, limits:[{key, label, free?, extra?,
  inherited?, changed?}], stackups, assembly:{economic, standard}, pages, changed:[line], fabs, kinds}`; each of `free`/`extra` is
  `{value, unit, source, fetched, confidence, note?, preferred?}`. `changed` lists what differs from the revision the project last saw
  (`seen` marks the current one seen). In UI copy the fab is "the fab".

## DRC rules (`fab/drc.rs`)

`fab_profile {action: "drc"}` (and `fab.profile {action: "drc"}`) writes the fab's limits into the project's `<project>.kicad_dru` as a
marked section, merged, never overwritten:

- Whole-line `#` comments `# BEGIN Backplane fab profile` … `# END Backplane fab profile`, right after the file's `(version 1)` (a new
  file gets one). Regenerated in place; every byte outside the markers stays as the person wrote it (a test proves the round trip).
- KiCad lets later rules win, so the person's own rules, after the section, keep precedence where they match.
- Where the fab's rules sit: ahead of every rule of the person's. The fab's limits are the floor; a rule of the person's that sets the
  same constraint decides, stricter or not, and one that allows less than the fab is a conflict (below), said and never silently won.
- With the board open in KiCad the rules go through it (`EditCustomRules`, by rule name: a replaced rule keeps its place). A KiCad with
  `rules-insert-position` (bp.13 batch 4) is sent `insert: RIP_FIRST`: every fab rule goes, ahead of the person's; when one is new or
  one stands after the person's (an older runtime's write), all of them are removed and sent again in one request, in the plan's
  order, so they move. An older KiCad appends a new name after everything else, where it would win: there a new fab rule whose
  constraint one of the person's rules also sets is left out and named (`drc.left_out`). `current` follows the same choice (and, with
  the capability, wants no fab rule after the person's).
- A board setup minimum (the .kicad_pro's `board.design_settings.rules`) already as strict as the fab's gets no rule, only a comment:
  the project wins. One under it gets the rule and is listed in `drc.suggestions` for the person to approve; Backplane never rewrites
  the board setup itself.
- Clearance is written only between net classes whose own clearance is under the fab's, so no net class is ever loosened.
- The person's rules that allow less than the fab are listed in `drc.conflicts`, never changed.
- `tier` (picked with the project, default `free`) chooses the base-price limits or what the fab makes for more.
- A suggestion the person approves is applied by the Fab panel's Apply (`fab.profile {pro, action: "apply", key}`; agents are refused
  and name it instead, `fab/setup.rs`): one board minimum raised to the fab's through KiCad's API (GetBoardDesignRules, then
  SetBoardDesignRules with the constraints alone, never lowering one). With the person's KiCad up it is set there and they save the
  board (KiCad keeps design rules out of its undo history: the answer says what it was); otherwise (the headless KiCad has no
  design-rules API: it answers AS_UNHANDLED), only while no thread holds the board's lock, the headless KiCad is retired and the one
  number is changed in the .kicad_pro, every other byte kept.
- A pick regenerates a section that is already there. The view's `drc` says `{file, present, current, suggestions, conflicts}`.
- Everything in the section loads in stock KiCad: `test/fab/stock_drc.sh [kicad-cli] [demo project]` writes it for each board kind
  and tier into a copy of a KiCad demo with a sentinel rule after it and runs stock `kicad-cli pcb drc` (KiCad drops a whole rules file
  that does not load, so the sentinel firing proves it loaded); last run on KiCad 10.0.7, all six cases.

## Gathering (`fab/gather.rs`)

Fabs with a profile and an adapter: `jlcpcb` (the fab Backplane orders from: rigid, flex, HDI and metal-core pages, assembly, impedance
stackups) `pcbway` (its standard PCB table), `oshpark` (its two- and four-layer service pages, with its 4-layer stackup) and `eurocircuits`
(its pattern and drill classes: the free values are what every service makes, the costs-more ones its pool services' finest); pick one with `fab_profile {action: "pick", fab: "<id>"}` to design against it. Per fab an adapter: its published pages and, per value of the profile, where on which page it is (a table row's label, the words
before the number, which number after them, a factor for a mil or a ring). More fabs are a baseline file plus an adapter.

- `refresh` GETs each page with curl (`proc::Kind::Net`, a second apart, a Backplane user agent, https only, 8 MB at most); nothing of
  the person's is ever sent. A copy hub, a test hub (stand-in agents) or `BACKPLANE_FAB_OFFLINE=1` never fetches.
- Every value it reads gets today's date; one it cannot read (a page that did not come, a row that moved) keeps its old value and
  date and is listed in the report's `missing`. A changed value bumps the revision; the history keeps every one. Stackups are read
  whole from the impedance page (plies, thickness, Er from the page's own table; a prepreg its table does not name has no Er).
- A table a fab publishes only as an image (eurocircuits' classification table) was read into the baseline once by hand; its page
  records the image's address (`pages.<id>.image`). A gather reads the page's text values as usual and watches that address: a new
  one is said in the report's `notes` ("the table image is now …"), and the values stay as they were read, with their dates, until the
  baseline is read again. Never a guess, never OCR.
- The hub runs it 3 minutes after start and then daily; `fab_profile {action: "refresh"}` and `fab.profile {action: "refresh"}` run
  it now. A change is pushed to clients as `{t: "fab.changed", fab, rev, changes, keys, projects}` (the open projects on that fab);
  a project's view marks its values `changed` until it is `seen`. The last run is `<home>/fab/gathered-<fab>.json`, shown as the
  view's `gathered`.
- Tests read saved copies of the pages (`test/fab/fixtures/<fab>/`, scripts and styles stripped): every rule finds its value and the
  values are the bundled baseline's; a changed page changes the value and the revision; a missing page or row keeps the old.
  `fab_gather_live_refresh` (ignored; `FAB_LIVE_HOME=<temp home>`) runs a real one by hand: on 2026-11-12, 121 values read, none
  missing, none different from the baseline.
- `fab_model_no_baseline_value_is_older_than_its_bound` fails once any bundled value or stackup was read more than 120 days ago
  (a new, copy or offline hub designs against the bundled baseline until its first refresh): read the pages again and update it.
- The review adds one note (`fab_stale`, info) naming each of the board's values read more than 120 days ago (`model::MAX_AGE_DAYS`).

## The Fab panel (`fab/panel.rs`)

A management panel beside the design (docs/MANAGEMENT.md), info `mgmt.fab.<digest>`: `{badge, fab, rev, updated, by, tier, board,
gathered, missing, drc: {state: current|stale|none|noproject, file}, suggestions, conflicts, changed, limits: [{key, label, free: {text,
source, fetched, confidence, note}, extra, changed, inherited}], stackups}`. The badge counts conflicts (bad), suggestions, changes since
the project last looked and a stale section (warnings). Its buttons are `fab.profile {pro, action}` (`drc`, `apply` with `key`, `seen`,
`refresh`); the hub publishes the panel again after each, and after a gather that changed a profile.

## The review prompt

The manufacturing review (`agents/prompt_text.rs`, `ORDER_PROMPT_REVIEW`) takes the fab's limits from `fab_profile`, not from numbers of
its own. Where Backplane recommends more than the fab's limit (tracks and gaps 0.127 mm, vias a 0.3 mm hole in a 0.45 mm pad, copper
0.3 mm from the edge) a design between the two is a note, not a must-change.

## Stackup and impedance (`fab/stackup.rs`)

The view's `stackup`: `{chosen, by: picked|proposed, thickness_mm, source, fetched, needs: {single, diff, gap_mm, given}, layers,
proposals, estimate}`.

- `proposals`: the published stackups that fit the board's layer count and thickness, best first, each `{id, default, feasible, why,
  outer_50_mm}`. With impedance needs (the project's options `impedance`, `diff_impedance`, `diff_gap_mm`), one whose every width is at
  or over the fab's free track minimum (and whose pair gap meets its spacing) comes first, then the narrowest 50 Ω outer line; without
  needs, the fab's default. It is a proposal: the person takes it (the panel's Use, `fab.profile {pro, action: "stackup", id}`), or an
  agent picks it (`options.stackup`) once they agree.
- `layers`, per copper layer top to bottom (`F.Cu`, `In1.Cu`, … `B.Cu`): `{layer, copper_mm, kind: microstrip|stripline, above, below,
  widths}`, `above`/`below` the neighbouring copper layer with the dielectric between (`h_mm`, `er`: series Er of its plies), `widths`
  the trace width per impedance (`"50"` single-ended, `"90d"` a pair at `gap_mm`). This is what the floorplanner and routing read:
  reference-plane adjacency and impedance widths per layer.
- The widths are closed-form estimates without solder mask (microstrip: Hammerstad and Jensen with the copper's thickness; stripline:
  IPC-2141; pairs: the usual coupling factors), within a few percent of the fab's calculator, which stays the reference for an order
  with impedance control (the 4-layer 7628 stackup's 50 Ω outer line: 0.367 mm).
