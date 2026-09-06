---
name: k3eda-viewer
description: Point k3eda's KiCad panel at a project's board, schematic, and generated Gerber directories, and inspect its live saved-file views through the collaborative browser. Use when configuring or navigating the KiCad viewer, including projects with several boards or custom fabrication output locations.
---

# Point the KiCad viewer at this project

The KiCad panel follows the active thread's workspace (its worktree when one is in use). It reads saved files without creating KiCad locks. Editing remains in KiCad or the agent's filesystem tools. Unsaved editor buffers are not visible.

Run the helper next to this skill to discover candidates, including ignored build output:

```sh
python3 scripts/project_view.py /absolute/workspace
```

Resolve the script path relative to this skill, not the workspace. Inspect project scripts, `.kicad_pro`/`.kicad_pcb` plot settings, and `.kicad_jobset` when identifying the intended Gerber output. Do not pick an archived board just because it sorts first. If the user's intended board remains ambiguous, list the candidates and ask which one to use.

Save explicit selection in `<workspace>/.k3eda.json`. Paths are relative to the workspace, including in worktrees:

```json
{
  "pcb": "hardware/controller/controller.kicad_pcb",
  "schematic": "hardware/controller/controller.kicad_sch",
  "gerbers": ["build/controller/gerbers"]
}
```

The helper can write this after validating the paths; it preserves unrelated settings:

```sh
python3 scripts/project_view.py /absolute/workspace \
  --pcb hardware/controller/controller.kicad_pcb \
  --schematic hardware/controller/controller.kicad_sch \
  --gerbers build/controller/gerbers --write
```

Use existing outputs where possible. If none exist, generate **review-only** Gerbers in a separate output directory with `kicad-cli pcb export gerbers --output <directory>/ <board>`, then point `gerbers` there. This does not certify fabrication readiness. Preserve any manufacturer's output directory; don't overwrite it just to preview a board. For a manufacturing release, use the Gerber/export review workflow available in that environment.

# Inspect together

Open **KiCad** from the thread's right-panel add menu. The four views are **GERBERs**, **PCB**, **Schematic**, and **3D model**. Select a file or Gerber layer inside the view. Saved-file changes refresh automatically while visible; **Refresh saved files** forces a reread. A changed `.k3eda.json` updates the default selection.

For model interaction, use the panel's **Open KiCad viewer in browser** action. It opens the same viewer in T3's collaborative browser, where `preview_status`, `preview_snapshot`, `preview_click`, `preview_scroll`, and `preview_press` work. Target that viewer's `tabId` explicitly so other browser tabs stay intact. The panel iframe itself is not a `preview_*` automation target. If no viewer browser tab exists and only browser tools are available, ask the user to open that action once; do not invent a token or navigate to a bare viewer URL.

Take a snapshot before clicking. Outer view tabs and file/layer selectors have accessible names; Prism canvas geometry may need screenshot coordinates. The Gerber view supports pan, zoom, and fit. Use Previous/Next or left/right arrows to flip layers; inspection presets combine copper or front/back placement layers from the same fabrication set. PCB and schematic support reference/net search and **Show in schematic/PCB** after selecting a component. Double-click cross-probe actions from Prism also navigate to the matching view. Browser tooling can inspect open shadow roots for the PCB/schematic controls.

Viewer links grant temporary, read-only access to one workspace. Do not commit or log those links. If access expires or the server restarts, reopen the KiCad panel to mint fresh access.

# Troubleshooting

- Empty/wrong project: check the thread's worktree root and `.k3eda.json`; paths outside that root and symlinks are not followed.
- Stale view: save the editor file, refresh, and check the selected board/layer. Existing Gerbers change only when regenerated; native PCB edits do not rewrite a fabrication package.
- 3D: the environment running T3 needs `kicad-cli` with GLB export and access to the board's model libraries. Preview GLBs are generated in an isolated temporary cache. STEP/VRML component files are not themselves the board view.
- Gerbers: rendering uses Prism's original fabrication renderer and requires `python3` on the T3 environment. `.gbrjob` is a manifest, not a drawable layer. A malformed or empty layer is reported as such; do not claim a successful render.

For richer project review artifacts, the optional `scripts/cruncher_review.py`
helper runs the documented `kicad-cruncher design` command (and optionally
`pcb-svg`) through pinned `uv tool run`, writing to a temporary or explicitly supplied
directory. It runs against a disposable copy because Cruncher may generate a config beside its input; the editable project stays untouched. It does not install packages into the system environment:

```sh
python3 scripts/cruncher_review.py /path/to/board.kicad_pro --pcb-svg
```

KiCad Cruncher's `design` output includes per-copper-layer SVGs plus enriched
drill, slot, plating, and schematic metadata. `pcb-svg` adds configurable
assembly views. These are project-level artifacts and should be generated on
demand; the live Gerber endpoint remains the fast layer-flipping path.
