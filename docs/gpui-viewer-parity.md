# GPUI viewers vs the web/Bend viewers

Audit of `native/gpui` (client) and `native/host/src/kicad` (hub) against the main-branch web viewers
(`src/web/{plot2d,solid,probe}.js`, `src/core/{client,layers,hier,hist,mech,...}.bend`, `src/mobile/view.bend`).
The hub already ships the web's viewer model to a native client as `thread.viewer` (`src/mobile/view.bend`
`View.json`: `open`, `key`, `choices`, `sheets`, `layerList`, `off/dim/lift`, `card`, `picked`, `hist`, `mech`,
`files`, `review`, `comments`, `panes` for Split) when the Bend helper is the presenter. The Rust hub ports those cases one
by one, so a gap is either "the Rust hub has not ported the case" or "the client does not draw it", often both.

| # | Feature (web/Bend) | Web | GPUI before | GPUI now | Evidence |
|---|---|---|---|---|---|
| 1 | Tabs Board/Schematic/3D/Mech/Files/Review, close | yes | yes | yes | `screens/viewer.rs` |
| 2 | Board/schematic pan, wheel zoom at pointer, fit | yes | yes | yes | `board.png`, `schematic.png` |
| 3 | Layer list: show/hide, active layer, All/Dim/Only | yes | menu drawn, **every press refused** by the hub | **works** (core/layers.bend ported to `present/viewer.rs`) | `layer-focus.png`, `test/gpui/viewers/layers.json` |
| 4 | Board 3D, free orbit, zoom, pan (shift/right drag), roll | yes | yes | yes | `3d.png` |
| 5 | Mech: part chips, Refresh, any STEP/GLB under `mech/` through FreeCAD/OCCT conversion | yes | **refused** (`PRESENTER_UNAVAILABLE`) | **works** (listing, default part = assembly, `mech-part`) | `mech.png`, `present/viewer.rs` tests |
| 6 | Split: schematic + board + 3D at once | yes | "not drawn here yet" | **works** (hub `kicad.watch` kind `split`, client store per source, camera per pane) | `split.png`, `test/gpui/viewers/split.json` |
| 7 | Tap a part/net: lit piece + card (net, layer, width, ref...) | yes | no | **works** on board and schematic canvases (`kicad.info`) | `pick-card.png`, `test/gpui/viewers/pick.json` |
| 8 | Agent `viewer_show` opens the page the user sees | yes | the frame was taken for a hub greeting and failed | **works** (board, schematic, 3D, Mech on a part path) | `agent-viewer-show.png`, `test/gpui/viewers/agent-show.json` |
| 9 | Cross-probe schematic <-> board (a part by `ref`, a net by name on the board) | yes (`probe.js`) | no | **works** (hub `kicad.probe`, cyan outlines on every plot of the page; Split lights the schematic symbol and the board pads) | `cross-probe.png`, `test/gpui/viewers/probe.json` |
| 10 | Schematic sheet menu / hierarchy | yes | no | **works** (`view-sheet` ported, `{"t":"sheets"}` kept); board picker for several boards still open | `test/gpui/viewers/sheets.json` |
| 11 | Touchpad pan, pinch/ctrl+wheel and wheel zoom, double-click fit, keys (arrows, + -, 0/Home, Esc) | yes | wheel zoom only | **works** with real X input (XTest): wheel, drag, double-click, keys (`camera-real-input.png`); a touchpad's pixel scrolls cannot be faked with XTest, the pixel-delta branch is built but not driven | `test/gpui/viewers/camera-drive.sh` |
| 12 | 3D part box lit by probe, snap view, quality | yes | no | open | - |
| 13 | History bar (the thread's design steps, previous/next/play, Live), compare with a version, Old/Diff/New | yes | no | **works** (`history.steps` new hub query, step ticks and playback every 1.2 s, Old/Diff/New for steps and for Compare; `screens/histbar.rs`) | `history-old-new-diff.png`, `history-playback.png`, `test/gpui/viewers/history.json` (`GPUI_STEPS=1`) |
..` watch); the history bar, step playback and Old/Diff/New toggle are open | `compare.png`, `test/gpui/viewers/compare.json` |
| 14 | Files: image / PDF / note preview | yes | folder rows only | open | - |
| 15 | Mech Renders page | yes | no | open | - |
| 16 | Review tab pages, comments pinned on the plot | yes | page chips + Markdown | open | - |
| 17 | Diff tab, Browser tab | yes | yes | yes | - |

Hub side (`native/host/src/kicad/*`): plot, sheets tree, plot info (pick/probe data), solid/GLB frames, BOM/checks,
history exist as modules in the hub; the presenter port (`present/viewer.rs`) is what limits the pages.

What was found by running the stack (`scripts/gpui/desktop-plot-run.sh`, own hub, ecc83 fixture, llvmpipe):
the Rust hub's presenter (`native/host/src/present/viewer.rs`) draws only what has been ported; anything else is refused and,
without the Bend helper, answers `PRESENTER_UNAVAILABLE`. So most gaps are two-sided: a hub port of a Bend case
(`src/mobile/view.bend`, `src/core/layers.bend`, `src/core/mech.bend`) plus the client drawing.

Open, in order: sheet menu (client must keep `{"t":"sheets"}`, hub must port `view-sheet`), cross-probe (hub needs a
`kicad.info`-style query for all pieces of a net/part, or the client needs the info file), camera input, history, Files
previews, Renders, Review pages.


Agent read-back: `viewer_show` now answers `{shown, viewer: {open, key, part, sheet, layer, layerMode, selection, lit}, settled}`; the client reports its state with `viewer.state` whenever it changes (test `viewer_show_answers_with_what_the_viewer_then_shows`).
