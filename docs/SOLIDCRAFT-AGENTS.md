# SolidCraft for agents

How agents model with SolidCraft: its own MCP server behind the hub, and the harness round it. The pane and the runtime are in
`native/host/src/solidcraft/`; this is `native/host/src/agents/solidcad.rs` and `solidcad/`.

## The tools

SolidCraft ships an MCP server, `solidcraft-cli mcp` (JSON-RPC over stdio). The hub does not hand that server to each agent CLI.
It runs one child per project and serves the child's tools from the hub's own MCP endpoint as `solidcraft_<tool>`
(`solidcraft_batch`, `solidcraft_measure`, ...), so that every provider gets them the same way, a thread on another machine is
routed like the KiCad tools (`agents/far.rs`), and every call passes through the harness. Tools that act on the person's app
rather than the design (`quit_app`, `preferences`, `agent_cursor`, `crash_reports`, `documents`) are not offered.

The harness adds five of its own: `solidcraft_look`, `solidcraft_summary`, `solidcraft_checkpoint`, `solidcraft_status` and
`solidcraft_call` (any upstream tool by name, a plugin's included).

A project gets the whole list when it holds a CAD file (`.sc`, `.solidcraft`, `.step`, `.stp`, `.iges`, `.igs`, `.stl`, `.3mf`,
`.obj`, `.glb`), a `mech` folder, no KiCad or Altium file yet, or has SolidCraft open in its pane. Any other project lists only
`solidcraft_call` and `solidcraft_status` (`harness::wanted`, applied in `agents/mcp.rs` `tools/list`). The tool definitions are
read from the program once, when it is installed; before that only the harness's own are listed.

## Headless or the pane

`solidcraft::embed::control(root)` gives the control port and token of the SolidCraft running in the project's pane. With one, the
child is started `mcp --connect 127.0.0.1:PORT` (the token in `SOLIDCRAFT_CONTROL_TOKEN`, never an argument) and every call acts
on the design in the window. Without one it is `mcp --headless` and works on the design's file, which the viewers follow. The mode
is checked before each call: when the pane opens or closes, the child is replaced and the answer says so. A headless child reopens
the design's file when it starts. Both get `--allow-path` for the project's design folder and the session's checkpoint folder, and
`--compact`.

## Around a call (`session.rs`)

| Step | What happens |
| --- | --- |
| Paths | `path` / `reference` arguments are made absolute from the project's design folder. |
| Snapshot | Files the call writes inside the project go through `files_write_guard`, as any project file write (the turn's snapshot; refused for a design file in review). |
| Checkpoint | Before a call that changes or replaces the design, the design's file is copied to `<home>/solidcad/<digest of the root>/<id>.sc` when its bytes differ from the newest checkpoint. The last 24 are kept. Machine-local, never in the project. |
| Budget | Each call has a time budget by kind (`harness::budget`: 20 s for control calls to 600 s for `open`, `export`, `batch`). |
| Stall | No answer inside the budget: bridged, the app is asked to `cancel` through a second server; the child is killed and replaced; headless, the design is reopened from its last save. The agent gets `{error, recovery, next}`. |
| Crash | The child ended: replaced the same way. A read is asked again of the new child; a change never is. |
| Interrupted | A changing call whose caller went away (the turn was interrupted) is said to the next caller, once. |
| Save | An edit that went through is saved at once when the design has a file, so the last save is at most one call old. |
| Digest | After a change: `inspect_design` and `measure {summary}` boiled down to bodies, volume, bounding box, failing features. |
| Look | When the geometry changed since the agent last saw it, `next` says to call `solidcraft_look`. |
| Stuck | The same tool and arguments failing three times in a row adds `stuck`: stop repeating, take another route. |

A hub on a copied log (`BACKPLANE_COPY=1`) writes no checkpoint and refuses the tools that write files.

## Provenance

The harness's ideas (a snapshot before each change, a dangling tool call resolved to an explicit error the model can act on, a
worker that outlives any one caller, parameter summaries, a render and inspect step in the loop) are common to chat-driven CAD
tools; CADAM (GPL-3.0) was read for which problems such a harness must solve. No code, text or structure was taken from it:
everything here is written from scratch in Rust against SolidCraft's documented MCP interface. The bundled `solidcraft-cad` skill
draws on SolidCraft's own agent documentation (Apache-2.0).

## Tests

`cargo test solidcad` runs the harness's pure functions and the session against a stand-in server (a shell script): an edit, a
stall, a crash, an interrupted caller, a repeated failure, the pane opening mid-session. With `BACKPLANE_SOLIDCRAFT_CLI` naming a
real `solidcraft-cli`, `cargo test real_solidcraft -- --ignored` models, measures, restores and exports a part.
