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

The supported model formats are `.step`, `.stp` and `.glb`. FreeCAD is not
required to inspect an existing STEP model. FreeCAD documents (`.FCStd`)
and STL meshes are not displayed directly; export or convert them to a
supported format first.

A model can also stay elsewhere in the project. Open it from **Files**,
or ask the agent:

> Show `vendor/spacer.step` on the Mech tab without changing it.

The agent can use `viewer_show` with the file's absolute path and
`kind: "mech"`. The viewer follows saves to that file. Copy or export it
into `mech/` if you also want it in the project's parts list. The **3D**
tab is for the KiCad board; standalone models belong on **Mech**.

## Model a part with an agent

Install FreeCAD on the hub's machine. Backplane looks for `freecadcmd`,
`FreeCADCmd` or `freecad.cmd` on `PATH`, and for FreeCAD's macOS app bundle.
If it is installed elsewhere, set `BACKPLANE_FREECAD` to its console
executable when starting the hub.

Start with a small, concrete request, for example:

> Make an 80 by 30 by 3 mm rectangular mounting plate with two 5 mm
> through-holes, centred across its width and 60 mm apart along its length.
> Centre the pair on the plate. Keep the dimensions as named parameters
> in `mech/plate.py`, save `mech/plate.FCStd`, export `mech/plate.step`,
> and show it on the Mech tab.

Backplane's mechanical instructions tell the agent to work in millimetres,
with Z up and the front facing -Y, check the shape's validity, and keep
the script, FreeCAD document and STEP export together. Ask for changes to
the script and regenerate the export to see each revision.

Keep the source and exports in Git if you want to track mechanical
revisions. The design history's per-tool KiCad snapshots and board or
schematic Compare do not provide a STEP geometry comparison.
