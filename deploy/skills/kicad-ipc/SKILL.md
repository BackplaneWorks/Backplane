---
name: kicad-ipc
description: "Read only when kicad_edit lacks a required operation and raw KiCad IPC is needed."
---

# kicad-ipc

- Call `kicad_ipc_status` for the installed API, capabilities and command names. Send one command at a time with `kicad_ipc {path, type, message}`. Short types like `GetItems` work. The hub opens the document and manages its named lock.
- Target explicit item IDs. Headless editors have no selection or view state. Wrap mutations in `BeginCommit` and `EndCommit`, then `SaveDocument` at each milestone. Use `EndCommit` with `CMA_DROP` to discard an unfinished commit. Check `GetNotices` after a deferred change.
- If the runtime lacks `footprint-partial-update`, send the full FootprintInstance. If it lacks `symbol-from-libid`, use `LoadAllLibraries`, `GetLibraryStatuses`, then `PlaceSymbolFromLibrary`.
- With `runaction-frame`, set `RunAction.frame` to `FT_SCHEMATIC_EDITOR` or `FT_PCB_EDITOR` for actions shared by both editors, including undo and redo. Older builds can send a shared action to the board editor while it is open.
- Use the hub's IPC connection. If IPC is unavailable, report the tool's reason and follow its supported recovery. Direct file writes are forbidden while KiCad has the project open.
