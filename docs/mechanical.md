# Mechanical projects

A Backplane project can be an ordinary folder containing mechanical parts.
It does not need a KiCad project or a board. The files and modelling tools
live on the machine running the hub, even when you use a browser or phone.

## View an existing model

1. Add the project folder to Backplane and open a thread in it.
2. Put the models you want in the parts list in a folder named `mech/` at
   the project's root. For example:

   ```text
   bracket-project/
     mech/
       bracket.step
       spacer.step
   ```

3. Open the viewer's **Mech** tab and select a part to turn it in 3D.
4. For still images, open **Renders**, then **Create renders**. Backplane
   writes isometric, front, top and right views under `mech/renders/`.
   The renders page also has **Refresh** to reload the parts list.

The supported model formats are `.sc`, `.solidcraft`, `.step`, `.stp`, `.iges`, `.igs`, `.stl`, `.3mf`, `.obj` and `.glb`. On Linux and Windows desktop, Backplane embeds SolidCraft directly in the **Mech** tab; on macOS, web and phones, the hub uses `solidcraft-cli` (to convert `.sc`/`.solidcraft` designs) and the configured STEP mesher (`backplane-step2glb`) for 3D viewing. Legacy FreeCAD documents (`.FCStd`) are not displayed directly — export or convert them to a supported format first.

A model can also stay elsewhere in the project. Open it from **Files** or **Project Home**,
or ask the agent:

> Show `vendor/spacer.step` on the Mech tab without changing it.

The agent can use `viewer_show` with the file's absolute path and
`kind: "mech"`. The viewer follows saves to that file (and live MCP edits when opened in embedded SolidCraft). Copy or export it
into `mech/` if you also want it in the project's parts list. The **3D**
tab is for the KiCad board; standalone models belong on **Mech**.

## Model a part with an agent

Install **SolidCraft** (`v0.50.0`) from onboarding or **Settings → CAD** (one-click download), or set `BACKPLANE_SOLIDCRAFT` when starting the hub; you can also install FreeCAD optionally for legacy `.FCStd` workflows (`BACKPLANE_FREECAD`). Existing GLB models can be viewed without a conversion.

Start with a small, concrete request, for example:

> Make an 80 by 30 by 3 mm rectangular mounting plate with two 5 mm
> through-holes, centred across its width and 60 mm apart along its length.
> Centre the pair on the plate. Keep the dimensions as named parameters
> in `mech/plate.sc`, export `mech/plate.step`, and show it on the Mech tab.

Backplane's mechanical instructions (`solidcraft_*` MCP tools and the bundled `solidcraft-cad` skill; see [SOLIDCRAFT-AGENTS.md](SOLIDCRAFT-AGENTS.md)) tell the agent to work in millimetres,
verify geometry and renders visually, keep named parameters in `.sc` designs, and export STEP/STL for manufacturing. Ask for changes to parameters or features to see live updates in the embedded **Mech** tab.

Keep the source and exports in Git if you want to track mechanical
revisions. The design history's per-tool KiCad snapshots and board or
schematic Compare do not provide a STEP geometry comparison.

Choose a [primary mechanical model](mechanical-model.md) when a project
has several parts and the assembly heuristic is not the preferred default.
