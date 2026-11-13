# Routing floorplanner: Backplane's side

The fork owns the plan itself (`<board>.kicad_plan`, the overlay and editing tools in KiCad, `kiapi.backplane.plan`: the fork's
`docs/backplane/floorplanner.md`). Backplane's side (`native/host/src/plan/`): agents' tools, the person's approval, events, the Plan panel,
the viewer's bands, and what agents are told.

## Agents' tools (`plan/mod.rs`)

`plan_get`, `plan_propose`, `plan_update`, `plan_approve`, `plan_route`, `plan_stop`, `plan_verify`: the fork's commands through
`kicad_ipc` (the open KiCad with the agent's own actor `agent:<thread>`, its holds and pauses), only where the runtime has `routing-plan`
(bp.11+). bp.12's background server answers `AS_UNHANDLED` to them, so the board must be open in KiCad; the tools say so.

- Every answer carries `changes`: what changed in the plan since that thread last read it (items added or removed, status, the person's
  edits, layer, lock, progress, layers judged, notes and constraints added). `plan_get` answers `{summary, changes, plan}`.
- Approval is the person's alone. An agent's `plan_approve` asks them (kept as an open request, said in the thread, shown in the Plan
  panel) and approves nothing. `plan.approve {pro|path, layer?, ids?, approve}` (the Plan panel, a phone) sends KiCad's `ApprovePlan` as the
  person (`coedit::Who::Person`).
- The audit gate is the hub's too: `plan_route` reads the plan first and refuses unless what it would route is approved (the ids given, or
  with none at least one approved item) by the fork's own rule: the item approved, its group and its layer not rejected. Tested
  (`plan_gate_*`) and end to end (`test/kicad_embed/embed_plan_e2e.sh`, bp.12: propose, refused, ask, the person approves, the agent is
  told, the approved corridor routes 2/2 with 0 DRC).

## Events (`plan/events.rs`)

KiCad's `ET_PLAN_CHANGED` (the editor's event stream, `kicad/embed.rs`) is taken in after 1.5 s of quiet (a drag is one change). The plan is
published as info `plan.<digest>` (`{board, summary, items, asks, revision}`) and in the Plan panel's data; a thread that asked for
approval is sent a message once what it asked about is judged ("The person judged c-k in the routing plan: approved c-k ...").

## The Plan panel (`plan/panel.rs`, `screens/mgmt_plan.rs`)

Info `mgmt.plan.<digest>`: `{badge, revision, live, awaiting, blocked, routing, asks, empty, layers: [{layer, role, status, stackup,
done, items: [{id, kind, name, color, nets, state, locked, order, routed, total, problems, reference}]}], draw: {bands, regions}}`.
`state` is proposed, edited (by the person), approved, routing, routed, verified, rejected, failed or blocked (the audit's or DRC's
problems). The plan is the open KiCad's (live, with derived data) when the board is open there, else the saved file: no background KiCad is
started to show it. Buttons: Approve / Reject per item, Approve layer / Reject, Approve all, Stop routing, Hide done; a row selects and
zooms its nets and parts in KiCad (`plan.focus`). Drawn in the floorplanner's look: proposed translucent and dashed, edited solid with ✎,
approved solid with ✓, routing n/m, routed faded, blocked red.

## The viewer

Backplane's board viewer draws `draw` read-only over the board: plane regions as filled outlines, corridors as bands along their path at
their width in their group's colour, vias as diamonds, by state (`plotview::plan_alpha`). Steering is in KiCad.

## Phones

All hub data: `mgmt.plan.<digest>` (the panel and the bands) and `plan.<digest>`. Over data/1 (feature `plan`): `plan.approve`, `plan.stop`,
`plan.focus` with `pro`, the same hub calls as the desktop's.

## What agents are told

A project with a KiCad board on Backplane KiCad bp.11+ gets `<backplane_routing_plan>` (`plan::guidance`): on boards with 4 or more copper
layers or more than 60 nets, plan before copper: the stackup from `fab_profile` (reference planes and impedance widths per layer), propose,
ask, end the turn, respect the person's edits, route only what was approved, inside its corridor.

## The person's edits in KiCad's plan view (bp.13)

`test/kicad_embed/embed_plan_person_e2e.sh` drives KiCad's own plan view with real X input (XTest), as a person would: the editor
full size, a click on the board to give KiCad the keyboard, Alt+P, Home; the corridors are found on the screen by their group's
colour (KiCad's GL canvas is no X window of its own), then c-p is clicked and accepted (A), locked (L) and widened (W), and c-q is
clicked and rejected (R). The hub sees each as the person's: `plan_get`'s `changes` (approved, locked, edited by the person, rejected,
with the person's constraints), the Plan panel's data live, and the agent that asked for approval is woken with the decision. First
run on the bp.13 test build (d5eaa3ae44): 11/11, and embed_plan_e2e 11/11. Layer approval from KiCad's context menu is not driven yet.
Linux (Xvfb) only, as every embedded-editor e2e.
