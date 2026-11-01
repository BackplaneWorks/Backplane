# README assets

The README assets were prepared in the isolated `docs/readme-rewrite` worktree. The user approved merging the documentation and requested `README.jp.md` with language buttons. This work does not install or deploy the application.

| Assets | Source and editable files |
| --- | --- |
| RoyalBlue viewer montage | Current 2026-11-01 desktop/web frames at native 2x, captured by [A-capture.mjs](source/A-capture.mjs) and losslessly composed by [A-export.py](source/A-export.py). Same two-upper/one-lower montage; current vertical tool rail. Original RoyalBlue PCB unchanged. Earlier renderer notes are archival. |
| Agent picker | Current three-column Agent / Model / Effort picker over the composer, native 2x. Includes OpenCode, Pi and Oh My Pi. Empty blocked-agent test home offers default plus the seeded thread model; no live provider catalog was fetched. [Source frame](source/A-picker-frame.png). |
| CAD selection, mechanical viewer, settings, cats | [CAD provenance](cad/provenance.json), including current CAD captures and clearly separated legacy records. The README mechanical screenshot is now fresh, with a synthetic case thread; original cat art is unchanged. |
| Orchestration, delegation, webhooks, Mailroom, Space | [Capture provenance](orchestration/PROVENANCE.md). Actual UI with public demo fixtures; seeded reviews are illustrative content. |
| References, terminal, btw, link chips | [Feature provenance](features/PROVENANCE.md). The terminal is a real local vi session; the side question was not submitted. |

[render-previews.mjs](source/render-previews.mjs) renders the editable HTML compositions and the current README with Chromium. Run it with Bun from the docs worktree. `README_COMPARE=1` also generates the earlier 150% and 175% presentation options. `source/preview.html` and `readme-200.png` are generated review outputs.

[references.html](source/references.html) vertically stacks lossless menu crops from the four actual `features/reference-*-2x.png` captures, including each typed sigil. These feature captures were refreshed on 2026-11-01 from build 0.13.2-dev.20261101.1606 (main 36e6a1024), at a real device scale factor of 2. See [feature provenance](features/PROVENANCE.md) for the isolated fixture, terminal calls, and lossless export script.

The Mailroom image was refreshed on 2026-11-01 from the current web client at real 2× pixel density. `orchestration/mailroom.png` and `mailroom-200.png` are byte-identical 2672 × 1128 lossless crops; no enlargement is used. [mailroom.html](source/mailroom.html) now presents the crop at its exact pixel size. See the [current orchestration provenance](orchestration/PROVENANCE.md) and [selection manifest](orchestration/source/current-selection.json).

The footer uses the existing three cat poses at a small displayed size. No AI imagery or recreated application chrome is used.

## Review dependencies

The refreshed internal-link screenshot uses the supplied current build. It renders plain link labels rather than the older bordered chips. Thread and bot navigation were verified by clicks; the project-link click selected the already-current Sensor Board project. See [feature provenance](features/PROVENANCE.md) for the evidence and bilingual README alt-text suggestions in the agent C results.

OpenCode support in the user-directed paragraph is supported by the newer release-preparation source at `ac2b41fd275cca1c2e607ee04b8bb606b0b78b5b`, including its ACP runtime implementation. It is not inferred from the older installed provider-picker screenshot. [Feature provenance](features/PROVENANCE.md) preserves the source excerpts and runtime distinctions.

Reference sigils in code and actual menus are `>` (thread), `#` (room), `%` (project), and `@` (bot). The requested caret was corrected to `@`; no caret support is implied.

## Current A captures (2026-11-01)

Build `0.13.2-dev.20261101.1606`, main `36e6a1024`. [Fresh synthetic fixture](source/A-demo-events.jsonl), [isolated hub runner](source/A-hub.sh), [CDP driver](source/A-drive.mjs), [capture interactions](source/A-capture.mjs), and [lossless export rectangles](source/A-exports.json). No agent ran, no network was available, and source CAD was unchanged. Old HTML recipes and older source frames remain archival; the five A README exports use the current recipes instead. Round 2 replaces the pad image with one contiguous native-2x crop of actual Split-view cross-probing: J2 pad 1 selects +BATT in the Connectors schematic and board, with the probe chip visible and no composer draft. Settings now discovers KiCad automatically under `/tmp/readme-demo/home`, with no path override. [Round-2 fixture](source/A-round2-demo-events.jsonl), [private mount/network runner](source/A-round2-hub.sh), and [capture script](source/A-round2-capture.mjs) supersede the round-1 recipes for these two images. The separate NFC antenna subproject is excluded from an otherwise byte-identical public demo copy to avoid this build’s multi-board Split navigation reset. The three accepted round-1 images are unchanged. See [CAD capture notes](cad/source/A-capture.md).
