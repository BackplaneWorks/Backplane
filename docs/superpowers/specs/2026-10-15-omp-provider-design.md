# oh-my-pi as an agent provider

Status: approved 2026-10-15, except the approvals section. A real turn showed that Supervised cannot create files over ACP as Backplane speaks it today. That needs a decision (see Approvals).

## Intent and why

Backplane runs one agent per thread, chosen by provider: Claude, Codex, Grok, Muse Code and Google Antigravity. This adds oh-my-pi (`omp`, <https://omp.sh>, installed as the `omp` command) as a sixth provider, so a thread, a bot or a sub-thread can run on it from every client.

Why (my guess, not stated by the person): they already run omp on the hub's machine and want Backplane's threads, bots and viewers to use it without a second setup.

## Goals

- A thread and a bot can use the omp provider. Model, thinking level and approvals are chosen as for the other providers.
- The window, the web client and both phone apps offer it, driven by the same Bend code.
- Backplane finds `omp` on the hub's PATH and tells the person how to install it when it is missing.
- Tests and an end-to-end check run against a stand-in omp. One recorded real turn provides fixtures.

## Non-goals (v1)

- Plan mode for omp. omp auto-approves its plan when the client has no form elicitation (`acp-agent.ts`, `#requestAcpPlanApprovalChoice`), so the person would never see the plan. Revisit once Backplane answers elicitation.
- omp writing thread titles, branch names and commit messages (the Writing provider). Decided: not in v1, as Antigravity.
- A managed install of omp (Antigravity-style pinned download). Hint only.
- A Backplane sign-in flow for omp. Decided: omp uses its own login.
- Passing keys from Backplane's saved API keys file to omp. Decided: no. omp reads its own login and its environment only.
- omp's RPC mode (`omp --mode rpc`).
- Showing omp's own `task` subagents in Backplane's Subagents panel.

## Anti-goals

- No change to how Claude, Codex, Grok, Muse or Antigravity behave.
- Approvals are never weakened. A Supervised thread on omp asks before every write and every shell command, as Supervised does for Grok.
- No key value reaches a client, a log or an agent prompt.
- Persisted events keep their meaning. The provider string is additive.

## Constraints

- Reuse the ACP path that Grok uses (`src/core/acp.bend`, the `Proto` type in `src/core/msp.bend`, `Grok.run` and `Grok.go` in `src/server/server.bend`). Do not fork the protocol.
- Observed on omp 18.8.8. omp releases often, so each fixture records its version.
- Bend 2.0.36 rules from AGENTS.md. `scripts/check.sh` passes before a commit. LAWS.bend may gain laws, never lose or weaken one.
- Tests use a temp `BACKPLANE_HOME`. `omp acp` wrote session files even with `--no-session`, so tests also point `HOME` or `PI_CODING_AGENT_DIR` at a temp folder.
- Provider detection searches `~/.local/bin`, where omp is on this machine. Tests need an `omp` shim (exit 1, as the others have), or an isolated `HOME`.
- House rules: all four clients, tests, PR, merge, update the dev servers, and a TestFlight or APK build because the phone bundle changes.

## The how (decided)

### Transport (my call)

ACP over stdio: `omp acp`, one process per turn, as Grok runs. The sequence:

1. `initialize`. omp answers protocolVersion 1, `authMethods: [agent]`, `loadSession`, and HTTP and SSE MCP.
2. `session/new`, or `session/load` to resume the thread's session id. Backplane's MCP URL goes in `mcpServers`, as for Grok. If `session/load` fails, the thread opens a fresh session and says so, as Antigravity's resume does.
3. `session/set_config_option` for `model` and `thinking`, only when the person chose them.
4. `session/prompt`. `session/update` and `session/request_permission` are handled by `acp.bend` as they are for Grok. The prompt response ends the turn.

Observed: `session/new` returns `configOptions` with `mode` (default, plan), `model` (`provider/model` ids, 35 here) and `thinking` (off, auto, low, medium, high, xhigh, max). `set_config_option` accepted all three with their current values. Update types emitted: `agent_message_chunk`, `tool_call`, `tool_call_update`, `plan`, which `acp.bend` parses already.

Observed in the recorded turn: `session/load` resumes the session. The history replays as updates, and the agent still knew an earlier tool call had been refused.

### Provider and names (my call)

Provider id `omp`. The copy says "oh-my-pi". The command is `omp`.

### Models

The list comes from `omp models --json` (its `selector` values, grouped by provider), read in the same detection cycle as the others. Blank means omp's own default, and nothing is sent. A chosen model goes over `session/set_config_option` as `model`.

### Thinking (decided from the catalog)

omp's catalog gives each model its own `thinking` list in `omp models --json`, and 29 of the 35 chat models have `reasoning` true. Checked against omp 18.11.1's ACP for all 35 models (0 mismatches): a reasoning model's options are `off`, `auto` and its catalog list, and nothing else. A level the model lacks is silently clamped (`max` on `anthropic/claude-haiku-4-5` ran as `xhigh`), and an unknown name is an error. So the chips are per model: `off`, `auto` and the model's own catalog levels, in the order `off, auto, minimal, low, medium, high, xhigh, max`. A non-reasoning model has no chips. The blank model (omp's own default) has no chips either, since the catalog does not say which model that is. Blank means nothing is sent.

### Approvals (decided: option a)

Backplane's runtime mode maps to omp's `--approval-mode` on the process. Observed: the flag takes effect. A `write` under `always-ask` was refused, which it could not be under omp's default `yolo`.

omp sends approval requests two ways. This comes from omp 18.8.8's source:

- `bash`, `edit`, `delete` and `move` go to ACP `session/request_permission` (`session/acp-permission-gate.ts`). Backplane answers them through the asks card, as for Grok. [INFERENCE: not yet seen in a live turn.]
- Every other tool that needs approval, `write` included, asks through `ui.select(["Approve", "Deny"])` (`extensions/wrapper.ts`). Over ACP that is form elicitation (`elicitation/create`), and omp only uses it when the client advertises `clientCapabilities.elicitation.form`. Backplane does not, so the call is denied at once.

Observed live: `write` under `always-ask` ended with `Tool call denied by user: write`. omp sent no request to the client, so the refusal came from omp itself.

Consequence today: Supervised cannot create files, and AcceptEdits cannot run `eval`, `browser` or `task`. These are exec-tier tools outside the gated list. [INFERENCE from the source; `write` is the only one seen live.]

Decided by the person: option (a). Option (b) was not chosen.

- **(a) Build the approval forms (recommended).** Advertise `elicitation.form`. Answer `elicitation/create` for the Approve/Deny select through the asks card: `Allow` sends Approve, `Deny` declines. Decline every other form in v1. OMP's form and `session/request_permission` approval cards offer only Allow and Deny. Older clients may send `session`; the hub normalizes it to one-time `accept` before storing, never the session-wide permission marker used by Grok/Muse. This adds one request type to `acp.bend`, its tests, and a second recorded turn that shows a form.
- **(b) Full access only in v1.** omp is offered in full access, and the other modes come later. Needs a per-provider filter on the mode picker, which is shared across providers today (`codec.bend`). I have not traced the four clients' pickers.

Rejected: mapping Supervised to AcceptEdits. That lets writes run without asking, which the anti-goals rule out.

Target mapping for (a):

| Backplane mode | omp `--approval-mode` | omp asks about |
|---|---|---|
| Supervised | `always-ask` | writes and shell |
| AcceptEdits | `write` | shell |
| AutoMode | `write` (omp has no judge, so this equals AcceptEdits) | shell |
| FullAccess | `yolo` | nothing |

### Auth (decided)

Backplane builds no sign-in flow for omp. Settings shows whether omp is found, not whether it is signed in, as for the other CLIs. The person runs `omp login` on the hub's machine.

### Detection and install (my call)

`command -v omp` runs after clipath's PATH merge, as for the other CLIs, so `~/.local/bin` counts. It joins the probes in `Models.detect` and `Models.present`. The install hint: "Install oh-my-pi with `curl -fsSL https://omp.sh/install | sh` or `bun install -g @oh-my-pi/pi-coding-agent`, then sign in with `omp login`." Both commands come from omp's README and CLI help.

### Protocol handling

- Permission replies use `Acp.choose` as it stands. omp's permission options are `allow_once`, `allow_always`, `reject_once` and `reject_always` (`acp-permission-gate.ts`). If the kinds differ, the request is cancelled, which fails safe.
- `Acp.hint` names the provider it belongs to. It says "grok login" today.
- `omp` joins `Hub.acp_like`, so steering restarts the process as it does for Grok.
- Under (a): `elicitation/create` gets its own branch in `Acp.request.k`. Today unknown methods get a "not supported" reply.

### Clients

Chips in all four clients. The window draws them from `app/layout.bend`, `app/bots.bend` and `client.bend` (`Chip.providers`). The web client draws them from `web/view.bend` and `web/bots.bend`. The phones draw them from `mobile/screen.bend`. No native code names a provider, so the phones need only the new bundle and a TestFlight or APK build.

## Touchpoints (the inventory to start from)

| Area | Where | Change |
|---|---|---|
| Runner | new `src/core/omp.bend` (pure); `server.bend` `Omp.go` beside `Grok.go` | spawn `omp acp`; config steps; approval flag; asks as Grok |
| Proto and ACP | `msp.bend` (`Proto`: new `POmp` case in cmd, name, install, gone, line, init); `acp.bend` (`Acp.hint`, and under (a) the elicitation branch) | per-provider hint and install text |
| Hub | `hub.bend` (`Hub.run.k`, `Hub.acp_like`) | add `omp` |
| Models and detection | `server.bend` (`Models.detect`, `Models.present`, `Models.found`, `About`) | `command -v omp`; `omp models --json` |
| Effort | `agent.bend` (`Effort.levels`, `Effort.args`) | omp levels; config option, not argv |
| Clients | `client.bend` (`Chip.providers`), `app/layout.bend`, `app/bots.bend`, `web/view.bend`, `web/bots.bend`, `mobile/screen.bend` | chips |
| Copy and tool text | `mcp.bend`, `bothub.bend`, `onboard.bend` | provider names |
| Tests | `mbots_test.bend`, `agent_test.bend`, `composer_test.bend`; new `omp_test.bend`, `test/tools/omp_e2e.ts` | updated assertions; fixtures; stand-in `omp` |
| Shims | the e2e scripts that fake `claude`, `codex`, `grok` and `muse` | add an `omp` shim or isolate `HOME` |
| Docs | `AGENTS.md`, `docs/parity.md` | provider list |

## Verification plan

- The recorded turn is at `/tmp/omp-fixtures/transcript.jsonl`, outside the repo. Observed on omp 18.8.8: `initialize`, `session/new`, `session/prompt` with a refused `write`, `session/load`, and a resume prompt that ended with `end_turn`.
- `src/core/omp.bend` gets laws and `test/omp_test.bend`: argv, approval mapping, config-step order, model parsing, install hint, hint text. Fixtures come from that transcript. Under (a), the form turn needs one more recording, which costs a few tokens on the person's omp login, so ask first.
- `test/tools/omp_e2e.ts` runs a stand-in `omp` (as `agy_standin.ts` does). It checks: a thread is created; a turn streams; a tool call is logged; Supervised shows an approval card; FullAccess asks nothing; the model and thinking levels reach the wire.
- The window and the web client show the chip. Phones get the same list through the bridge tests.
- Still unverified until a live turn: bash's `session/request_permission`; the permission option kinds (source says standard ACP); the elicitation method and reply shape (`elicitation/create` is confirmed in omp's utils ACP code); delete, move and rename under Supervised; Backplane's MCP tools appearing in a turn; the update fields for omp's own tools; whether any model sets `reasoning`.

## What's left to the builder

- The Bend code, names, and law statements, following the existing Grok and Antigravity patterns.
- Which call sites reuse existing functions and which get their own.
- The fixture recording script and the e2e harness.

## Calls I made (check these)

1. ACP through `omp acp`, not RPC.
2. Provider id `omp`, copy "oh-my-pi".
3. Detection and a hint only: no managed install, no sign-in UI.
4. Models from `omp models --json`, blank means omp's default.
5. Thinking levels, shown only for models whose catalog entry has `reasoning` true.
6. AutoMode maps to `write`, so it equals AcceptEdits.
7. No plan mode and no Writing provider in v1.
8. `omp` is added beside Grok's runner. Grok's code is not refactored in the same change.
9. Fixtures record their omp version.
10. Resolved: the approval flag takes effect (observed). If it ever stops, omp is not shipped, because omp's default mode is `yolo`.
11. A lost session on resume opens fresh and says so, as Antigravity does.
12. Pending your decision: the approvals path (a) or (b) above.
