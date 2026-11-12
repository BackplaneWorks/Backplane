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
