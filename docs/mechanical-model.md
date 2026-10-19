# Primary mechanical model

When a project has several mechanical parts, choose which one the Mech tab
opens first with the optional `mech` field in `.backplane.json` at the
project root:

```json
{
  "mech": "mech/fixture.step"
}
```

Add the field to the existing JSON object if the file already contains
settings. It does not change the project's KiCad file selection.

The path is relative to the project root and must exactly match a model
listed in Mech, including its `mech/` prefix and letter case. STEP
(`.step` or `.stp`) and GLB (`.glb`) models are supported. For example:

```text
project/
  .backplane.json
  mech/
    bracket.step
    fixture.step
    fixture-asm.step
```

With the setting above, `fixture.step` opens before `fixture-asm.step`.
Without a valid setting, Backplane chooses an assembly-named model, then
the first model in the sorted list. Assembly names contain `assembl` or
end in `-asm`, ignoring case and the file extension.

A part you or an agent explicitly open takes precedence over this default.
Refresh in Mech rereads the setting and the parts list without replacing
the model already selected in the thread. The setting applies when no
model has been chosen.

A missing file, invalid JSON, a non-string `mech` value, or a path that
does not match a listed model falls back to the usual assembly/first-model
choice. Absolute paths and paths outside `mech/` cannot select a default.
