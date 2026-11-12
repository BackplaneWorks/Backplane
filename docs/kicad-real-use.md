# Embedded KiCad in real use: numbers

What working in embedded KiCad costs on real boards, measured on the machine's GPU. The harness is `test/kicad_embed/real_use_*.sh`: it drives
`backplane-gpui` and a test hub (temp home, stand-in agents) on copies of real projects, never the originals, as a person would (XTest clicks on
the client's controls, XTest pan/zoom/keys into KiCad), with an agent's commits over KiCad's API and the hub's RPCs, while the pane's pixels are
sampled throughout.

## How to run

```sh
# one KiCad at a time on this desktop: run under the kicad-embed-e2e lock (work_run lock)
REAL_HARDWARE=1 test/kicad_embed/real_use_all.sh [OUT]           # every board x runtime, then the table against real_use_budgets.json
KEMBED_BOARDS="hubble" KEMBED_RUNTIMES="bp12" KEMBED_JOURNEYS="switch,edit"   # narrow (names in lib/real_use.sh; journeys below)
REAL_HARDWARE=1 EMBED_MODE=fork BACKPLANE_KICAD_BIN=.../bin/kicad KEMBED_PROJ_SRC=<staged copy> test/kicad_embed/real_use_journeys.sh  # one run
python3 test/kicad_embed/lib/real_use_table.py OUT [--strict]   # the table again; --strict (or KEMBED_REAL_STRICT=1) exits 1 on a miss
REAL_HARDWARE=1 test/kicad_embed/real_use_baseline.sh [OUT]      # embed_perf_e2e.sh on each board x runtime (the first baseline)
test/kicad_embed/real_use_watch.sh                               # Edit then Schematic with a screenshot every 4 s (what a hang shows)
```

Each run prints `PERF <measure> <ms> key=value...` lines and a `JSON` line, and writes `OUT/<board>-<runtime>/real_use.json`; the matrix writes
`OUT/summary.json` (every run, every budget miss). Screenshots: `OUT/<run>/shots/` (each thread round trip, each dialog, the end).

- **Real hardware.** `REAL_HARDWARE=1` runs on a private display the GPU draws (`lib/gpu_display.sh`): a headless GNOME Shell (Wayland only, a
  virtual 1800x1050 monitor at 60 Hz, its own session bus and an empty private system bus, so it never reaches the person's GDM or logind) with a
  rootful Xwayland on it (glamor) and i3 inside, as on Xvfb. Nothing shows on the person's screen; XTest reaches only that Xwayland. The client
  reports `"software":false` (RTX 5080) and KiCad's GAL draws with the GPU (the hub gets no `LIBGL_ALWAYS_SOFTWARE` then). Xvfb has no GPU:
  its numbers are llvmpipe's. `KEMBED_XVFB=1` forces Xvfb.
- **Boards.** `lib/real_use.sh` stages a slim copy (KiCad files, sheets, libraries, 3D models; no backups, build, mfg, git, locks) of
  `~/Documents/Nordic/Hubble_DK` (hubble) and `~/Documents/i2cjak/Collar_Wearable/boards/main` (collar_main).
- **Runtimes.** bp11: the released runtime the person runs (`~/.backplane-gpui-fresh/home/kicad/10.99.0-bp.11`, only read); bp12: the test build
  (`~/.cache/kicad-test/10.99.0-bp.12*`). Add a runtime (bp.13 for the floorplanner) as one line in `REAL_RUNTIMES`.

### What is measured, and how

- **Pixels** (`lib/pane_sampler.py`): `XGetImage` of the pane's rectangle on the root (what the person sees, composited) about every 2 ms; each
  sample's CRC, and for a new picture the share of its most common colour. A *switch* event (a click, a key, an RPC): `first_change` (old content
  shown until), `settled` (the last change before 400 ms of stillness: click to drawn), `blank_frames` (distinct pictures in between that are
  one colour: a flash of background), `mid_frames`. An *input* event (pan, wheel, drag, routing): the intervals between changed pictures while it
  runs, p50/p95/p99/max and the counts above 33 and 50 ms. A *commit*: the agent's call to the first repaint. The sampler's own cost is checked
  each run: `pan_6s_sample20` repeats the pan with a sample at most every 20 ms; neither shows hitches, so the readback does not make them.
- **Input** (`lib/rt_input.py`): middle-button pan circling at 120 Hz, wheel at 60 notches/s (8 in, 8 out), drags at 60 Hz, keys, each with its
  epoch times. The client's controls come from its `controls` metric (`BACKPLANE_GPUI_RECTS=1`); the editor's tabs register no rect yet, so they
  are clicked at their measured place over the pane (`TAB_AT` in `real_use_drive.py`).
- **Dialogs**: a KiCad window that appears on its own (not an editor frame, not embedded) is recorded with its title and a screenshot and
  dismissed with Return; it blocks KiCad. **Lost editor**: after the thread journey, no KiCad frame over the pane in the X tree is a `lost_after_*`
  miss; the editor is reopened so the rest still measures.

Journeys (`KEMBED_JOURNEYS`): `open` (cold Edit by the client's script; View then Edit three times), `switch` (Board/Schematic/3D 16 times;
another thread and back three times through the command palette), `edit` (pan 10 s, wheel zoom 10 s, click, drag, Delete, Ctrl+Z, a track routed
with X), `coedit` (5 agent tracks while idle; 6 during an 8 s pan), `xprobe` (`editor.kicad.select` of 4 references with zoom), `save` (an agent
edit, then Ctrl+S: written and clean), `sch` (Schematic pan and zoom), `comment` (the fork's CommentSelection to the message in the thread).
Not yet: schematic-to-board cross-probe by a click, DRC/ERC/Gerbers, hierarchical sheet/move symbol/wire/annotate.

## Budgets

`test/kicad_embed/real_use_budgets.json` (non-strict: misses print; `"strict": true` once lane B meets them). Proposed and corrected from the
hardware: first paint warm (reopen) <= 300 ms with no blank frame; cold <= 2.5 s; tab or thread switch <= 50 ms click to settled with no blank
frame; pan/zoom p99 interval <= 25 ms and no frame > 50 ms (the display is 60 Hz: a perfect pan is 16.7 ms a frame, and 1 to 2 ms of sampling
jitter puts p99 near 20, so 16 could never pass); routing and dragging no frame > 50 ms; agent commit to repaint <= 100 ms; cross-probe <= 100 ms;
save written and clean <= 300 ms; comment posted <= 300 ms; no KiCad dialog during the journeys; the editor still there after each journey.

## Numbers (2026-11-12, origin/main 5c6dd03c4, RTX 5080, 1800x1050 at 60 Hz, pane 804x920)

ms. Matrix run `realuse-m1`: real_use_all.sh, both boards, both runtimes.

| measure | collar bp11 | collar bp12 | hubble bp11 | hubble bp12 | budget |
|---|---|---|---|---|---|
| cold Edit to PCB painted | 2855 | 2820 | 3169 | 3213 | 2500 |
| reopen (View, Edit; warm), settled | 104-114, 1 blank each | 106-121, 1 blank each | 101-123, 1 blank each | 120-121, 1 blank each | 300, 0 blank |
| first switch to Schematic | 230, 1 blank | 364, 1 blank | 839, 2 blank | 940, 2 blank | 50, 0 |
| first switch to 3D | 1288, 1 blank | 1301, 1 blank | 1392, 1 blank | 1398, 1 blank | 50, 0 |
| later tab switches (14 each) | 67-82 | 65-83 | 67-87 | 66-82 | 50 |
| ... of them with a blank frame | 6 | 6 | 5 | 6 | 0 |
| thread away and back | editor lost | editor lost | editor lost | editor lost | kept |
| pan 10 s: p50 / p99 / max | 16.7 / 19.8 / 21 | 16.7 / 19.3 / 20 | 7.1 / 7.1 | 8.1 / 8.1 | p99 25 |
| wheel zoom 10 s: p50 / p99 | 11.9 / 18.5 | 9.1 / 16.6 | 10.6 / 17.1 | 10.5 / 17.1 | p99 25 |
| pan during agent commits: p99, > 50 ms | 19.8, 0 | 19.6, 0 | 22.2, 1 | 20.6, 1 | 25, 0 |
| route a track 1.5 s: p99, > 50 ms | 89, 9 | 90, 9 | 197, 5 | 212, 7 | 0 > 50 |
| drag a footprint: p99, > 50 ms | 116, 2 | 103, 2 | 31, 0 | 35, 0 | 0 > 50 |
| click select / Delete / Ctrl+Z, settled | 60 / 40 / 27 | 60 / 40 / 24 | 206 / 65 / 53 | 118 / 61 / 56 | |
| agent commit to repaint (idle) | 19-22 | 19-21 | 27-31 | 30-46 | 100 |
| editor.kicad.select: pixels settled | 298-337 | 284-405 | 323-353 | 319-344 | 100 |
| editor.kicad.select: RPC answered | 544-600 | 547-597 | 595-627 | 593-619 | |
| Ctrl+S: written / clean | 36 / 55 | 57 / 78 | 56 / 192 | 56 / 117 | 300 |
| schematic pan p99 / zoom p99 | 21.2 / 18.2 | 21.0 / 18.0 | 20.7 / 18.8 | 20.2 / 16.6 | 25 |
| Comment to Backplane to the thread | 52 | 48 | 45 | 47 | 300 |
| KiCad dialogs during the run | Load PCB | none | Load PCB, Information, Load Schematic | Information | none |

Hubble's pan p50 of 7 to 8 ms: its board redraws faster than the compositor's 60 Hz, so Xwayland shows intermediate pictures.
The RPC times include starting `bun` for `hub_rpc.mjs` (about 50 ms).

### First baseline (embed_perf_e2e.sh, `wait_painted`)

`wait_painted` counts a mapped frame with pixels as drawn, so a warm switch reads 2 to 9 ms whatever is on screen; the pixel sampler above
supersedes it. Collar: cold 3175 (bp11) / 3315 (bp12), first Schematic 209 / 286, first 3D 223 / 282, save written 209, clean 690 / 231. Hubble:
Schematic never showed (> 30 s, then KiCad unresponsive) because of the modal "An error was found when loading the schematic that has been
automatically fixed. Please save the schematic..." (`Load Schematic`), centred on the screen outside the pane; while it is up KiCad answers no
API ping, the hub reports `unresponsive` and the pane keeps saying "Starting KiCad...".

## Jank, ranked (2026-11-12)

1. **Thread away and back loses the editor** (switch; all 4 runs). After switching to another thread and back (through the command palette), the
   thread shows its chat with the viewer closed and no KiCad (shots `thread_back*.png`); budget: the editor kept. Cause: the client or hub
   does not keep the thread's viewer/editor open across a thread switch (client: viewer state per thread; or hub: the thread's `viewer.open`).
2. **KiCad's modal dialogs block it, outside the pane** (open, switch; 3 of 4 runs, and the person's Hubble). `Load PCB`, `Information`, `Load
   Schematic`: top-level windows centred on the screen, not over the pane; KiCad's loop is blocked (no API pings), the hub says `unresponsive`,
   the pane says "Starting KiCad...". Cause: the fork shows load-time message boxes as free top-level dialogs; the hub reports a modal as
   unresponsive instead of "KiCad is asking something" and the client does not bring the dialog to the pane.
3. **First switch to 3D: 1.3 to 1.4 s with a blank frame** (switch; all 4; budget 50). The prewarm (`editor.kicad.prewarm` 2 s after the first
   editor) does not make 3D's first show cheap. Cause: the fork's 3D viewer (its first show builds the GL scene and loads models; prewarm creates
   the frame at size but does not render it).
4. **First switch to Schematic: 230 to 360 ms on Collar, 840 to 940 ms with 2 blank frames on Hubble** (switch; budget 50). Same cause as 3 for
   the schematic canvas: the first paint at the pane's size happens on show. Hubble's sheets make it 3 times longer.
5. **Every warm tab switch: 65 to 87 ms, and Schematic/3D targets flash one blank frame** (switch; 6 of 14 per run; budget 50 ms, 0 blank).
   Board never flashes. The pane shows one picture of plain background between the old editor and the new one: the new frame is mapped before
   its canvas has repainted. Cause: client/native_child placement (map the new frame and unmap the old in one step only after the new one has
   painted, or keep the old one up until then) with the fork's Expose repaint of the SCH and 3D canvases.
6. **Reopen (View then Edit, KiCad warm): 100 to 123 ms, one blank frame every time** (open; all 12; budget 300 ms, 0 blank). The time is
   within budget; the flash is the same mechanism as 5.
7. **Cross-probe from Backplane: 280 to 405 ms to KiCad's selection on screen; the RPC answers after 545 to 627 ms** (xprobe; budget 100).
   The answer comes about 250 ms after the pixels changed, so the hub waits after KiCad has acted (a fixed delay or a poll before replying), and
   KiCad's select + zoom itself takes about 300 ms. Cause: hub (`editor.kicad.select`) and the fork's select/zoom.
8. **Routing hitches: 5 to 9 frames over 50 ms in 1.5 s, p99 89 to 212 ms; dragging a footprint 2 frames over 50 ms on Collar** (edit; budget no
   frame > 50). Cause: the fork (the interactive router and the move tool), worse on Hubble.
9. **Cold Edit: 2.8 to 3.2 s** (open; budget 2.5 s). Cause: KiCad start and board load (fork) after the hub's spawn.
10. **Pan during agent commits: one frame over 50 ms on Hubble** (coedit; budget 0). Pan alone and on Collar: none. Cause: the fork repainting
    after an API commit on a large board.

Within budget: pan and zoom (p50 16.7 ms, the display's rate; p99 17 to 22), agent commit to repaint (19 to 46 ms), Ctrl+S (36 to 57 ms written,
55 to 192 clean), Comment to Backplane (45 to 52 ms to the thread).
