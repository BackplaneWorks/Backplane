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
  tools against the fab's smallest hole (laser files against the microvia minimum).
- `assembly` (assembled rigid boards): parts on both sides (a note: the standard service), fiducials then, the placement file in the
  outputs against the board (parts missing, parts on the other side).
- `mech`: fab files of 3D-printed, CNC and sheet metal parts: their own DFM results (a SendCutSend part's failed rules with their sources).
  Geometry rules (walls, radii) need the model: they belong to the agent-judged review.

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

`review.<digest>` is hub data: the verdict, counts by source, what was skipped and the findings, as the Checks list will show them once E's
list takes these sources (`kicad.checks.set`).
