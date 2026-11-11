# STEP meshing (Mech tab)

A STEP is meshed by `backplane-step2glb` (`native/step2glb/step2glb.cpp`): OpenCascade 7.9.2, natively, beside the hub
(`kicad/stepglb.rs`). FreeCAD is the last resort; Bun is not in this path any more.

## How it works

- **Read:** `STEPCAFControl_Reader` with colours on, into an XCAF document. The reader's own shape healing runs one solid after another
  inside the transfer, and it was about 90% of a big file's transfer time (callgrind on the SOT-23 card). So the reader transfers without
  it, and each distinct solid is healed afterwards with the reader's own parameters (`XSAlgo_ShapeProcessor`, FixShape), several at once.
- **Mesh:** each distinct solid (an instanced part once) is healed and meshed by `BRepMesh_IncrementalMesh` on its own thread, biggest
  first, at the hub's `NORMAL` detail (0.03 mm, 0.2 rad). With fewer solids than cores, each one's faces are meshed in parallel too.
- **Colours:** a face's own colour, else its solid's, else a tint from the palette `freecad_gui.py` uses. The colours move from each
  unhealed face to what healing made of it, through the processor's map. FreeCAD's default grey-blue counts as no colour.
- **Output:** the same GLB the FreeCAD scripts write (mm, Z up, one primitive a colour, linear light), so the hub reads both the same way.
- **Streaming:** with `--parts DIR`, each batch of placed solids is written as its own GLB the moment its meshes are done (about 20,000
  triangles, or 60 ms, a batch) and announced on stdout. The hub (`watch.rs` `run_native`, `step_stream`) sends each batch to the views
  as the board's part frames, in the model's own integer coordinates (`solid::part_mesh`). The whole model follows and replaces them, and
  no part frame is ever sent after it (`a_steps_streamed_parts_never_come_after_its_model`). The desktop camera follows a model that grows
  until the person moves it. Phones draw parts only over a model, so they show the whole model when it comes.
- **Cache:** unchanged. A model is kept under the sha256 of its file in the models cache (`plat::dirs::models_cache`, one native folder
  on each system, #1102). A STEP the turn-end design commit records is meshed in the background (`agents/design.rs`), and so are a shown
  part's neighbours in `mech/` (`warm_mech`).
- **No shell:** the hub starts the helper itself and reads its output line by line. A guard stops it after 600 s and wakes the moment its
  output ends.

## Building

`scripts/build-step2glb.sh [OUT]` uses the system's OpenCascade for a dev build (`libocct-*-dev`; kept between builds of the same
sources, and `dev-build.sh` puts it in every cut). With `STATIC=1` it uses `scripts/build-occt.sh`'s pinned, static OCCT (only the
toolkits used; about 3 minutes on this desktop, 4.5 on the Mac mini), as `release.yml` does on each platform. Results:

| | Linux x64 | macOS arm64 | Windows x64 (MSVC) |
|---|---|---|---|
| program, static | 30 MB stripped, needs only libc and libstdc++ | 26 MB, system frameworks only | 15.6 MB |

The Bun helper was 111 MB. Notices: `deploy/licenses/step2glb`.

## Numbers (2026-11-11, this desktop, 32 cores, under other lanes' load)

The helper alone, against what ran before: FreeCAD's GUI offscreen (the hub's default until now) and the Bun/WASM `backplane-step2glb`.
Triangles are the same from all three, since both FreeCAD and native mesh with OCCT's BRepMesh at the same deflection.

| file | MB | FreeCAD GUI s | Bun s | native s | triangles | colours FreeCAD / Bun / native |
|---|---:|---:|---:|---:|---:|---|
| fab_reel_arm | 0.05 | 4.40 | 0.25 | 0.02 | 796 | 1 / 1 / 1 |
| scs_beam_bracket | 0.06 | 4.33 | 0.14 | 0.03 | 1428 | 1 / 1 / 1 |
| usb_receptacle | 0.16 | 4.31 | 0.20 | 0.04 | 628 | 3 / 3 / 3 |
| fab_right_plate | 0.23 | 4.44 | 0.25 | 0.08 | 4032 | 1 / 1 / 1 |
| usb_slide_switch | 0.23 | 4.40 | 0.21 | 0.04 | 180 | 4 / 4 / 4 |
| scs_drawer_deep | 0.36 | 4.62 | 0.30 | 0.06 | 4304 | 1 / 1 / 1 |
| sot23_card | 1.14 | 7.60 | 1.03 | 0.19 | 67132 | 7 / 2 / 7 |
| fab_sprocket_wheel | 3.62 | 5.31 | 1.90 | 0.69 | 25436 | 1 / 1 / 1 |
| fab_feeder_assembly | 5.61 | 6.43 | 2.78 | 1.05 | 60382 | 21 / 21 / 21 |
| scs_pandaplacer_bay | 5.75 | 6.87 | 2.82 | 0.57 | 109540 | 8 / 8 / 9 |
| scs_workbench_standing | 8.52 | 7.96 | 4.88 | 1.03 | 205640 | 8 / 8 / 9 |
| usb_sniffer_board | 9.48 | 10.16 | 6.63 | 1.50 | 170666 | 15 / 5 / 15 |

Through a hub (`test/files/step_timing.mjs`): from the view's watch request to the first streamed part and to the whole model. Each file
is alone in a project of its own, measured cold (empty cache), reopened on the same hub, and opened on a freshly started hub over that cache.

| file | FreeCAD: model | native: first part | native: model | part frames | reopen | after a hub restart |
|---|---:|---:|---:|---:|---:|---:|
| fab_reel_arm | 3756 ms | 11 ms | 12 ms | 1 | 0 ms | 1 ms |
| scs_beam_bracket | 3865 ms | 14 ms | 14 ms | 1 | 1 ms | 1 ms |
| usb_receptacle | 3806 ms | 21 ms | 21 ms | 1 | 1 ms | 1 ms |
| fab_right_plate | 3857 ms | 43 ms | 43 ms | 1 | 1 ms | 2 ms |
| usb_slide_switch | 3957 ms | 26 ms | 26 ms | 1 | 0 ms | 1 ms |
| scs_drawer_deep | 3908 ms | 42 ms | 43 ms | 1 | 0 ms | 2 ms |
| sot23_card | 7387 ms | 79 ms | 124 ms | 3 | 1 ms | 10 ms |
| fab_sprocket_wheel | 5467 ms | 401 ms | 405 ms | 1 | 1 ms | 8 ms |
| fab_feeder_assembly | 5922 ms | 278 ms | 651 ms | 3 | 1 ms | 15 ms |
| scs_pandaplacer_bay | 5827 ms | 319 ms | 417 ms | 6 | 2 ms | 23 ms |
| scs_workbench_standing | 7244 ms | 475 ms | 788 ms | 11 | 2 ms | 41 ms |
| usb_sniffer_board | 9685 ms | 489 ms | 969 ms | 8 | 2 ms | 31 ms |

The client's share, on those frames (`real_frames_decode_and_draw_in_time`, fast profile), all on its render thread, none on the UI
thread: decode with the normals up to 43 ms (168,000 triangles), the GPU's vertices up to 5 ms, a CPU picture at 1280×800 up to 18 ms.
As parts arrive, the CPU drawer (macOS, Windows) appends each new layer instead of copying every layer again.

On the Mac mini, the helper alone: reel arm 10 ms, SOT-23 card 100 ms, feeder 0.49 s, USB Sniffer board 0.85 s. On bp-win (4 vCPU,
shared at the time): 82 ms, 0.45 s, 1.66 s and 3.1 s, with the same triangles as on Linux.

## Parity

Colours, compared triangle for triangle against FreeCAD's GUI output:

- They are equal on the parts and on the feeder assembly (21 colours).
- Native keeps colours FreeCAD loses. On the USB-C receptacle the STEP says `COLOUR_RGB(0.8, 0.8, 0.82)`: FreeCAD drew a dark
  (0.168, 0.192, 0.216), native draws the file's silver. The same happens on the slide switch and on one solid of the pandaplacer bay.
- The Bun helper lost colours: 2 of the SOT-23 card's 7, and 5 of the board's 15.

FreeCAD placed every component of the SOT-23 card and of the USB Sniffer board at the origin, away from the board. Native places them on
it. Renders of both: the hub's `--render` of each GLB.

Normals are the client's own (`smooth_normals`), from the same positions, so they shade the same.
