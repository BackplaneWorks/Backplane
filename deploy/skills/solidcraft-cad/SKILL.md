---
name: solidcraft-cad
description: "Use for modelling, editing, measuring, rendering or exporting mechanical parts with SolidCraft (the solidcraft_* tools): .sc designs, STEP, STL, 3MF."
---

# solidcraft-cad

SolidCraft is the parametric CAD editor behind Backplane's 3D pane. The backplane tools `solidcraft_*` are SolidCraft's own MCP tools, run by Backplane: one session per project. When SolidCraft is open in the person's pane, the session acts on the design in that window and they watch each change; otherwise it is headless and works on the design's file. `solidcraft_status` says which. Units are millimetres and degrees, Z is up.

Files: keep each part as `mech/<part>.sc` (the design, with its parameters and timeline) and export `mech/<part>.step` beside it (and `.stl` or `.3mf` for printing). Paths may be given from the project root. A STEP that is opened has no design file until you `solidcraft_save {path: "mech/<part>.sc"}`. Show the result to the person with `viewer_show`.

## The loop

1. Read. `solidcraft_skill_reference {skill: "solidcraft-modeling"}` holds the command shapes, ids and known traps of this version: read it before the first build (`solidcraft-editing` when changing an existing design, `solidcraft-checking` for tests, `solidcraft-mechanisms` for joints). Then `solidcraft_summary`: the file, parameters, features and mass properties of what is open. If a design with features is open and it is not the one you were asked about, ask before `solidcraft_open` or `solidcraft_new_design`: both replace it.
2. Plan in numbers: the parameters, the placement (centre the part on the origin), each feature, and the volume and bounding box you expect after each stage.
3. Model one stage per `solidcraft_batch` (`verbose: false`; all-or-nothing, the reply names the failing step). Name every feature. Sizes that must follow a parameter are expressions (`"w/2 - 6"`), never plain numbers. Put the expected values in `expect` (`volume_mm3`, `bodies`, `bbox`) so a miss is an error, not a silent success. Put a risky step (a fillet over many edges, a boolean between touching curved faces) in a batch of its own.
4. Look. Every answer to a change ends with a `backplane` block: `design` (bodies, volume, bounding box, `failing` features, whether it is `unsaved`), the `checkpoint` taken before it, where it was `saved`, and `next`. When `next` says the geometry changed since you last saw it, call `solidcraft_look` (iso, front, top and right in one answer) and compare the pictures with the intent: proportions, positions, holes that go through, nothing missing or doubled. A cut that leaves the volume unchanged has missed.
5. Fix, do not rebuild: `solidcraft_set_parameter` for a user parameter, `execute parameters.change` for a feature input, `execute timeline.edit` for structure. `solidcraft_check_parameters` tries values and puts the design back.
6. Finish: `solidcraft_summary` once more (no failing feature, every body closed, the mass plausible), `solidcraft_export` the STEP, then `viewer_show` it.

## What Backplane does around each call

- Checkpoint: before a call that changes the design, its file is copied to a checkpoint outside the project. `solidcraft_checkpoint {action: "list"}` shows them, `{action: "restore", id}` puts one back and reopens it (the current state is kept as a checkpoint first), `{action: "save", note}` marks a state you may want back. Inside one session, `execute edit.checkpoint` and `edit.restore_checkpoint` also work, and a failed `batch` undoes itself.
- Save: once the design has a file, a change that goes through is saved at once. Give a new design its file early.
- A stalled kernel: a call that does not answer in time is ended, the session is replaced and the design is reopened from its last save. The error says so under `recovery`. Do what `next` says: `solidcraft_summary` first, then send the change again in a smaller or simpler form. Never resend the same call blindly.
- An interrupted call: if an earlier call was cut off, the next answer says it may or may not have been applied. Check before relying on it.
- `stuck`: the same call failed three times in a row. Stop repeating it: read the error, `solidcraft_describe_command {id}`, restore a checkpoint, take another route.
- A project with nothing mechanical yet lists only `solidcraft_call {tool, arguments}` and `solidcraft_status`: every tool is reachable through `solidcraft_call` by its upstream name (`batch`, `measure`, `open`, ...).

## Traps

- `solidcraft_list_commands` always with a `filter`, `prefix` or `panel` (and `compact: true`): the whole catalog is very large.
- Picks (`edges: [[x, y, z]]`, `faces: [[x, y, z]]`) are plain numbers that snap to the nearest edge or face: compute the exact point, or take it from `solidcraft_body_topology`.
- Fillet outside edges before a shell; build inside features after it; then holes and cuts through walls.
- An open body has no volume and does not print: `design.open_bodies` in the answer names the count.
- `viewer_render` gives pictures of a file on disk; `solidcraft_look` gives the session's design as it is now, saved or not.
- These tools work on this machine's files. For a project on another machine, start a thread there (`thread_launch {machine}`).
