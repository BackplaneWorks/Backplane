# e2e journeys for the final gate (docs/final-gate.json)

Scripted end-to-end journeys for GPUI, iOS and Android against the Rust hub, with frame measurements.

- `journeys.json` — the 12 gate journeys, client-agnostic steps + the capabilities each needs; `stories` holds the user-story journeys (from
  `stories.md`, `story: S04`) and the day-in-the-life walk (`story: day`), and `step_budgets_ms` the per-press budgets.
- `stories.md` — what a person does with Backplane, as short stories; each names the journey that drives it (or the gate journey that covers it).
- `CONTRACT.md` — step vocabulary, how every measurement is defined, what each client has today, and the requests (G1-G5) the GPUI client must meet for the blocked journeys.
- `lib/frames.mjs` (+ `frames.test.mjs`) — click -> next frame, frame times / jank, layout shift, dead controls, RSS slope. Pure; unit-tested.
- `lib/hubfx.mjs` — a seeded fixture hub (Rust or JS) with stand-in agents only.
- `lib/capture.mjs` — Xvfb + i3 + ffmpeg capture (Boat only).
- `adapters/{desktop,android,ios}.mjs (`--client gpui` loads desktop.mjs)` — one per client; a missing capability makes the journey `blocked`, never pass.
- `run.mjs` — the runner; writes `out/e2e/<client>/summary.json` with a gate mapping (pass / fail / open) and the capture paths, and
  `report.md`: per-press timing, the frictions ranked by harm, what is blocked. `--journey all` (gate) | `stories` | `day` | `story:S04` | `<id>`;
  `--copy-of HOME` runs them on a copy of a home (BACKPLANE_COPY=1; CONTRACT.md "Copy runs").
- `lib/report.mjs` (+ `report.test.mjs`) — step verdicts against the budgets and the harm ranking. `lib/seed.mjs` — the lived-in workspace.
  `lib/hubcheck.mjs` — the hub's own counts for `expect {hub}`. `rpc.mjs HOME METHOD [JSON]` — one call to a test hub by hand.
- Agents: `skills/backplane-e2e/SKILL.md` is the entry point (rules, commands, how to read the results).
- `bp-run.sh` — desktop-side launcher for h@backplane (Xvfb, own D-Bus; `scripts/boat-job.sh` no longer exists, so `boat-run.sh` is stale): `selftest`, or `run HUB GPUI [journey]`.

Rules: nothing runs on the desktop (no Xvfb, hub or client) without the person's word; Boat renders in software, so its frame numbers
validate the harness and are labelled software-rendered, they are not GPU evidence; simulated agents only; captures are handed to the
mechanics thread by path (`summary.captures`).

Status: harness written; GPUI runs the journeys its script contract supports (select, popup, type, send, wait) and reports the others
`blocked: G1-G5`; Android has the adb primitives (dump, tap, gfxinfo, dead-control sweep) but no step driver and no device on Boat; iOS is
`blocked: ios_driver`.

## Hub-level rows (any platform with bun and a built hub)
- `settings-walk.mjs HUB OUT [port]` — the Settings walk (S01-S14): every chip and form on every page the hub lists, applied, persisted over a hub restart. Writes `OUT/settings-walk.json`. The macOS and Windows lanes rerun it as is.
- `f-rows.mjs HUB OUT [port]` — F01-F04 through the data/1 socket.
- `cad-rows.mjs HUB OUT [port]` — C02: `kicad.path`, the CAD page, ERC/DRC and exports run by the hub (needs KiCad 10; set `KICAD_CLI`).
- `rig-hub.mjs HUB ROOT PORT` — a seeded fixture hub that stays up. The GPUI client needs a window manager (i3) on a bare Xvfb to present a frame.
All run with `NO_SCOPE=1 PERF_REPO=<repo root>` and bun.
