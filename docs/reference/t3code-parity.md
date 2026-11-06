# T3Code / Backplane (TS) parity reference

What the TypeScript Backplane (`i2cjak/Backplane`, a `pingdotgg/t3code` fork)
does, distilled so rewrite can match it. Paths refer to that repo.
Parity status lives in `docs/parity.md`.

## Architecture

- Clients send typed WebSocket requests. The server turns each one into a *command*.
- A pure *decider* turns commands into persisted *events*.
- A *projector* folds those events into the read model.
- Side effects run in *reactors*, which report back by dispatching commands.
- An acknowledgement means "the intent is committed", not "the work is done".
- Events, projections and the command receipt commit together; the command id makes commands idempotent.

## Providers

- Codex: `codex app-server`, JSON-RPC over stdio.
  - Start with `thread/start {cwd, approvalPolicy, sandbox, model}`.
  - Then `turn/start {collaborationMode{mode, settings{model, reasoning_effort, developer_instructions}}, sandboxPolicy}`.
- Claude: Agent SDK with these options:
  - `systemPrompt {preset: "claude_code", append: <runtime block>}`
  - `settingSources [user, project, local]`
  - partial messages on, `resume`
  - `additionalDirectories [cwd, attachments, kistack cache]`
- Routing is by `instanceId` (one configured account of a driver). `ModelSelection = {instanceId, model, options}`.

Runtime modes map to providers like this (Codex approval/sandbox → Claude permissionMode):

| Mode | Codex | Claude |
|---|---|---|
| `approval-required` | untrusted / read-only | default |
| `auto-accept-edits` | on-request / workspace-write | acceptEdits |
| `auto` | on-request / workspace-write + auto_review | auto |
| `full-access` (default) | never / danger-full-access | bypassPermissions |

Interaction modes are `default | plan`.
- Plan mode ends in a `<proposed_plan>` block, which becomes a *proposed plan* that can be implemented in a new thread.
- Claude's ExitPlanMode is intercepted and denied with "The client captured your proposed plan. Stop here and wait…".
- AskUserQuestion becomes a user-input request.

## Wire protocol (WebSocket `/ws`)

Orchestration methods:
- `dispatchCommand`
- `subscribeShell`: snapshot, then upserts/removes with a `sequence`
- `subscribeThread {threadId, afterSequence?, turnLimit?}`
- `getTurnDiff`, `getFullThreadDiff`, `searchThreads`, `getArchivedShellSnapshot`

Other method groups:
- `projects.*`: list/add/remove/listEntries/readFile/writeFile/search
- `vcs.*`: status/refs/worktrees/branches
- `git.runStackedAction`: commit | push | create_pr | commit_push | commit_push_pr
- `terminal.*`, `server.*` (config/settings/keybindings/update)

Commands:
- Project: `project.create/meta.update/delete`
- Thread lifecycle:
  - `thread.create/delete/archive/unarchive`
  - `settle/auto-settle/unsettle/snooze/unsnooze/pin/unpin/pin.reorder/active.reorder`
  - `meta.update`, `runtime-mode.set`, `interaction-mode.set`
- Turns: `turn.start {message, attachments, runtimeMode, interactionMode, bootstrap{createThread, prepareWorktree{projectCwd, baseBranch, branch?}}}`, `turn.interrupt`
- Responses: `approval.respond (accept|acceptForSession|acceptAlways|decline|cancel)`, `user-input.respond`
- Other: `checkpoint.revert`, `session.stop`

## Read model

- **Thread**: id, projectId, title, modelSelection, runtimeMode, interactionMode, branch, worktreePath, archivedAt, settledOverride, settledAt, unsettledAt, snoozedUntil, pinnedAt, pinOrderKey, activeOrderKey, latestTurn{state}, messages, proposedPlans, activities, checkpoints, session.
- **Message**: {role user|assistant|system, text, attachments, turnId, streaming}.
- **Activity**: {tone info|tool|approval|error, kind, summary, payload, turnId, sequence}.
- **Session**: {status idle|starting|running|ready|interrupted|stopped|error, activeTurnId, lastError}.
- **Shell** (the sidebar) adds latestUserMessageAt, hasPendingApprovals, hasPendingUserInput, hasActionableProposedPlan.

## Settle rule (ThreadSettlementPolicy)

A server tick (every 1 minute) auto-settles a thread when all of the following hold:
- It is not archived and settledOverride is unset.
- Nothing is pending (approvals, user input).
- It is not starting or running.
- It has no queued turn and is not snoozed (or it woke up).
- **Either** its activity time (the max of the last user message and the latest turn's requested/started/completed times) is more than N days ago (N = `sidebarAutoSettleAfterDays`, default **3**, range 1–90, null = off),
- **or** its PR merged or closed after the latest user anchor.

Settling clears the pin and the active slot. Unsettling stamps `unsettledAt`, which puts the thread back at the top of the active list.

## Sidebar order

- Sections: pinned → active → snoozed → settled.
- Pinned and active: threads with an order key first (fractional base-26 key, set by drag). The rest by max(createdAt, unsettledAt), newest first. New activity does not reorder threads.
- Settled: by settle time, newest first.
- Projects: by updated_at (max over non-archived threads of the latest user message time or createdAt), created_at, or manual. Ties break by title, then id.
- Status priority: approval > input > working > failed > background > monitoring > ready.

## Worktrees and git

- Env mode `local | worktree`. The default is per project, then from `backplane.json`, then global.
- Worktree path: `<base>/worktrees/<repo>/<sanitizedBranch>`.
- The temporary branch is `backplane/<8 hex>`. On the first turn it's renamed from an LLM-proposed fragment (at most 64 characters, sanitized).
- Checkpoints: after each turn the workspace is captured into a hidden ref `refs/backplane/checkpoints/<b64url(threadId)>/turn/<n>`, using a temp index, with no commits on the user branch. Revert restores the files and rolls the provider back.

## System prompt

The runtime block is appended to every provider:

    <runtime_info>In case you're asked: you are running in Backplane through the {harness} harness[, as {model}][ with {effort} reasoning effort]. No need to mention this otherwise. You can embed images and videos in your response using Markdown with absolute file paths.</runtime_info>
    <kicad_runtime>kicad-cli path, export/erc/drc usage, fork IPC notes</kicad_runtime>
    <kistack_skills>…MUST read SKILL.md… board-editing guidance … - name: description Read "<path>"</kistack_skills>

Codex developer instructions (`<collaboration_mode>`):
- **Plan mode**: ground in the environment, then chat about intent, then about implementation. Non-mutating actions only. The plan goes in one `<proposed_plan>` block and must be decision complete.
- **Default mode**: prefer assumptions over questions.

Skills:
- Scanned from `~/.claude/skills/*/SKILL.md` (user scope, wins on a clash) and `<cwd>/.claude/skills/*/SKILL.md` (project scope).
- Frontmatter: name, description, disable-model-invocation, user-invocable.
- KiStack skills are pinned by revision in `~/.cache/backplane/kistack/<sha>/`.
- A `$skill` mention becomes `/name args` for Claude.

## Auto-update

- Desktop: electron-updater against GitHub releases.
  - First check 15 s after start, then every 4 minutes.
  - Channels `latest | nightly` (nightly versions look like `-nightly.YYYYMMDD.N`).
  - Explicit download, then install.
- The CLI never updates itself in the foreground. As a service, a launcher stages the new version, runs a trial boot, and commits or rolls back.

## Viewers (TS Backplane)

- KiCanvas-derived WebGL PCB/schematic viewer, in an iframe.
- Updates by polling a manifest every 2.5 s (revision = sha256 of path+size+mtime) and re-parsing the whole file. This caused the jitter and pop-in, which later code tried to patch around.
- Gerbers go through a Python SVG pipeline.
- 3D: `kicad-cli pcb export glb` rendered with Three.js.
- STEP: occt-import-js WASM.

## Keybindings

| Key | Action |
|---|---|
| mod+b | sidebar |
| mod+j | terminal |
| mod+d | diff |
| mod+k | command palette |
| mod+n / mod+shift+o | new thread |
| mod+shift+n | new local thread |
| mod+p | file picker |
| mod+shift+f | search |
| mod+shift+m | model picker |
| mod+shift+[ / ] | previous / next thread |
| mod+shift+s | settle |
| mod+shift+p | pin |
| mod+1..9 | jump to thread |
| mod+enter | submit in the background |

## House rules carried over

- Never kill processes by name, only PIDs you captured.
- Never write to live userdata.
- Persisted events stay decodable forever.
- Tests wait on receipts, never on sleeps.
- No continuously repainting animations: animate only while something is actually changing.
- Square corners, minimal copy.
- Complexity lives at the adapter boundary; orchestration is pure; the UI is dumb.
