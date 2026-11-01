2026-11-01, build `0.13.2-dev.20261101.1606`, main `36e6a1024`.
All screenshot pixels are native 2x from the current desktop/web UI. Export
rectangles are in `../../source/A-export.py` / `A-exports.json`; run the exporter
with `--round2` to overwrite only the two revised images.

Round 2 supersedes round 1 for `A-pad-frame.png` and `A-settings-frame.png`.
Reproduce with `../../source/A-round2-seed.py`, `A-round2-hub.sh start`,
`A-round2-hub.sh desk`, then `A-round2-hub.sh node <absolute A-drive.mjs> run
<absolute A-round2-capture.mjs>`, and finally `A-round2-hub.sh stop`.
Use a fresh `run/A-round2` folder. The original synthetic fixture is reused by
`A-round2-demo-events.jsonl`. Empty stand-in agents prevent real turns.

The runner uses private user, network and mount namespaces. A private tmpfs on
`/tmp` contains a bind mount of `run/A-round2` at `/tmp/readme-demo`; no host `/tmp`
files are written. All persistent files stay under the assigned scratch root.
KiCad is symlinked to the supplied test runtime and automatically discovered
under `/tmp/readme-demo/home/kicad/10.99.0-bp.4/bin/kicad-cli`. There is **no
KiCad path setting**: the empty input shows its actual Automatic placeholder,
with status “from installed”. The input naturally clips the last characters of
the long placeholder. FreeCAD remains `/usr/bin/freecadcmd`; IPC is on. The
heading/rows use exactly the round-1 crop/join, 1676 × 582.

The public RoyalBlue project is copied byte-for-byte into the run folder,
excluding its separate `nfc_antenna` subproject. This supplied build resets the
Split schematic pane to the top sheet when multiple boards are indexed;
excluding the separate antenna board allows actual sub-sheet navigation.
The main PCB, schematic and models are unchanged. In Split, a real pointer click
on J2 pad 1 yields `n\t+BATT\tJ2.1`. The Connectors sheet and board both highlight
the net; the real `Net +BATT` probe chip is visible. The 3D pane is hidden to keep
the crop wide and readable. `pad-workbar-detail-200.png` is **one contiguous
3598 × 1228 crop** of `A-pad-frame.png`, with no resizing or compositing.
No composer draft is typed, no mention is generated and no probe is sent to an
agent. The old native “Mention in chat” interaction is not claimed.

The accepted round-1 mechanical capture is unchanged. `A-mech-frame.png` shows
`mech/cm5-case-assembly.step` from the supplied project copy. Its simple populated
board is part of that existing STEP. No model or geometry was edited. The richer
`case/out/case-assembly.glb` was rejected by this viewer's 12 MB input limit.
The round-1 runtime, fixture and other capture recipes remain under
`../../source/A-*` for the three accepted images.
