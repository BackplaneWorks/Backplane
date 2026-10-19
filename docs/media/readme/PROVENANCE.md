# README assets

The README assets were prepared in the isolated `docs/readme-rewrite` worktree. The user approved merging the documentation and requested `README.jp.md` with language buttons. This work does not install or deploy the application.

| Assets | Source and editable files |
| --- | --- |
| RoyalBlue viewer montage | Actual Backplane frames composed by [viewers.html](source/viewers.html). Corrected board/3D frames and renderer validation are recorded in [castellation-render.json](source/castellation-render.json). Original RoyalBlue PCB unchanged. |
| Agent picker | Native screenshot cropped by [agent-setup.html](source/agent-setup.html). This older installed picker does not show OpenCode. |
| CAD selection, mechanical viewer, settings, cats | [CAD provenance](cad/provenance.json), including the archival mechanical-view screenshot and original cat art. |
| Orchestration, delegation, webhooks, Mailroom, Space | [Capture provenance](orchestration/PROVENANCE.md). Actual UI with public demo fixtures; seeded reviews are illustrative content. |
| References, terminal, btw, link chips | [Feature provenance](features/PROVENANCE.md). The terminal is a real local vi session; the side question was not submitted. |

[render-previews.mjs](source/render-previews.mjs) renders the editable HTML compositions and the current README with Chromium. Run it with Bun from the docs worktree. `README_COMPARE=1` also generates the earlier 150% and 175% presentation options. `source/preview.html` and `readme-200.png` are generated review outputs.

[references.html](source/references.html) vertically stacks lossless menu crops from the four actual `features/reference-*-2x.png` captures. It removes repeated blank composer areas without drawing replacement UI. The source captures are 200% nearest-neighbor enlargements, as documented by the feature capture manifest.

[mailroom.html](source/mailroom.html) enlarges the native `orchestration/mailroom.png` crop exactly 2× in Chromium with pixelated image scaling, yielding `mailroom-200.png` at 2672 × 880. This is a presentation enlargement, not a native HiDPI capture. Its unchanged input hash and crop rectangle are recorded in [selection.json](orchestration/source/selection.json).

The footer uses the existing three cat poses at a small displayed size. No AI imagery or recreated application chrome is used.

## Review dependencies

The internal link-chip screenshot uses an existing development candidate. It demonstrates rendering; activation was not verified by this documentation work. Another owner's PR259 tracks activation work. Keep that dependency visible when reviewing publication readiness.

OpenCode support in the user-directed paragraph is supported by the newer release-preparation source at `ac2b41fd275cca1c2e607ee04b8bb606b0b78b5b`, including its ACP runtime implementation. It is not inferred from the older installed provider-picker screenshot. [Feature provenance](features/PROVENANCE.md) preserves the source excerpts and runtime distinctions.

Reference sigils in code and actual menus are `>` (thread), `#` (room), `%` (project), and `@` (bot). The requested caret was corrected to `@`; no caret support is implied.
