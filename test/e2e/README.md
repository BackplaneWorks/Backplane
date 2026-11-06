# e2e journeys for the final gate (docs/final-gate.json)

Scripted end-to-end journeys for GPUI, iOS and Android against the Rust hub, with frame measurements.

- `journeys.json` — the 12 gate journeys, client-agnostic steps + the capabilities each needs.
- `CONTRACT.md` — step vocabulary, how every measurement is defined, what each client has today, and the requests (G1-G5) the GPUI client must meet for the blocked journeys.
- `lib/frames.mjs` (+ `frames.test.mjs`) — click -> next frame, frame times / jank, layout shift, dead controls, RSS slope. Pure; unit-tested.
- `lib/hubfx.mjs` — a seeded fixture hub (Rust or JS) with stand-in agents only.
- `lib/capture.mjs` — Xvfb + i3 + ffmpeg capture (Boat only).
- `adapters/{gpui,android,ios}.mjs` — one per client; a missing capability makes the journey `blocked`, never pass.
- `run.mjs` — the runner; writes `out/e2e/<client>/summary.json` with a gate mapping (pass / fail / open) and the capture paths.
- `boat-run.sh` — desktop-side launcher that only starts Boat jobs: `selftest`, or `run HUB GPUI [journey]`.

Rules: nothing runs on the desktop (no Xvfb, hub or client) without the person's word; Boat renders in software, so its frame numbers
validate the harness and are labelled software-rendered, they are not GPU evidence; simulated agents only; captures are handed to the
mechanics thread by path (`summary.captures`).

Status: harness written; GPUI runs the journeys its script contract supports (select, popup, type, send, wait) and reports the others
`blocked: G1-G5`; Android has the adb primitives (dump, tap, gfxinfo, dead-control sweep) but no step driver and no device on Boat; iOS is
`blocked: ios_driver`.
