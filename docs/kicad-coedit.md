# Live co-editing with KiCad

With a co-editing runtime (Backplane KiCad bp.11+, capability `coedit`), agents edit the board and
schematic the person has open, at the same time as the person. The design is in the fork:
`docs/backplane/coedit.md` in BackplaneWorks/Backplane_KiCad.

What the hub does:

- **Actors.** `kicad_ipc` requests to the open editor carry the client name `agent:<thread>`
  (`kipc.rs` job `client`), so each thread's changes are its own undo group, change-log entry and
  attribution in KiCad. Agents' own scripts get `BACKPLANE_KICAD_ACTOR` and find the open
  editor's socket as `live.sock` beside the headless servers' (`embed.rs publish_live_sock`).
- **No waiting.** An agent's `SaveDocument` is no longer refused for the person's unsaved edits
  (the save includes both), and document locks are needed only for whole-document commands
  (`design.rs structural`). The prompt says never to ask the person to close KiCad, save, revert or
  wait, and never to write design files directly while KiCad has them open.
- **Visibility.** `SetAgentActivity` (the thread's title) on its KiCad calls, `idle` at its turn's
  end (`coedit.rs`). `ET_AGENT_CONTROL` from KiCad's Agents panel: pause holds the thread's KiCad
  writes until resume (the turn keeps running), steer goes into the thread.

Older runtimes keep the earlier rules (one writer per document, refused saves).

## What the person is doing

There is no accept or review step: an agent's kicad_ipc edits land in the open editor as the person's do, and the person undoes any of
them from KiCad's Agents panel (per author; their own Ctrl+Z never takes an agent's change back). So that an agent does not work over
the person, the hub keeps, per project, the person's selection (ET_SELECTION_CHANGED, bp.9+) and their own recent edits
(ET_DOCUMENT_EDITED with actor "", bp.11) for two minutes (`kicad/coedit.rs`). Every live kicad_ipc result an agent gets carries them
as `person` (`selected`: editor, items, seconds ago; `recent_edits`: document, description, item KIIDs, seconds ago). The prompt tells
the agent to ask the person first when its next change touches those items or their area, and to keep working on other things
meanwhile. Agents never start their own api-server or write the design files of a project the person has open: kicad_ipc is the one way
in, so nothing reloads under the person.

## Who a request is from

`kicad/coedit.rs` `client_name(&Who)` names every caller of the open KiCad, so its co-editing knows whose change it is:

| `Who` | client name | in KiCad |
|---|---|---|
| `Agent(thread)` | `agent:<thread>` | its own actor: the person's Ctrl+Z leaves it, UndoAgentChange / the Agents panel take it back, the Agents pane opens for it |
| `Person` | `person:backplane-<pid>` | the person's own change (bp.13+: a `person:` name acts as the person), so their Ctrl+Z takes it back: BOM panel edits, fab-profile Apply, panel buttons |
| `Hub(job)` | `hub:<job>` | the hub's own housekeeping (3D-model migration): an actor of its own, never the person's, so it never eats a Ctrl+Z |

Send with `embed::gui_call_as(route, type, message, &client_name(&who))`. A call with no name goes out as the transport's own
`backplane-<pid>`, an actor of its own. Before bp.13 KiCad has no person name: `person:` is an actor of its own too, so a
person-originated schematic edit is not in their undo on bp.10 to bp.12.
