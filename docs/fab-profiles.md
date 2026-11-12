# Fab profiles

What each fab (and its assembly service) can make, kept by the hub so boards are designed against a real fab's limits, not remembered
numbers. Code: `native/host/src/fab/`.

## The data (`fab/model.rs`)

A profile is one fab's capabilities, `schema` 1, with a `rev` that goes up with each stored change and the day it was `updated`.

- `pages`: each page or API it is read from, `{url, fetched, etag?, sha256?}`.
- `kinds`: per board kind (`pcb`, `flex`, `hdi`, `aluminum`, `copper_core`, `rogers`, `ptfe`, catalog.py's words) a list of values. A
  kind whose page lists only what differs `inherits` the rest from another kind (the rigid page's), and those values say `inherited`.
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
- A board setup minimum (the .kicad_pro's `board.design_settings.rules`) already as strict as the fab's gets no rule, only a comment:
  the project wins. One under it gets the rule and is listed in `drc.suggestions` for the person to approve; Backplane never rewrites
  the board setup itself.
- Clearance is written only between net classes whose own clearance is under the fab's, so no net class is ever loosened.
- The person's rules that allow less than the fab are listed in `drc.conflicts`, never changed.
- `tier` (picked with the project, default `free`) chooses the base-price limits or what the fab makes for more.
- A pick regenerates a section that is already there. The view's `drc` says `{file, present, current, suggestions, conflicts}`.
- Everything in the section loads in stock KiCad: `test/fab/stock_drc.sh [kicad-cli] [demo project]` writes it for each board kind
  and tier into a copy of a KiCad demo with a sentinel rule after it and runs stock `kicad-cli pcb drc` (KiCad drops a whole rules file
  that does not load, so the sentinel firing proves it loaded); last run on KiCad 10.0.7, all six cases.
