# Pi and Oh My Pi

> Paths under `src/` below (`src/core`, `src/server`, `src/web`, `src/mobile`, `src/app`) name the Bend JS tree, removed from this checkout (tag `main-bend-final`). Where the current Rust code for each is, is unverified; look under `native/host/src/` and `native/gpui/`.

Backplane can run two more agents, `pi` and `omp`. They are separate
programs with different protocols. Backplane uses each one's own protocol,
not OpenCode's and not each other's.

| | Pi | Oh My Pi |
|---|---|---|
| Project | earendil-works/pi (`@earendil-works/pi-coding-agent`), MIT | can1357/oh-my-pi (`@oh-my-pi/pi-coding-agent`), MIT |
| Verified against | 1.4.0 | 18.12.6 |
| Run as | `pi --mode rpc`, Pi's JSONL RPC | `omp --mode rpc-ui`, its own RPC (a Pi dialect) |
| Code | `src/core/pi` | `src/core/omp` |

## Install and sign in

- **Pi:** install it from https://pi.dev and put `pi` on PATH. Run `pi` once and use `/login`, or set the provider's API key.
- **Oh My Pi:** install it from https://github.com/can1357/oh-my-pi and put `omp` on PATH. Run `omp` once and use `/login`.

Backplane never writes either agent's settings or credentials. They stay in `~/.pi/agent` and `~/.omp`. Pi's sessions stay there too; omp's for Backplane's threads live in Backplane's home, a folder per thread (`<home>/omp/sessions/<thread>`).

OMP receives Backplane's masked system prompt through a private UTF-8 file
passed to `--append-system-prompt`. Each turn has independent launch files,
removed after its process exits. This preserves large skill lists without
exceeding Windows' command-line limit; no symlink or Developer Mode is needed.

Each provider appears in every model picker once its CLI is on PATH. The models come from `pi --list-models` (run offline) and from `omp models --json`. Both list only models they have credentials for. If no models show, sign in first.

## What a thread gets

- **Sessions:** a thread keeps its session from turn to turn.
  - Pi gets `--session-id <Backplane's id>` and resumes without replaying history.
  - omp gets the thread's own session folder (`--session-dir`) with `--continue`, which takes up its newest session (a new one in an empty folder).
  - If the agent no longer has the session, the turn says so once as an info row. The agent then starts without the earlier conversation.
- **Instructions:** Backplane's instructions go in with `--append-system-prompt` at every start. They never enter the conversation. Each agent also reads `AGENTS.md` and `CLAUDE.md` in the thread's folder.
- **Backplane tools:**
  - Pi gets the thread's Backplane MCP server from Backplane's extension, which is written to `<home>/pi/backplane.js` and loaded with `-e`.
  - omp gets them from Backplane's extension (`<home>/omp/backplane.js`, `-e`): it lists the thread's Backplane MCP tools and registers each under the name Claude sees (`mcp__backplane__<tool>`), calling the MCP server for it. They ask for no approval (the hub has its own rules for them).
- **Effort:**
  - Pi takes `--thinking`. The picker offers off through high for models that think.
  - omp takes `--thinking`, with the levels it reports per model, plus off and auto.
- **Stop** ends the agent's process. Both agents save their sessions as they go.

## Supervision

| Backplane mode | Pi (Backplane's extension) | omp (`--approval-mode`) |
|---|---|---|
| Supervised | Asks before every tool except read, grep, find, ls and Backplane's own | `always-ask`: asks before writes, edits and commands |
| Accept edits | Edit and write run; everything else asks | `write`: writes, edits and MCP tools run; commands ask |
| Auto, Full access | Nothing asks | `yolo`: nothing asks |

Pi has no approval prompts of its own, so Backplane's extension gates every tool call, including MCP tools and tools that codemode calls. omp asks through its own dialogs: a `select` titled "Allow tool: <name>" with Approve, Approve <name> for this session and Deny; the choice the person makes goes back. Both kinds go to the person as Backplane approvals.

"Allow for session" works as it does for Grok, by tool name.

## Questions

These go to the person as Backplane questions:

- A dialog from another Pi extension (select, confirm, input or editor).
- omp's `ask` questions and other dialogs (select, confirm, input, editor), one at a time.

## Not mapped

- **Reasoning text:** shown, folded, while Settings → Agents → Show thinking is on (each thought's end carries it whole).
- **Token usage and cost:** omp's are counted (its `get_session_stats` after each turn: tokens, cost and the context it holds, so the token stats chip and the Usage page include it); Pi's are not yet.
- **Steering:** a message sent mid-turn stops the agent and starts its turn again with it, as for the other agents run a process per turn.
- **Images:** attached images reach either agent as file paths in the text, not as image blocks.
- **omp subagents:** omp's subagents run inside its turn and show only as their task tool call.
