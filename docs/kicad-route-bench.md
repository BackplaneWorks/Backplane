# The routing benchmark: SketchRoute against the hub's router

What routing costs and how good it is, with no model in the loop: the fork's `kiapi.backplane.SketchRoute` (routing inside
KiCad, on KiCad's own collision rules) against the hub's grid router (`native/host/src/kicad/kroute.rs`, what `kicad_edit`'s
`route` runs today). `test/kipc_bench/routebench.py` builds the cases, runs both and scores them with KiCad's own DRC.

## Cases

Every case is a copy of a board with the copper of some nets taken up. Nothing is ever read from or written to a person's
project.

| case | board | nets | what it is |
|---|---|---|---|
| `route_leds` | StickHub (2 layers) | 7 | the token benchmark's LED task, its net class already set |
| `route_drv` | StickHub | 7 | the LED driver nets: bottom to top, three pins each |
| `hubble_route` | Hubble_DK (8 layers, laser micro vias) | 4 | the token benchmark's debugger nets |
| `stickhub_all` | StickHub | 42 | every net but the pours' own, 99 connections |
| `collar_dense` | Collar main board (4 layers, 0.4 mm parts) | 23 | its signal nets, 27 connections |
| `hubble_known` | Hubble_DK | 29 | the nets its designer finished, 47 connections |
| `hubble_dense` | Hubble_DK | 49 | signal nets the designer has **not** finished (one of the 49 is complete on the board; the rest are escape stubs): no known answer |

`hubble_dense` is kept to see what a router adds to an unfinished board. It is not part of the completion target: nobody has
routed those nets.

## What is measured

- **done**: nets with nothing unconnected in KiCad's DRC afterwards.
- **vias**, and **ratio**: the routed nets' track length over the shortest tree through their pads.
- **drc+**: DRC findings the routes added, by type, against the case before routing (pours refilled for both).
- **margin**: the largest extra clearance over the rule the new copper keeps (0 means it runs at the rule).
- **route s**: the routing call alone. SketchRoute: the IPC call, with the router's own `elapsed_ms` beside it. The hub:
  `kicad_edit` with `check: false`, which still opens the board, refills and saves.

## Result

2026-11-17, one machine, under load from other builds (load average near 78), so the seconds are comparable with each other
and not much else. `old` is SketchRoute as released in bp.13.1; `new` is the fork branch `impl/sketch-speed-bp13`
(`ce4f2fd2b9`); `neck` is the same with `min_track_width` set to the board's own least track width.

| case | router | done | vias | ratio | drc+ | route s |
|---|---|---|---|---|---|---|
| `route_leds` | old | 5/7 | 2 | 1.85 | 1 | 0.30 |
| | **new** | **7/7** | 0 | 1.068 | 0 | **0.05** |
| | hub | 7/7 | 0 | 1.066 | 0 | 2.2 |
| `route_drv` | old | 2/7 | 6 | 1.16 | 0 | 0.65 |
| | **new** | **7/7** | 9 | 1.301 | 0 | **0.08** |
| | hub | 7/7 | 7 | 1.212 | 0 | 6.3 |
| `hubble_route` | old | 2/4 | 1 | 1.42 | 0 | 0.99 |
| | **new** | **4/4** | 10 | 1.302 | 0 | **0.18** |
| | hub | 4/4 | 10 | 1.193 | 0 | 9.6 |
| `stickhub_all` | old | 22/42 | 27 | 1.59 | 8 | 10.9 |
| | **new** | 34/42 | 48 | 1.513 | 15 | **3.5** |
| | hub | 34/42 | 32 | 1.202 | 16 | 100 |
| `collar_dense` | old | 17/23 | 33 | 1.34 | 0 | 17.2 |
| | **new** | **22/23** | 55 | 1.367 | 0 | **4.5** |
| | hub | 20/23 | 37 | 1.188 | 0 | 318 |
| `hubble_known` | old | 21/29 | 20 | 1.73 | 1 | 30.7 (its time limit) |
| | new | 25/29 | 32 | 1.216 | 1 | 8.6 |
| | **neck** | **29/29** | 34 | 1.209 | 1 | **3.9** |
| | hub | none: the call failed after more than ten minutes (`AS_UNHANDLED ... BeginCommit`) | | | | |
| `hubble_dense` | old | 9/49 | 13 | 1.33 | 7 | 31 (limit) |
| | new | 33/49 | 140 | 1.167 | 11 | 30 (limit) |
| | hub | none: the same failure after 35 minutes | | | | |

Read as:

- **Speed.** Against the released SketchRoute: 6 to 12 times on the three benchmark tasks (the router's own time: 228 to 25 ms,
  601 to 56 ms, 934 to 125 ms), 3 to 4 times on the dense boards while routing more. Against the hub's router: 40 to 80 times.
  The 10 times aim is met on two of the three tasks and not on the dense boards.
- **Completion.** All three benchmark tasks and `hubble_known` are complete. `collar_dense` is one net short (`/I2C_SCL`, which
  the designer brought out with a via in the pad); the hub's router is three short. `stickhub_all` is eight nets short for
  both routers. So 100 percent is met on four of six cases with a known answer.
- **DRC.** Nothing added on the tasks and on Collar. `hubble_known`: one more island of a pour that is already in three
  islands on that board. `stickhub_all`: 15 solder mask bridges (the hub's router: 16); the case has no via left to copy, so
  new vias are not tented as the board's were.
- **Quality.** Equal on `route_leds`. Elsewhere the hub's routes are shorter (1.19 to 1.21 against 1.30) and on the two-layer
  boards use fewer vias. This is the gap that is left.

## What changed, one step at a time

Each step was kept only when it was faster or more complete with no DRC loss. Router time in ms, connections routed.

| step (fork commit) | effect |
|---|---|
| Obstacle field (`2e86239415`): 80 to 88 percent of the time was KiCad working out the clearance again for every probe; it is asked once per obstacle, kept per millimetre tile | same copper; `route_leds` 198 to 59, `route_drv` 635 to 108, `hubble_route` 1288 to 177, `collar_dense` 11535 to 2541 |
| Free pad exits, cheapest route of a window, search from the end (`9b94858b96`): most failures were searches that never started | `route_leds` 5 to 7 of 7 with no via, `route_drv` 6 to 14 of 14, `hubble_route` 3 to 5 of 5 |
| The board's own via kinds, made like its vias; no track on plane layers (`286e1ee897`) | `hubble_route`: 5 through vias to 10 micro vias, 10 DRC findings (stacks not filled) to none |
| Via checks in the field (`1e40d607ec`) | same copper; `route_drv` 149 to 93, `hubble_route` 343 to 111, `collar_dense` 4546 to 2067 |
| Micro via in the starting pad; search without exits first (`abdcad2fc4`) | `hubble_dense` 45 to 57 of 81, `collar_dense` 2067 to 1207 |
| Rules by place; the board's via rules; a via not yet placed is its own kind to the rules (`fb760827ce`) | `collar_dense` 22 to 23 of 27; no via rule broken where a smaller via is allowed only inside a part's fanout |
| `min_track_width` (`20a051222f`) | `hubble_known` 41 to 47 of 47 (6 narrowed) |
| Rip-up and retry; via in pad where the board does it; bundles fall back (`714cd61798`) | `collar_dense` 23 to 26 of 27 |
| A slotted pad hole is drilled (`c17d216c2d`) | the fork's whole-board test: a via 0.09 mm from an oval mounting hole, gone |

The fork's own whole-board test (StickHub, every track removed): 118 of 124 connections in 11.5 s with the field off, 124 of
124 in 0.8 s now, DRC clean.

Tried and not kept: a cost for cutting another net's pour (no fewer islands, longer routes); joining a multi-pin net's next
connection to the nearest routed copper instead of the pad the ratsnest names (no shorter, one dangling track); an unweighted
search (`collar_dense` 23 of 23 and `stickhub_all` 36 of 42, but twice the time and longer routes elsewhere); more rip-ups per
route (no more routed).

## Running it

```sh
SB=$BP_TMP/sketchbench; T=test/kipc_bench
python3 $T/routebench.py mk H2H_FX HUBBLE_COPY COLLAR_COPY CLI $SB/cases       # the cases, from mkfix2.py's fixtures and copies
python3 $T/routebench.py sketch CLI FORK/api/proto $SB/cases $SB/out --tag new [--neck] [--reps 3]
python3 $T/routebench.py hub MCP_URL $SB/cases $SB/out --tag hub/a             # a held turn of a benchmark hub (hub.mjs, HOLD)
python3 $T/routebench.py score CLI $SB/cases $SB/out --tags new,hub/a
```

The SketchRoute arm needs `pynng` and `protobuf` (a venv) and `protoc`. The hub's arm routes inside the hub's project, so its
tag is a folder under the held turn's project (`hub/a`).

## Open

- Route quality against the hub's router: length ratio and via count.
- `stickhub_all` and the last Collar net.
- A new via on a board with none to copy takes KiCad's default tenting, not the board's.
- The hub's router cannot finish the Hubble dense cases at all; `kicad_edit` should not be given that many nets today.
