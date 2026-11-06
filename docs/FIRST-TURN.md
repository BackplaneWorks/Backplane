# The first real Claude and Codex turn through the Rust hub

Written for the person who says "go". Nothing here has been run with a real agent: every agents test uses a stand-in (`BACKPLANE_AGENTS=<dir>`). Do not run one until this list is read.

## What the hub does for a Claude turn (agents/mod.rs `user` → `spawn`)

1. A `turn.start {thread, text}` (or any `deliver`) posts the message, sets the turn running, and starts `claude` once per thread (the process stays between turns).
2. The binary is found in: the hub's own `PATH`, then the login shell's (`$SHELL -lc`, 5 s), then the usual install directories that exist (nvm newest first, volta, bun, `~/.local/bin`, npm's global bin, cargo, homebrew, `/usr/local/bin`). The agent then runs with that merged `PATH`.
3. Command line (rules.rs `claude_args`, equal to the JS hub's, golden-tested): `-p --verbose --input-format stream-json --output-format stream-json --include-partial-messages --permission-mode <mode> --disallowedTools Agent,Task --append-system-prompt <runtime prompt> [--permission-prompt-tool mcp__backplane__permission_prompt] [--model M] [--resume SESSION] --add-dir <kistack>, <attachments> [--effort E] [--thinking-display summarized] --plugin-dir <kistack> --mcp-config {backplane: <loopback URL>, pcbparts: https://pcbparts.dev/mcp}`.
4. Working directory: the thread's worktree, else the project's root. It must exist (else "its folder … is gone").
5. Environment: the hub's own, plus `BACKPLANE_AGENT_OWNER=<home>` and the merged `PATH`. **No credentials are added or removed.** Claude uses its own sign-in (`~/.claude/`, `HOME` as the hub has it) or `ANTHROPIC_API_KEY` if the hub process has it. The API keys file (`<home>/secrets/api.env`) is not injected: agents are told to load it in the command that needs it (as in the JS hub).
6. The MCP URL is `http://127.0.0.1:<ephemeral port>/mcp/<thread>.<gen>?k=<hmac>`: loopback only, one key per thread and process generation. A stale process may only read.
7. Input: one stream-json user line per message (images read from the attachments, `$skill` mentions expanded). Output lines become events (assistant messages, tool rows, usage, session id, todos, background work).
8. Ends: a `result` line closes the turn via the command ledger (Claude ≥ 2.1.286 sends command lifecycle lines; older CLIs use the legacy rule). Stop interrupts; a dead process fails the turn ("agent exited with code N …" with the scrubbed stderr tail; 137 says out of memory).

## Codex (agents/codex.rs, providers/codex.rs)

One `codex exec --json` process per turn, resumed with `exec resume <session>`; `-s read-only | workspace-write` or `--dangerously-bypass-approvals-and-sandbox` by the thread's runtime; Backplane's MCP URL and pcbparts through `-c mcp_servers.*.url`; the runtime prompt as `developer_instructions`. Credentials: Codex's own (`~/.codex`, or `OPENAI_API_KEY` in the hub's env). Messages sent while it runs wait as the thread's ride and arrive with its next Backplane tool result or start the next turn; Steer stops and restarts it.

## Before the first turn

- Run the hub **without** `BACKPLANE_AGENTS` and **without** `BACKPLANE_COPY=1` (either one makes it refuse real agents: stand-in only / none). The one exception is the side-by-side mode on a copy of the real data (`docs/SIDE-BY-SIDE.md`: `BACKPLANE_COPY=1` + `BACKPLANE_SIDE_BY_SIDE=1`), where only a turn the person starts may run a real agent, under the checks written there.
- Use a throwaway home and project: a new directory with a tiny git repo, not the real store.
- Check from the same account the hub runs as: `claude --version`, `claude -p "say hi"` signed in, `codex --version`, `codex exec "say hi"`. If these fail there, the hub's turn will too.
- Check the hub's PATH: the hub logs nothing about it; if `claude` is not found the turn fails with `claude is not installed (looked in …)`.
- Watch: `<home>/events.jsonl` (events appended), the hub's stderr (refusals, "could not write N events"), and `ps` for the agent's process group (setsid; it dies with the hub via PDEATHSIG; leftovers of a SIGKILLed hub are ended at the next start through `BACKPLANE_AGENT_OWNER`).

## What could go wrong (and how it shows)

| Symptom | Cause | Where it shows |
|---|---|---|
| "claude is not installed" | CLI outside every searched directory | error row `spawn`, turn failed |
| "could not start claude: its folder … is gone" | worktree removed | same |
| Turn fails at once, exit row with stderr | not signed in, expired token, wrong `HOME`, unsupported flag in a new CLI | error row `exit` with the scrubbed stderr tail |
| "agent was killed (signal 9, likely out of memory)" | OOM killer, or a cgroup cap | exit row |
| Turn never ends | the CLI's result line missing; the stuck watchdog ends it after 120 s (`BACKPLANE_STUCK_S`) with a "stuck" row | info row `stuck` |
| Permission prompts hang | `permission_prompt` asks wait for the person; none answered | open ask, `pending` count |
| MCP tools "unknown tool" | a tool the Rust hub does not serve yet (see PARITY.md "MCP tools") | tool error in the agent's transcript |
| Wrong/missing live text | delta merge differs from JS in count only; joined text is equal | cosmetic |
| Nothing listens on the MCP port | the hub was restarted: the agent's URL is stale (a new generation) | tool errors "stale process"; the next turn gets a fresh URL |
| Costs | a real turn spends the person's quota/credits | the usage rows (`TurnUsage`) |

Not covered by any test with a real agent: Claude's actual stream-json (the replays are recordings from the stand-ins and the JS hub), image input, the limit/quota fallback driver, Codex's real JSON, `$skill` expansion against real skill folders.

## What a real turn leaves in the person's `~/.claude`

A Claude turn through the hub runs with the real `HOME` (side-by-side mode passes `BACKPLANE_PERSON_HOME`; a plain hub passes its own), so the CLI keeps its usual state there. Observed after the first real turns on a scratch folder (`/tmp/bp-first-turn/proj`, five turns across two sessions, one a launched subthread):

- `~/.claude/projects/<folder path with / turned into ->/<session id>.jsonl`: one transcript per session, the whole conversation with tool calls and the cached prompt text. The two sessions were 460 KB and 313 KB; 772 KB for the folder with its `memory/` directory (created empty or by the CLI's auto-memory).
- Likely also (not checked one by one): shell snapshots, session env and debug logs under `~/.claude/`, an entry for the folder in `~/.claude.json`, and a line per prompt in `~/.claude/history.jsonl`.

Nothing in `~/.claude` is created or removed by the hub itself, and tests never run with the real `HOME`. The hub deletes nothing there. Cleaning up a scratch session is the person's call (the folder named above is the one the first-turn runs wrote; it was left in place). A side-by-side turn on a real project adds its transcripts to that project's folder under `~/.claude/projects/` the same way, next to the person's own sessions for it; it never resumes them (fresh session) and never edits them.
