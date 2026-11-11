# Embedded KiCad E2E Test Plan (`test/kicad_embed/`)

End-to-end verification plan for real KiCad (`backplane-kicad-v10.99.0-bp.8+`) embedded as native child windows inside `backplane-gpui`. Every editor, dialog, cross-probe path, live agent `kicad_ipc` mutation, shortcut boundary, and crash/swap transition is exercised on an isolated display against copied fixtures and checked by three independent oracles.

---

## 1. Execution Stages

The harness supports two modes through a single flag (`EMBED_MODE=spike|fork`, default `fork` once `bp.8` lands) so harness construction does not block on the C++ fork build:

1. **Stage A (`EMBED_MODE=spike`, runs immediately on stock KiCad 10.0.7):**
   - Uses `/home/h/.local/bin/kicad` (or `pcbnew` / `eeschema`) on a seeded `XDG_CONFIG_HOME` (suppressing the first-run wizard via pre-written `kicad_common.json` with `api.enable_server = true`).
   - Exercises client-side `XUnmapWindow` + `XReparentWindow` into `native_child::container()` (`scr.root_visual` 24-bit container inside GPUI's 32-bit X11 window), `ConfigureWindow` resize/move sync, synthetic `ConfigureNotify` on top-level window moves, overlay hide/cutout, `XTest` mouse/key routing, GAL + 3D OpenGL rendering, and DRC/Plot floating dialogs.
2. **Stage B (`EMBED_MODE=fork`, `backplane-kicad-v10.99.0-bp.8`):**
   - Hub spawns `kicad --embedded --parent-window <container_xid> --api-socket <sock> --parent-pid <hub_pid> <proj>.kicad_pro` through `proc.rs`.
   - Asserts zero top-level WM flash on startup and when KiCad opens secondary frames (`EDA_3D_VIEWER_FRAME`, `SYMBOL_EDIT_FRAME`, `FOOTPRINT_EDIT_FRAME`), `WM_TRANSIENT_FOR` pointing to GPUI's top-level XID on all `DIALOG_SHIM`s, `_BACKPLANE_FOCUS` / `_BACKPLANE_KEY` `ClientMessage`s, `kiapi.backplane.embed` RPCs (`AttachToParent`, `ShowFrame`, `GetFrames`, `NextEvents`, `SetChrome`, `Close`), headless `kserve` handoff, live agent undoable commits, unsaved-changes gate, crash recovery, and client hot-swap re-parenting.

---

## 2. Isolation, Display, and Hardware Rules

Every script under `test/kicad_embed/` obeys Backplane's test and safety rules:

- **Temp & home isolation:** sources `scripts/lib-bptmp.sh` so scratch state lives under `$BP_TMP` (`mktemp -d "$BP_TMP/bp-kembed.XXXXXX"`), never `/tmp` and never `~/.backplane*`, `~/.local/share/backplane`, or `~/.config/kicad`. Sets isolated `HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`, and `0700` `XDG_RUNTIME_DIR`.
- **Process hygiene:** uses `test/e2e/lib/own-group.sh` and explicit captured PIDs (`PIDS`); never `pkill`, `killall`, or pattern kills. Asserts zero leftover `kicad`, `kicad-cli`, or `backplane-hub` processes after exit (`--parent-pid` verification).
- **Private X11 display + WM:** launches `/home/h/.local/xvfb-root/usr/bin/Xvfb` (with `LD_LIBRARY_PATH=/home/h/.local/xvfb-root/usr/lib/x86_64-linux-gnu`, falling back to `$HOME/.cache/xvfb-root` or `command -v Xvfb`) on a free display `:500+` at `1600x1000x24 -nolisten tcp`, plus `i3` with a minimal borderless config (`ipc-socket $OUT/i3.sock`) so floating dialogs, `WM_TRANSIENT_FOR`, and WM-driven window moves/resizes behave like a real desktop.
- **Software vs. hardware GPU:**
  - CI / Boat runs set `LIBGL_ALWAYS_SOFTWARE=1`, `GALLIUM_DRIVER=llvmpipe`, `LP_NUM_THREADS=2`, and `VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd*.json`.
  - Gate-closing runs (`test/e2e/CONTRACT.md` real-hardware rule) run with `--real-hardware` against the machine's GPU (`software:false` in `GPUI_METRIC`).
- **Stand-in agents only:** sets `BACKPLANE_AGENTS=$OUT/bin` with `claude` bound to `test/tools/claude_standin.ts` (`BP_STANDIN_MODE=stream`) so agent `mcpcall:{"name":"kicad_ipc",...}` turns execute deterministically with zero real model calls.
- **Copied fixtures only (never user boards):** each run initializes a fresh git repo (`$OUT/proj` + bare `$OUT/origin.git` on `main`, matching `test/e2e/kicad_main.py`) populated from copies of:
  - `test/files/fixtures/plot/ecc83-pp.{kicad_pcb,kicad_sch,kicad_prl}` (+ synthesized `ecc83-pp.kicad_pro`)
  - `test/files/fixtures/plot/complex_hierarchy.kicad_sch` (multi-sheet hierarchy)
  - `Backplane_KiCad` `demos/pic_programmer` (complete schematic + routed 2-layer PCB + custom symbol/footprint libs) and `demos/backplane-structure` (companion `.backplane.json` preservation).

---

## 3. Three-Oracle Verification Model

No check passes on log lines alone. Each step cross-checks three sources of truth:

1. **Window-system & pixel oracle (`lib/xtree_assert.py` + `import`):**
   - Parses `xwininfo -root -tree` and `xprop -id <wid>` to verify:
     - Active `KIWAY_PLAYER` XID is a descendant of `Backplane GPUI`'s 24-bit container XID (`MapState == IsViewable`), positioned within `±1 px` of `KicadPane`'s reported `view_x, view_y, w, h`.
     - Hidden frames (`SCH` while `PCB` is active) have `MapState == IsUnMapped`.
     - `KICAD_MANAGER_FRAME` is never mapped; zero `KIWAY_PLAYER` windows appear as direct children of the X root/i3 frame.
     - Open `DIALOG_SHIM` windows are top-level managed windows whose `WM_TRANSIENT_FOR(WINDOW)` equals `Backplane GPUI`'s top-level XID and whose screen center lies within `Backplane GPUI`.
   - Captures full-root PNGs (`import -window root "$OUT/<mark>.png"`) and asserts non-background painted sample counts (`native_child::painted`) plus region bounding-box pixel diffs before/after edits, zoom, routing, zone fill, and 3D orbit.
2. **Live GUI IPC oracle (`lib/gui_ipc_probe.py`):**
   - Connects directly (or via hub debug RPC) to `--api-socket` (`pynng.Req0`) to read `GetFrames`, `GetOpenDocuments`, `GetDocumentModifiedState`, `GetItems`, and `GetSelection` from the live GUI `KICAD_API_SERVER` in memory.
3. **Filesystem & `kicad-cli` oracle:**
   - After saves and exports, runs `/home/h/.local/bin/kicad-cli` (or the `bp.8` runtime's `kicad-cli`) `sch erc --format json`, `pcb drc --format json`, `sch export netlist`, and Gerber/Excellon header/aperture checks on the written files, plus `git status` and companion `.backplane.json` diff checks.

---

## 4. Feature & Journey Matrix

### Suite 1: `test/kicad_embed/embed_window_e2e.sh` (Fast Gate, ~45 s)

| ID | Scenario | Actions (`--script` + `xinput.py` + `i3-msg`) | Pass Assertions |
|---|---|---|---|
| **W1** | Cold open into **Edit** | Open project `pic_programmer`, click `rail:Board`, click `ui:Edit` | Hub shuts down any headless `kserve` for `pic_programmer`, spawns GUI `kicad --embedded`; `PCB_EDIT_FRAME` appears inside GPUI container within `±1 px`; no wizard, no splash, no manager window; `painted > 50`. |
| **W2** | Layout reflow, WM resize, and pure WM move | Fold sidebar (`Ctrl+B`), unfold; `i3-msg 'floating enable, resize set 1200 780, move position 180 110'`; then `i3-msg 'move position 320 190'` (move *without* resize); right-click canvas to open context menu | Child frame follows pane width/height within `< 50 ms`; after pure window move (no size change), right-click popup menu opens inside the moved pane bounds (verifies synthetic `ConfigureNotify` updates GTK3's cached root coordinates). |
| **W3** | Rail frame switching & Split | Click `rail:Schematic`, `rail:3D`, `rail:Board`; enable `Split` (`PCB` left, `SCH` right); toggle `View` and back to `Edit` | `ShowFrame` opens `SCH` and `PCB_3D` born-embedded (zero root top-level flash in X event watch); switching maps one and unmaps others in `< 16 ms` while preserving zoom/selection; Split maps both sibling XIDs at disjoint rects. |
| **W4** | GPUI overlays (`Ctrl+K` palette, menus) | Press `Ctrl+K` while `PCB` is focused; open `ui:Improvements` | Palette/overlay is visible and legible (never occluded by native X11 child: either SHAPE cutout leaves surrounding board live or fallback snapshot/hide triggers cleanly); closing overlay restores live child immediately. |
| **W5** | Focus ring & shortcut boundary | Open `Edit` (no click yet) -> press `S`; click PCB canvas -> press `M`, `Esc`, `Ctrl+B`, then `Ctrl+J`, `Ctrl+K`, `Ctrl+\`` | Before click, `taken == false` and Backplane owns keys; after click, 2 px accent ring shows (`Keys: KiCad`), `M`/`Esc`/`Ctrl+B` stay in KiCad (zone unfill, not sidebar fold!), `Ctrl+J` opens Backplane terminal, `Ctrl+\`` returns focus to composer (`Keys: Backplane`). |
| **W6** | Hotkeys vs. reserved chords static audit | Call `ListActions` on `PCB`, `SCH`, `SYMBOL_EDITOR`, `FOOTPRINT_EDITOR`, `PCB_3D` | Zero default KiCad hotkeys collide with default reserved chords (`Ctrl+K`, `Ctrl+J`, `Ctrl+Tab`, `Ctrl+Shift+Tab`, `Ctrl+\``). |

### Suite 2: `test/kicad_embed/embed_editors_e2e.sh` (Full EDA Workflow Gate, ~120 s)

| ID | Editor / Feature | Actions | Pass Assertions |
|---|---|---|---|
| **E1** | **Schematic edit & save** (`Eeschema`) | Open `pic_programmer` Schematic in `Edit`; press `A` (Choose Symbol dialog), filter `R`, place resistor; press `W` and wire two pins; press `L` and place net label `TEST_NET`; press `E` (Symbol Properties), set `Value=4k7` and add `MPN=RC0603FR-074K7L`; traverse subsheet in `complex_hierarchy`; press `Ctrl+S` | Choose Symbol and Properties `DIALOG_SHIM`s have `WM_TRANSIENT_FOR == gpui_toplevel`; typing `k`/`j` in dialog filter stays in dialog; `GetItems` shows new symbol + wire + `TEST_NET`; before `Ctrl+S`, `modified == true`; after `Ctrl+S`, `modified == false` and `kicad-cli sch export netlist` contains `TEST_NET` and `RC0603FR-074K7L`. |
| **E2** | **Symbol editor** (`SYMBOL_EDIT_FRAME`) | Select placed resistor, press `Ctrl+E` (or `ShowFrame {kind: SYMBOL_EDITOR}`); add/edit pin or description; save to project `.kicad_sym` | `SYMBOL_EDIT_FRAME` is born embedded (no WM top-level map), listed in pane strip, closes cleanly via strip `×`, and `.kicad_sym` on disk reflects the edit. |
| **E3** | **Footprint editor** (`FOOTPRINT_EDIT_FRAME`) | Open `FOOTPRINT_EDITOR` from PCB (`Ctrl+E` on a footprint or `ShowFrame`), edit pad/courtyard, save to project `.pretty` | `FOOTPRINT_EDIT_FRAME` is born embedded, GAL renders footprint pads, `.kicad_mod` saved and valid under `kicad-cli fp export svg`. |
| **E4** | **KIWAY cross-probing & Update PCB from Schematic** | Select `R1` in Schematic -> check `NextEvents` + switch to `PCB`; press `F8` (`Update PCB from Schematic`) -> click `Update PCB` -> place new footprint | Selecting `R1` emits selection event to hub (`NextEvents`) and highlights/centers `R1` in `PCB` (`GetSelection`); `F8` quasi-modal dialog imports the new `4k7` resistor from **E1** onto the board without file reload. |
| **E5** | **PCB placement & interactive routing** (`Pcbnew`) | Select footprint, `M` move + `xinput.py drag`, `R` rotate, `F` flip; press `X` on unrouted ratsnest pad, route track with bend + `V` via to target pad, click to finish | `GetItems` reports updated footprint pose/layer plus new `PCB_TRACK` segments and `PCB_VIA`; canvas screenshot diff shows copper trace on `F.Cu`/`B.Cu`. |
| **E6** | **Copper zone fill & unfill** | Press `B` (fill all zones) on `pic_programmer` / `ecc83-pp`; capture canvas + `GetItems`; press `Ctrl+B` (unfill); press `B` again and `Ctrl+S` | Filled polygon count in `ZONE` items goes `> 0 -> 0 -> > 0`; copper pour region pixels change accordingly; saved `.kicad_pcb` retains filled polygons. |
| **E7** | **ERC & DRC** (interactive + CLI parity) | Trigger ERC in Schematic and DRC in PCB (`Inspection -> Design Rules Checker`); introduce a deliberate clearance overlap (`M` drag track onto wrong-net pad), click `Run DRC`, click violation row, undo overlap (`Ctrl+Z`), re-run DRC | `DIALOG_DRC` floats transient over Backplane; violation count goes `0 -> >=1 -> 0` matching `kicad-cli pcb drc --format json`; clicking violation pans/zooms PCB GAL view (`FocusOnItems`). |
| **E8** | **3D Viewer** (`EDA_3D_VIEWER_FRAME`) | Switch to `3D` (`Alt+3` or rail `3D`); `xinput.py drag` (orbit) and `wheel` (zoom); move a large connector in `PCB` and switch back to `3D` | `EDA_3D_VIEWER_FRAME` (`CreateAndShow3D_Frame`) is born embedded inside the container, renders 3D board (`painted` check + screenshot), orbit changes pixels, and `Update3DView` reflects moved connector. |
| **E9** | **Plotting & Fab export** + companion integrity | Open `File -> Plot` and `Generate Drill Files` (or `RunAction`), export Gerbers + drill into `$OUT/proj/mfg/`; also test board with `<board>.backplane.json` | `.gbr` and `.drl` files created in `$OUT/proj/mfg/` with non-empty apertures/hits; `.backplane.json` and `<board>.backplane.json` companions remain byte-intact. |

### Suite 3: `test/kicad_embed/embed_agent_resilience_e2e.sh` (Agent Co-Edit & Resilience Gate, ~60 s)

| ID | Scenario | Actions | Pass Assertions |
|---|---|---|---|
| **R1** | **Live agent `kicad_ipc` edit + human `Ctrl+Z`** | With `PCB` open in `Edit`, stand-in agent calls `kicad_ipc` (`BeginCommit` -> `UpdateItems` move footprint / `CreateItems` track / `SketchRoute` -> `EndCommit`); then human clicks canvas and presses `Ctrl+Z` | Edit lands in the live GUI without closing/reopening; screenshot shows moved footprint immediately; `Ctrl+Z` in GUI reverts the agent's commit cleanly (`GetItems` returns original coords)! |
| **R2** | **Modal dialog guard (`AS_BUSY`)** | Open `Board Setup` (`ShowQuasiModal()`) in GUI; while open, agent sends mutating `kicad_ipc` commit | Request returns clean `AS_BUSY` naming the open dialog instead of dispatching inside nested `wxGUIEventLoop` (zero crash/UAF); closing dialog allows retry to succeed. |
| **R3** | **Canonical `main` + `doc_lock` & unsaved-changes gate** | Human makes unsaved edit in `PCB`; agent attempts `SaveDocument` or raw write; human clicks close thread / quit app | Human edit holds/marks modified state on `main`'s canonical board (never a thread worktree copy); Backplane shows single `Save all / Discard / Cancel` bar (KiCad's native "Save changes?" prompt never appears); `Save all` writes canonical file and commits design on turn/close per `canonical.rs`. |
| **R4** | **Client hot-swap (`AttachToParent`)** | With dirty unsaved `PCB` + `SCH` open in `Edit`, exit `backplane-gpui` and start a fresh `backplane-gpui` against the same `backplane-hub` | KiCad detaches all frames unmapped (`AttachToParent {window: 0}`), stays alive under hub, and re-parents into the new client's container XID with dirty state, open frames, and undo stack intact. |
| **R5** | **KiCad crash & hang recovery** | `kill -KILL <kicad_pid>` while `Edit` is open; verify fallback + click **Restart**; then `kill -STOP <kicad_pid>` past ping timeout (`10 s`) and `kill -CONT` / end | `backplane-gpui` and `backplane-hub` survive; pane shows `"KiCad stopped"` + **Restart**; headless `kserve` answers agent `kicad_ipc` meanwhile; **Restart** re-embeds frames; `SIGSTOP` hang triggers `"KiCad is not responding"` banner and reserved `XGrabKey` (`Ctrl+K` / `Ctrl+\``) still reaches Backplane! |

---

## 5. Planned File Layout in `kicad-embed-e2e`

```text
docs/kicad-embed-e2e.md                        # this specification
test/kicad_embed/
  lib/
    env_xvfb.sh                                # BP_TMP, Xvfb + i3 bootstrap, own-group cleanup, software/hw GL env
    seed_project.py                            # copies fixtures/demos into isolated git repo + seeds kicad_common.json
    xtree_assert.py                            # xwininfo/xprop assertions: container parenting, transient-for, zero stray top-levels
    gui_ipc_probe.py                           # pynng client against --api-socket for live GetFrames/GetItems/modified/selection
  embed_window_e2e.sh                          # Suite 1: W1-W6 (supports EMBED_MODE=spike today and EMBED_MODE=fork on bp.8)
  embed_editors_e2e.sh                         # Suite 2: E1-E9 (Schematic, Symbol, Footprint, Cross-probe, PCB, Zones, DRC/ERC, 3D, Plot)
  embed_agent_resilience_e2e.sh                # Suite 3: R1-R5 (live agent IPC + Ctrl+Z, modal guard, unsaved gate, hot-swap, crash/hang)
```

---

## 6. CLI & Report Contract Needed from Hub and Client

To keep `test/kicad_embed/*` deterministic across Linux (and reusable on Windows via a `windowed_proof.py`-style sibling), `KicadPane` and `kicad/embed.rs` should emit the same diagnostic shape CEF already uses (`cef_page.rs` / `windowed_e2e.sh`):

1. **Client script actions (`--script`):**
   - `{"click": "ui:Edit"}` and `{"click": "ui:View"}` (viewer bar mode switch)
   - `{"kicad": {"report": true}}` -> emits one `GPUI_LOG {"event":"kicad","mode":"embedded"|"separate","project":"...","active_frame":"PCB","view_x":...,"view_y":...,"w":...,"h":...,"covered":false,"taken":false,"native":{"xid":...,"container_xid":...,"x":...,"y":...,"w":...,"h":...,"shown":true,"painted":...},"frames":[...]}` line reading actual X11/Win32 geometry back via `native_child::geometry` + `native_child::painted`.
2. **Hub info state (`editor.kicad.<project_digest>`):**
   - `state`: `"starting" | "ready" | "unresponsive" | "failed" | "stopped"`, `pid`, `sock`, `frames: [{kind, document, title, modified, wid, min_w, min_h}]`.
3. **Fork flags (`bp.8`):**
   - `kicad --embedded --parent-window <container_xid> [--toplevel-window <gpui_xid>] --api-socket <sock> --parent-pid <hub_pid> <project>.kicad_pro`
   - Prints `KiCad API server listening at <sock>` on stdout/stderr once `KICAD_API_SERVER::SetReadyToReply()` has registered handlers.
