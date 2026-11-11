# Embedded KiCad E2E Test Plan (`test/kicad_embed/`)

End-to-end verification plan for real KiCad (`backplane-kicad-v10.99.0-bp.8+`) embedded as native child windows inside `backplane-gpui`. Every editor, dialog, cross-probe path, live agent `kicad_ipc` mutation, shortcut boundary, and crash/swap transition is exercised on an isolated display against copied fixtures and checked by three independent oracles.

---

## 1. Execution Stages

> **Feasibility vs. product verification:** In `EMBED_MODE=spike`, the test harness itself creates the 24-bit X11 container and withdraws/reparents/places KiCad's windows (`lib/x11_embed_driver.py`). `spike` runs therefore prove X11/KiCad/GPUI feasibility, **not** the product's own placement. Product verification requires `EMBED_MODE=stock` or `EMBED_MODE=fork`, where the suites drive `backplane-gpui` and `backplane-hub`'s own placement (`ui:Edit`, `ui:View`, frame strip, and `{"kicad":{"report":true}}`) without harness-side reparenting.

The harness supports three modes through `EMBED_MODE=spike|stock|fork` (`fork` once `bp.8` lands):

1. **Stage A (`EMBED_MODE=spike`, feasibility proof on stock KiCad 10.0.7):**
   - Uses stock `/usr/bin/kicad` on a seeded `XDG_CONFIG_HOME` (suppressing the first-run wizard via pre-written `kicad_common.json` with `api.enable_server = true`).
   - Harness-driven `XWithdrawWindow` + `XReparentWindow` into a harness 24-bit container inside GPUI's 32-bit X11 window; proves `ConfigureWindow` resize/move sync, synthetic `ConfigureNotify` on top-level window moves, `XShape` overlay cutouts, `XTest` mouse/key routing, GAL + 3D OpenGL rendering, DRC/Plot floating dialogs, and live `api.sock` commits + `Ctrl+Z`.
2. **Stage B (`EMBED_MODE=stock`, product integration on stock KiCad 10.0.7):**
   - Hub spawns GUI KiCad via `proc.rs` (`kicad/embed.rs`) with seeded config + isolated `TMPDIR` `api.sock`, shuts down headless `kserve`, and routes `kicad_ipc` to the GUI socket; client (`native_child.rs` + `KicadPane`) owns the 24-bit container, reparenting, resize/move sync, `XShape` cutouts, and emits `GPUI_LOG {"event":"kicad",...}`. Zero harness-side placement.
3. **Stage C (`EMBED_MODE=fork`, product integration on `backplane-kicad-v10.99.0-bp.8`):**
   - Hub spawns `kicad --embedded --parent-window <container_xid> --toplevel-window <gpui_xid> --api-socket <sock> --parent-pid <hub_pid> <proj>.kicad_pro` through `proc.rs`.
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

## 5. File Layout & Invocation (`test/kicad_embed/`)

```text
docs/kicad-embed-e2e.md                        # specification + empirical findings (spike, stock, fork)
test/kicad_embed/
  lib/
    env_xvfb.sh                                # BP_TMP, private Xvfb + i3 bootstrap, own-group cleanup, llvmpipe env
    seed_project.py                            # copies ecc83-pp into isolated git repo + seeds KiCad 10.0/10.99 JSON configs
    x11_embed_driver.py                        # C/libX11/libXext oracle + spike driver: X11 tree/pixel checks, transient_for, XShape
    gui_ipc_probe.py                           # zero-dependency AF_UNIX NNG REQ0 + raw Protobuf client against api.sock
    hub_rpc.mjs                                # zero-dependency Node WebSocket client for editor.kicad.* RPCs & info frames
  embed_window_e2e.sh                          # Suite 1 (14 checks in stock|fork; 12 in spike): cold Edit, reflow, move popups, XShape, View/Edit, rail switches, focus
  embed_editors_e2e.sh                         # Suite 2 (15 checks): Schematic, Symbol, PCB + copper zones, Footprint, DRC, 3D orbit, Plot + Gerber/Drill/STEP
  embed_agent_resilience_e2e.sh                # Suite 3 (9 checks in stock|fork; 8 in spike): live agent IPC + GAL diff + Ctrl+Z, SIGSTOP grab_escape, close(ask), GPUI SIGKILL save-set hot-swap, KiCad SIGKILL + restart + close
  run_all.sh                                   # sequential RAM-light runner (strictly one KiCad process at a time)
```

Run all three suites sequentially in `spike`, `stock` (stock KiCad 10.0.7), or `fork` (`backplane-kicad 10.99.0-bp.8+`):

```sh
EMBED_MODE=spike ./test/kicad_embed/run_all.sh
EMBED_MODE=stock ./test/kicad_embed/run_all.sh
EMBED_MODE=fork BACKPLANE_KICAD_BIN=/path/to/bp8/bin/kicad BACKPLANE_KICAD_CLI=/path/to/bp8/bin/kicad-cli ./test/kicad_embed/run_all.sh
```

---

## 6. Actual Client & Hub Contract (`KicadPane`, `native_child.rs`, `kicad/embed.rs`)

1. **Client script actions (`--script`):**
   - `{"click": "ui:Edit"}` and `{"click": "ui:View"}` (viewer bar mode switch), plus `{"click": "rail:Board"}`, `{"click": "rail:Schematic"}`, `{"click": "rail:3D"}`.
   - `{"kicad": {"report": true, "tag": "..."}}` and `{"kicad": {"wait_painted": 45000, "kind": "PCB", "tag": "..."}}` -> emits `GPUI_METRIC {"event":"kicad","ok":true,"project":"...","root":"...","open":"board","want":"PCB","state":"ready","mode":"stock"|"fork","pid":...,"holder":...,"sock":"...","pane":{"x":...,"y":...,"w":...,"h":...},"container":{"x":...,"y":...,"w":...,"h":...,"shown":true,"painted":...,"depth":24,"root_x":...,"root_y":...,"shape_rects":...,"in_tree":true,"xid":...},"toplevel":...,"toplevel_depth":32,"keys":false,"frames":[...],"shown":...,"shown_root":{"x":...,"y":...},"in_gpui_tree":true,"tag":"..."}`.
2. **Hub info state (`editor.kicad.<16-hex-digest>`):**
   - Delivered on initial WebSocket connect (`wire/mod.rs` `hub.info_frame()`) and pushed via `hub.set_info` on every transition: `project`, `root`, `state` (`"starting" | "ready" | "unresponsive" | "failed" | "stopped"`), `mode` (`"stock" | "fork"`), `pid`, `reason`, `active`, `container`, `toplevel`, `holder`, `sock`, `display`, and `frames: [{kind, doc, title, modified, shown, xid, min_w, min_h}]`.
3. **Hub RPCs (`native/host/src/kicad/embed.rs`):**
   - `editor.kicad.attach {project, container, toplevel, display, frame}`
   - `editor.kicad.show {project, frame, document?, item?, remount?}`
   - `editor.kicad.detach {project}`
   - `editor.kicad.close {project, save: "ask"|"save"|"discard"}`
   - `editor.kicad.restart {project, frame?}`
   - `editor.kicad.frames {project, frames}` (stock client-discovered frame sync for dirty-state gating)

---

## 7. Empirical Findings on Stock KiCad 10.0.7 (`EMBED_MODE=spike`)

1. **Zero-patch wizard suppression & isolated `api.sock`:**
   - Pre-seeding `$XDG_CONFIG_HOME/kicad/10.0/kicad_common.json` (`"version": 6`, `"api": {"enable_server": true}`, `"do_not_show_again": {"data_collection_prompt": true, "update_check_prompt": true, "zone_fill_warning": true}`) plus `sym-lib-table`, `fp-lib-table`, `design-block-lib-table`, and per-instance `TMPDIR=<dir>` makes stock KiCad 10.0.7 skip `STARTWIZARD` completely and listen on `$TMPDIR/kicad/api.sock` with zero C++ changes.
2. **Config schema versions & default AUI docks:**
   - Stock KiCad 10.0.7 expects exact `"meta.version"` numbers per editor (`kicad_common.json: 6`, `pcbnew.json: 5`, `fpedit.json: 5`, `eeschema.json: 3`, `symbol_editor.json: 1`, `3d_viewer.json: 4`); mismatched versions reset the file to defaults.
   - Setting `"aui": {"show_properties": false, "show_layer_manager": false, "show_schematic_hierarchy": false}` reclaims ~460 px of horizontal canvas width in narrow half-window panes, and overriding `WM_NORMAL_HINTS` `PMinSize` (`min_width = 500` on `PCB_EDIT_FRAME`) allows clean narrow reflow.
3. **ICCCM `XWithdrawWindow` required on tiling/reparenting WMs:**
   - Hiding `KICAD_MANAGER_FRAME` or born-top-level `KIWAY_PLAYER` windows before `XReparentWindow` must use `XWithdrawWindow` (sending synthetic root `UnmapNotify`), not bare `XUnmapWindow`, or `i3` retains an empty tiled container.
4. **Stock `API_HANDLER_BOARD` `RunAction` requires mapped `PCB_EDIT_FRAME`:**
   - In stock KiCad 10.0.7, `RunAction` dispatches through `PCB_EDIT_FRAME`'s `TOOL_MANAGER`. Actions (`pcbnew.EditorControl.showEeschema`, `common.Control.show3DViewer`, `pcbnew.DRCTool.runDRC`, `common.Control.plot`, `pcbnew.ZoneFiller.*`) execute reliably when `PCB_EDIT_FRAME` is mapped, whereas unmapping `PCB_EDIT_FRAME` stalls coroutine tool transitions on its GAL canvas; `Symbol Editor` (`Ctrl+L`) and `Footprint Editor` (`Ctrl+F`) open cleanly via `KICAD_MANAGER_FRAME` (`KICAD_MANAGER_CONTROL::ShowPlayer`) inside the same single `kicad` process.
5. **Dirty-state detection on stock 10.0.7 vs `10.99-bp`:**
   - Stock KiCad 10.0.7 returns `AS_UNHANDLED` for `GetDocumentModifiedState` (added in `10.99`), but reliably prefixes `_NET_WM_NAME` with `*` when modified and strips `*` on `SaveDocument` / `Ctrl+S`.
6. **Sibling frame switch latency & live agent `Ctrl+Z`:**
   - Mapping/unmapping reparented `PCB <-> Schematic <-> 3D Viewer` siblings inside the 24-bit container takes **0.5–0.8 ms**.
   - Live `BeginCommit -> CreateItems(Track) -> EndCommit` over `api.sock` immediately repaints the embedded OpenGL GAL canvas without file reload, and `Ctrl+Z` inside the embedded editor atomically reverts the agent's commit.

---

## 8. Empirical Findings in Product Modes (`EMBED_MODE=stock` & `EMBED_MODE=fork` `10.99.0-bp.8`)

1. **Stock `kicad --frame pcb <board>` segfaults in 10.0.7 (`exit 139`):**
   - Launching stock 10.0.7 directly into `pcbnew` via `kicad --frame pcb <board>` segfaults on a fresh config. `stock_argv` launches `kicad <project>.kicad_pro` (project manager) and `KicadPane` opens editors by sending KiCad's manager shortcuts (`Ctrl+P`, `Ctrl+E`, `Ctrl+L`, `Ctrl+F`, and `Alt+3` on `PCB`). Real-KiCad unit test `stock_argv_launches_real_kicad_without_crashing` guards against regressions.
2. **Two-phase WM withdrawal (`native_child::adopt`):**
   - Reparenting a stock top-level immediately after `XWithdrawWindow` loses a race to `i3`'s `UnmapNotify` handler, which reparents unmanaged windows back to root. Waiting 150 ms between withdrawal and `XReparentWindow` makes stock frame adoption deterministic.
3. **Fork (`bp.8`) cold-start wxGTK `gs_decorExtentSupport` race & external `XMapWindow` hazard:**
   - Because `kicad --embedded` never shows `KICAD_MANAGER_FRAME`, the first `ShowFrame` call triggers wxGTK's process-wide 1000 ms `_NET_REQUEST_FRAME_EXTENTS` probe (`wxRequestFrameExtentsTimeout`, flipping `gs_decorExtentSupport: 0 -> 2`). Because `PrepareFrame` strips window decorations, `GTKUpdateDecorSize` returns early when that timer fires without calling `gtk_widget_show` on the first frame.
   - Calling external `XMapWindow` from the client on a fork frame whose `gtk_widget_show` was deferred causes GTK3's `gtk_window_map_event` to see `!gtk_widget_get_visible` and immediately call `gdk_window_hide`, leaving the frame permanently unmapped.
   - Fix: in fork mode, `native_child::place` never calls `XMapWindow`/`XUnmapWindow` on KiCad frames (leaving map state exclusively to `ShowFrame`/`HideFrame`), `switch_fork_frame` calls `HideFrame` on previously shown siblings before `ShowFrame`, and `spawn_gui_session` waits `1150 ms` past `wxRequestFrameExtentsTimeout` on cold start and toggles `HideFrame` + `ShowFrame`.
4. **Centered GPUI modal `XShape` cutouts:**
   - Centered GPUI modals (`Ctrl+K` command palette, `Improvements`) draw a full-pane dim backdrop hitbox behind a smaller card hitbox; filtering the full-pane backdrop out of `over` when a smaller card is present cuts a crisp `XShape` hole for the card while keeping the uncovered KiCad canvas live around it.
5. **`XFixesChangeSaveSet` client-crash survival & post-crash `LocalHistory` recovery dialog:**
   - Inserting `holder` (fork) and adopted frames (stock) into `XFixesChangeSaveSet(INSERT, ROOT, UNMAP)` keeps KiCad and unsaved edits alive across `SIGKILL` of `backplane-gpui` and warm `AttachToParent` into a fresh client.
   - After `SIGKILL` of KiCad `10.99.0-bp.8` with unsaved edits, `LocalHistory` opens a modal `"Recover Unsaved Changes"` dialog (`WM_TRANSIENT_FOR == gpui_toplevel`) inside `PGM_KICAD::OnPgmInit()` *before* `SetReadyToReply()`. Polling `<sock>.events` (`ET_DIALOG_OPENED` / `ET_DIALOG_CLOSED`) while `Ping` returns `AS_NOT_READY` extends the startup deadline so the hub never kills KiCad while the user is answering the recovery prompt.
6. **KiCad `10.99-bp.8` companion `custom_properties` migration:**
   - On board load/save, `10.99-bp.8` migrates item `custom_properties` from `<board>.kicad_pcb.backplane.json` into native `(custom_property ...)` S-expressions inside `<board>.kicad_pcb` while keeping `<proj>/.backplane.json` byte-identical.
