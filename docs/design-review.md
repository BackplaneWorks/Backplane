# Design review

Backplane's own review of a design, deterministic checks first. Code: `native/host/src/review/`; findings in lane E's model
(`kicad/findings.rs`: `{id, source, severity, rule, title, detail, anchors, fix}`), so the Checks list, the phones and "Fix with agent"
take them as they take DRC and ERC.

## Severity

`error` must change (the fab refuses it, or the order cannot be made), `warning` should change, `info` a note (a capability that costs
more, a value between the fab's limit and Backplane's recommendation, a board setup minimum the person may raise). In `review.md` and
the tool's answer: "must change", "should", "note".

## Sources (`review/checks.rs`)

- `drc`, `erc`: E's runs (kicad/checkrun.rs). A full or order review asks for them fresh and waits (5 minutes at most); what is not
  fresh then is listed as not checked.
- `fab`: the DRC rules' fab section missing or stale (fix: `fab_profile {action: "drc"}`); the person's rules that allow less than the
  fab (a conflict) or only what costs more (a note); board setup minimums under the fab's (notes: the person applies them from the Fab
  panel); the fab file's options against the fab (layer count, thickness and finish it offers, layers and thickness against the board);
  the board's size; the narrowest track, the smallest via (a microvia against the microvia minimum) and pad hole against the free and
  costs-more limits and the recommendation (tracks 0.127 mm, vias 0.3/0.45 mm).
- `bom`: the BOM the hub publishes (`mgmt.bom.<digest>`, lane D's): an MPN on every placed line; with D's stock, lifecycle and library
  fields, out of stock or short at the order's quantity (must change), NRND (should), EOL (must change), extended parts (note). Lines not
  checked yet are listed, never guessed; the parts service is never asked from here.
- `outputs`: the newest Gerber set in `mfg/`: every copper layer of the board, the outline, both masks, the drill files, and their smallest
  tools against the fab's smallest hole (laser files against the microvia minimum). A full or order review also exports the saved board
  again (kicad-cli, the board's own plot settings, into the hub's cache) and compares it with the set, layer by layer: written by the same
  KiCad, the whole drawing (dates left out); by another KiCad, whose apertures and zone fills are its own, only the outline's points, and
  the run says so (Hubble_DK's outputs, made by bp.4: the outline compared, 1.3 s).
- `assembly` (assembled rigid boards): parts on both sides (a note: the standard service), fiducials then, the placement file in the
  outputs against the board (parts missing, parts on the other side).
- `mech`: fab files of 3D-printed, CNC and sheet metal parts: their own DFM results (a SendCutSend part's failed rules with their sources).
  Geometry rules (walls, radii) need the model: they belong to the agent-judged review.

## Agent-judged checks (`review/judged.rs`)

Kept apart from the deterministic ones, never mixed in silently: a part's symbol and footprint against its datasheet.

- `design_review {scope: "judged"}` hands the agent the evidence: for each part worth it (ICs, connectors, transistors, crystals, anything
  with six pads or more, with an MPN), most pins first, 24 at most (`more` counts the rest): reference, value, MPN, maker, the datasheet
  link (the symbol's), the symbol's pins `[number, name, type]` from the schematic's own library copy, the footprint's pad count, numbers
  and smallest centre-to-centre distance from the board; and what to do (`how`): the parts service (pcbparts jlc_get_part, jlc_get_pinout,
  cse_get_kicad) and the datasheet, mismatches only, never a guessed pinout.
- `design_review_judged {findings: [{reference, pin?, severity, title, detail, page, confidence, fix?}], checked, unchecked}` records the
  answer: source `judged` in the Checks list (`meta.judged: true`, label "Datasheets"), each finding anchored to the part (and pin), with
  the datasheet page (required) and the confidence in its detail; a section of its own in the run (`judged`, kept while the design is the
  one judged) and in `review.md` ("Agent-judged (datasheets)"). Its must-changes join the order's issues.
- When: on request (the Fab panel's Datasheets sends the hub's request as the person's message in the pane's thread), and before an order
  (the manufacturing review's instructions: `design_review` scope order, then judged). Never on save.
- On copies: Hubble_DK hands 24 parts (3 more), from the nRF54LM20 (98 pins and pads) down; Collar main 5.

## A run (`review/mod.rs`)

`design_review {scope}` (every thread's tool): `full` (default) or `order` (DRC and ERC fresh) or `save` (file checks only). It keeps the run
in `.backplane/design-review.json` (`{at, scope, fingerprint, rev, took_ms, counts, by_source, skipped, verdict, findings}`), writes
`.backplane/review.md` and publishes it as info `review.<digest>` (findings cut at 300). On a copy hub nothing is written.

`order_review {action: "record"}` rests on it: the run for the design as it is now (the order fingerprint), else a run of scope `order`
first; its must-changes lead the issues and make the verdict "issues" whatever the agent says; `review.md` is the run's findings with the
agent's notes. The Buy gate is unchanged: a review older than the design is stale and asks again.

## When (`review/policy.rs`)

On demand (the tool); before an order (`order_review record`); on save, scope `save`, for a project shown in the last 30 minutes, after
1.5 s of quiet, one run at a time, not with under 400 MB free.

## Phones

All hub data: `review.<digest>` (the run: verdict, counts by source, what was skipped, findings), the Checks list's sources (`mgmt.checks.<digest>`
and `kicad.checks.list`, with `judged` rows marked), and the Fab panel's `review` (`mgmt.fab.<digest>`: the last run's verdict and counts,
`running`, `judge`: the request Datasheets sends).

## In KiCad: review markers (Backplane KiCad bp.13)

Where the open KiCad has `SetHostMarkers`, each review run also sends its findings (DRC and ERC excepted: KiCad draws its own) as one
named marker set, `review`, to each editor: the board's findings to the PCB editor, the schematic's to the schematic editor, worst
first, at most 400 each, placed on the finding's items (KIIDs) or its point. Markers are host state: nothing is written to the design,
its DRC/ERC results or its undo; a run with no findings clears the set. A click on one (ET_MARKER_CLICKED, set `review`) opens the
Checks panel and publishes `review.clicked.<digest>` {id, at} for the panel to show that finding. Older runtimes get nothing (no
command, no call). Linux-tested so far; nothing platform-specific in it.

## DRC second opinion (stock KiCad 10.0) and the zone-fill warning

Backplane KiCad bp.10 to bp.12 can miss a short inside a zone fill: its DRC can leave real fill copper out of the check, so a track
of another net inside a GND fill passes (in the CLI and the editor alike; the saved copper and Gerbers are right). bp.12.1 fixes it
and advertises the capability `drc-zone-guard`. Source `stockdrc` ("Stock DRC (second opinion)", review/stockdrc.rs):

- When the runtime lacks `drc-zone-guard` and the board has filled zones, one should-level finding says this KiCad build's DRC can
  miss a short inside a zone fill, and to get a second opinion before ordering. `order_review record` appends the same words to its
  summary, so the Buy flow's review never reads as clean on that DRC alone.
- On a full or order review (never on save; under a memory guard), when a stock KiCad 10.0.x `kicad-cli` is on the machine (one
  that is not a Backplane runtime: no manifest.json beside its bin/, not the runtime in use; `BACKPLANE_STOCK_KICAD_CLI` for tests),
  the board's .kicad_pro, .kicad_pcb and .kicad_dru are copied to the hub's cache, exported with the runtime's
  `project downgrade --target 10.0` (forced only when a feature must be dropped, and the dropped features named), and checked by
  stock `pcb drc --format json`. Each violation stock reports that the runtime's DRC did not (same type and items, or the same type
  at the same places; unconnected items, library checks and exclusions left out) is a finding: errors are must-change, warnings
  should. After a forced export, only a short stays a must-change; anything else becomes a should naming what was dropped. When
  stock ran and added nothing, the warning drops to a note: "Second opinion (stock KiCad 10.0.x DRC on the exported project): no
  additional violations". Stock staying quiet is said as that, never as the board being clean. The copy is removed after the run.
- With `drc-zone-guard` present the warning and the note go; the stock cross-check still runs when stock is installed.

Measured: Hubble_DK copy, bp.13 test build against stock 10.0.7, 534 vs 564 violations, no extras (the export dropped the custom
rule 'U1 fanout area', so a forced export). Linux-tested; the install paths for macOS and Windows are in, untested.
