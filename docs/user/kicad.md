# View KiCad projects

T3CAD includes KiStack's nine electronics skills automatically for every project and agent provider. Ask your agent to use them for schematics, symbols, footprints, BOM review, PCB review, exports, Gerbers, panelization, or product renders. No separate skill installation is needed. Tools required by a workflow, such as KiCad or Blender, must still be installed on the connected environment.

Open **KiCad** from a thread's right-panel add menu to inspect its PCB, schematic, Gerber layers, and 3D board using KiCAD-Prism's viewers. The panel follows the thread's workspace or worktree. It reads saved files without locking them; you can continue editing in KiCad or through your agent.

On mobile, open **KiCad** from the thread toolbar. The same read-only viewer opens in a full-screen native web view, with its PCB, Gerber, schematic, and 3D tabs available at the top. The viewer uses a short-lived session tied to the active environment; reconnect and tap **Retry** if that session expires.

For a workspace containing several boards, create `.k3eda.json` at its root:

```json
{
  "pcb": "hardware/controller.kicad_pcb",
  "schematic": "hardware/controller.kicad_sch",
  "gerbers": ["build/gerbers"],
  "symbol": "hardware/lib/parts.kicad_sym",
  "symbolMember": "Controller_unit1",
  "footprint": "hardware/lib/parts.pretty/Controller.kicad_mod"
}
```

Paths are relative to the workspace. These assignments select the design shown in the viewer; the agent and viewer can use the same files. Without assignments, the viewer opens an unambiguous project and leaves tools, examples, and routing intermediates out of automatic selection. Use **Browse** to preview another workspace file, and **Return to assigned design** to clear that temporary choice. PCB and 3D share a selection, as do schematic and BOM. Only referenced child sheets load with the selected schematic.

Generated output is discovered even in Git-ignored folders. Saved changes refresh while the viewer is visible. Gerbers show the existing generated package; editing the board does not regenerate that package.

Use **STEP** to inspect `.step` and `.stp` models. The project picker lists the most recently edited files first; **Open file** also accepts a file from your device without uploading it. Drag to rotate, pinch or scroll to zoom, and use **Fit model** to restore the view. STEP previews run in the viewer and support files up to 100 MB.

The 3D preview requires `kicad-cli` with GLB export on the environment running T3. The preview includes outer copper, pads, silkscreen, and translucent soldermask using the board’s stackup colors. Exports go into a separate temporary cache. Gerber rendering requires `python3` there. Neither operation modifies the project.

Use **Open KiCad viewer in browser** to let your agent inspect and interact with the viewer through the collaborative browser. Its link grants temporary read-only access to this workspace; reopen the panel after the link expires or the server restarts. The `k3eda-viewer` agent skill includes a helper for finding and configuring project files.

Find a component by its reference (such as `U1`), or enable **Net** to search by net name. Select a component and use **Show in schematic/PCB** to inspect its counterpart without losing the other view’s camera.

In Gerbers, use the arrow buttons or left/right arrow keys to flip layers. **Copper layers**, **Front placement**, and **Back placement** combine aligned layers from the current fabrication set. The viewer caches rendered layers and warms adjacent layers for faster switching. Placement presets use available silkscreen, fabrication, and paste artwork; they are not a component BOM or a substitute for populated assembly inspection.

Use **Tools** inside the KiCad viewer to open **BOM**, **Footprints**, **Symbols**, or **EMerge / Analysis**. Close an optional tab with its close button and reopen it from the same menu. These views are shared by web, desktop, and mobile.

The BOM follows the selected schematic and the current saved BOM settings in its matching `.kicad_pro`: visible fields, column labels, grouping, sorting, filtering, DNP handling, and output formatting. Changes saved in KiCad refresh the table. Select the root schematic when using a hierarchical design. If the matching project has no saved BOM settings, the viewer explicitly reports that it is using KiCad defaults. Search the table or download the exported BOM. Export runs against a temporary copy and leaves project files untouched; it is a preview, not a manufacturing release check.

Footprints (`.kicad_mod`) and symbol libraries (`.kicad_sym`) are discovered within the workspace. Select a library and then a symbol/unit to inspect its SVG preview. The BOM and library previews require `kicad-cli` on the connected environment.

The analysis view creates a planar antenna starter specification with feed and ground contacts and one or two target bands for the antenna-rl workflow. Export or copy the specification for your agent to use. EMerge results can be embedded by adding an `analysisUrl` to `.k3eda.json`, using a dashboard address reachable from every device that will open the viewer:

```json
{
  "analysisUrl": "https://antenna-dashboard.example.com"
}
```

The dashboard service must allow embedding; **Open dashboard** opens it separately. The openEMS option currently prepares a specification only; an openEMS execution adapter is not included. Exporting a specification does not start a solver or training job or qualify an antenna design.
