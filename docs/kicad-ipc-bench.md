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
