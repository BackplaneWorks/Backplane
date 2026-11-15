# The KiCad-IPC token benchmark

What an agent spends to make a KiCad edit through Backplane, measured with real turns on a real headless KiCad
(`test/kipc_bench`, whose README says how to run it). Eight edits a person could ask for (move and rotate parts, set
schematic fields, a net class, place symbols, a track and a via, a zone, custom rules, fix a DRC finding), each on a
fresh copy of a demo project, each checked item for item against a reference edit.

## Result

Before: `kicad_ipc` alone (raw protobuf commands, a lock, a commit and a save by hand). After: `kicad_edit`
([kicad-edit.md](kicad-edit.md)), loaded up front in a design project. Model `claude-fable-5-2`, medium effort, medians
per task; before is 3 runs a task, after is 6 (2026-11-14, Backplane KiCad 10.99.0-bp.12.1).

| task | passes | context tokens | output tokens | tool calls | wall s | cost $ |
|---|---|---|---|---|---|---|
| move_rotate | 3/3 → 6/6 | 960,532 → 83,069 | 6,088 → 738 | 20 → 2 | 94 → 19 | 1.36 → 0.54 |
| sch_fields | 3/3 → 6/6 | 2,298,400 → 83,874 | 25,141 → 815 | 37 → 2 | 266 → 18 | 3.51 → 0.55 |
| netclass | 3/3 → 6/6 | 1,064,066 → 124,305 | 7,743 → 735 | 28 → 2 | 114 → 20 | 1.33 → 0.54 |
| sch_place | 3/3 → 6/6 | 1,947,974 → 83,310 | 12,226 → 895 | 34 → 2 | 180 → 20 | 1.95 → 0.55 |
| track_via | 3/3 → 6/6 | 518,367 → 103,591 | 3,119 → 734 | 15 → 2 | 68 → 19 | 0.80 → 0.54 |
| zone | 3/3 → 6/6 | 1,011,852 → 82,810 | 7,354 → 644 | 22 → 2 | 120 → 18 | 1.30 → 0.53 |
| rules | 3/3 → 5/5 | 912,537 → 124,135 | 5,659 → 728 | 24 → 2 | 91 → 19 | 1.11 → 0.54 |
| drc_fix | 3/3 → 4/4 | 582,287 → 127,124 | 3,148 → 992 | 15 → 3 | 65 → 25 | 0.85 → 0.58 |
| **all (sum of medians)** | 24/24 → 45/45 | 9,296,015 → 812,218 | 70,478 → 6,280 | 195 → 17 | 996 → 158 | 12.22 → 4.36 |

Context is input plus cache writes plus cache reads over the turn's requests: every request carries about 40 thousand
tokens of prompt and tool definitions, so the number of requests is what moves it. An edit is now two or three requests where it was 15 to 37.

Of the 48 runs after, 3 were cut short by the benchmark account's usage limit and are left out (rules 1, drc_fix 2);
the other 45 passed.

## Against editing the files directly

The same eight edits, the same check, made two ways on `claude-opus-5-5` (medium effort, 3 runs a task, medians, 2026-11-15,
Backplane KiCad 10.99.0-bp.12.1). Raw file: no MCP servers, so only the CLI's own file and shell tools, KiCad not running,
and the prompt's KiCad editing lines replaced by "edit the design files directly; kicad-cli runs DRC, ERC and upgrades"
(`KIPC_BENCH_NOMCP=1`, `KIPC_BENCH_PROMPT=prompts/rawfile.json`). `kicad_edit`: the hub as on main at c6e665e62.

| task | passes | context tokens | output tokens | tool calls | wall s | cost $ |
|---|---|---|---|---|---|---|
| move_rotate | 2/3 → 3/3 | 369,533 → 129,959 | 4,377 → 820 | 8 → 2 | 55 → 17 | 0.45 → 0.30 |
| sch_fields | 3/3 → 3/3 | 197,422 → 175,408 | 2,178 → 846 | 4 → 3 | 24 → 16 | 0.34 → 0.32 |
| netclass | 2/3 → 3/3 | 152,271 → 173,360 | 1,710 → 875 | 3 → 3 | 22 → 17 | 0.30 → 0.31 |
| sch_place | 3/3 → 3/3 | 377,440 → 218,546 | 3,705 → 1,142 | 8 → 4 | 55 → 22 | 0.44 → 0.33 |
| track_via | 3/3 → 3/3 | 190,989 → 129,351 | 1,766 → 644 | 4 → 2 | 25 → 15 | 0.31 → 0.29 |
| zone | 2/3 → 3/3 | 371,014 → 129,334 | 3,384 → 698 | 8 → 2 | 46 → 15 | 0.43 → 0.29 |
| rules | 3/3 → 3/3 | 149,312 → 173,160 | 1,237 → 872 | 3 → 3 | 19 → 16 | 0.28 → 0.31 |
| drc_fix | 3/3 → 3/3 | 153,117 → 177,398 | 1,153 → 1,170 | 3 → 3 | 19 → 22 | 0.29 → 0.33 |
| **all (sum of medians)** | 21/24 → 24/24 | 1,961,098 → 1,306,516 | 19,510 → 7,067 | 41 → 22 | 265 → 141 | 2.84 → 2.48 |

The three raw file runs that failed each said they were done. Two are wrong edits: a net class with via sizes nobody asked
for (netclass 1), and J9 rotated in the file so that its pads no longer match its library footprint (DRC
`lib_footprint_mismatch`, move_rotate 1). The third is a difference, not a mistake: zone 1 refilled with `kicad-cli pcb drc
--refill-zones --save-board`, and that fill has no clearance finding where the reference's fill (KiCad's refill over IPC)
has one. Raw file editing wins on the small edits that are one block of text (netclass, rules, drc_fix); `kicad_edit`
on the ones that need geometry or KiCad's own rules (rotating a part, a zone refill, placing a symbol). The raw file prompt
is shorter and carries no MCP tool definitions, so each of its requests reads a little less: the context gap is a floor.

These are Opus 5.5 numbers; the tables above are Fable 5.2, so compare within a table, not across them.

## Design work (pilot, 1 run a task)

Realistic edits, where many results are right, so a run is held to rules written before any run (`checks.py`), not to
one reference: DRC and ERC with nothing new, the netlist, the spec's places and widths, and nothing else moved. Fixtures
from `mkfix2.py` (`tasks2.json`; `KIPC_BENCH_TASKS` picks the task file): `sch_ldo` (an AMS1117 with its caps and power
symbols laid out in a free area of pic_programmer's root sheet), `route_leds` (StickHub's seven LED nets taken up: a net
class, the routes, a refill), `place_decap` (eight decoupling caps parked off StickHub, placed again within 2 mm of their
U1 pins), `swap_fp` (nine pic_programmer footprints to a shorter pitch, schematic and board, re-routed), `hubble_route`
(four debugger nets on a copy of Hubble_DK, an 8-layer board part way routed). Opus 5.5, medium effort, 2026-11-15,
one run a task each way, so read it as a first look, not a median.

| task | raw file | `kicad_edit` |
|---|---|---|
| sch_ldo | pass, 1,446,190 tok, 26 calls, 214 s, $1.02 | pass, 2,238,997, 37, 165 s, $1.27 |
| route_leds | pass, 1,071,396, 20, 161 s, $0.96 | pass, 2,298,611, 27, 408 s, $1.58 |
| place_decap | fail, 3,247,644, 42, 584 s, $2.04 | pass, 1,914,940, 25, 389 s, $1.52 |
| swap_fp | pass, 1,553,667, 23, 247 s, $1.45 | pass, 8,019,421, 60, 640 s, $5.38 |
| hubble_route | pass, 1,330,239, 20, 257 s, $1.47 | pass, 1,995,058, 19, 272 s, $2.04 |
| **all** | 4/5, 8,649,136, 131, $6.94 | 5/5, 16,467,027, 168, $11.79 |

What the transcripts show:

- The cost is geometry, and both sides pay it the same way: Python over the board file (one `kicad_edit` route_leds run
  installed shapely, drew the area to a PNG and looked at it) and `kicad-cli pcb drc`, 14 to 28 shell calls a task on
  either side. `kicad_edit`'s rows have no shapes (pad sizes, track outlines, courtyards), so the agent reads the file
  anyway and then also pays for the tool.
- `kicad_edit` has no footprint swap: swap_fp fell back to raw `kicad_ipc` (ImportNetlist, PlaceFootprintFromLibrary,
  UpdateItems in chunks) and read the API's proto sources: 8 M tokens.
- Setting a symbol's Footprint field through `kicad_edit` took the agent four tries.
- The raw place_decap failure ended its turn while a search it had started in the background ran; in a thread it would be
  woken and go on, a benchmark turn ends there.
- Runs share `/tmp`, and an agent's memory notes land in the login's Claude folder (one wrote a "footprint swap method"
  note); each run's folder has its own name, so no run read another's, but run them with that in mind.

## What was tried

| variant | what | passes | context tokens | tool calls | cost $ |
|---|---|---|---|---|---|
| base | `kicad_ipc` as it was | 24/24 | 9,296,015 | 195 | 12.22 |
| forgiving | `kicad_ipc` takes a free lock, opens its document, completes short command names | 21/24 | 7,720,497 | 158 | 11.31 |
| edit | `kicad_edit` behind the tool search | 24/24 | 1,684,672 | 44 | 5.27 |
| edit3 | `kicad_edit`, its tool description and prompt lines reworded | 23/24 | 995,162 | 18 | 4.43 |
| edit4 | a change answers with the items as they are | 24/24 | 957,096 | 18 | 4.44 |
| edit4n | loaded up front (no tool search) | 24/24 | 791,617 | 17 | 4.42 |
| final | what landed | 45/45 | 812,218 | 17 | 4.36 |

## Found on the way

- A hub's KiCad server that answered its first Ping with "not ready to reply" was marked failed; it is now asked again
  for up to 30 s (`kicad/ipc.rs`, `READY_WAIT`).
- A hub whose stderr reader had gone stopped at its next "kicad ipc: up" line and never handed the server its requests
  (the benchmark's stall); that line can no longer stop the handshake.
- Two KiCad API defects, written up for the fork with scripts: `test/kipc_bench/repros/README.md`.
