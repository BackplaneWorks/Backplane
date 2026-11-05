# Side-by-side: GPUI + the Rust hub on a copy of the real data, doing real work

Design, revision 2 (switchover lane, 2026-11-05), after the reviewer's NOT OK. Nothing runs on real data until the reviewer OKs this
document, agents', files', bots' and state's fixes have landed and been verified, a hub built from them opens a real-size log, and the
coordinator confirms. `BACKPLANE_COPY=1` stays as it is (tests, rehearsals, stand-ins only).

## Blockers before any design is OK

1. **The hub must open a real-size log.** On the real copy (1,691,619 events, 1.48 GB) the hub exited after 28 s with no error line while
   its policy helper was at 1.5 GB and climbing; cause unknown. A synthetic log of the same event count (1.14 GB) is being run to find it
   (results: `build/gpui/results/switchover/`, logs kept). Until the hub opens and the helper fits 3700M, nothing below matters.
2. Every row marked **pending** or **unverified** in the actor table below is held, by code, and verified by a test.

## What this is, honestly

A private copy of the log, opened in the Rust hub and GPUI, beside today's Backplane (never stopped or written; its log is read once, to
make the copy, and its log's tail is read again at each turn start, see 5). **Real agents run on a turn the person starts, with the person's
logins, in the person's real project folders. Their file edits are real.** Going back (`rollback`) deletes the copy (threads, messages,
settings made here); it does **not** undo anything an agent did to a file, and with design history off Backplane keeps no way back for
those edits: only the person's own git. Nothing else acts by itself.

## The one command, the one line back

- Start: `scripts/gpui/side-by-side.sh`: refuses without a terminal, prints the text below, needs a typed `yes`.
- Back: `scripts/gpui/side-by-side.sh rollback`: stops and deletes the copy; today's app never stopped.
- Text shown at start and at rollback (the same words in the client's strip, Known missing and here):
  "This is a copy of your data. Agents you start here run in your real project folders: their file changes are real, and going back does not
  undo them (this copy keeps no history of them; use git). Threads and settings made here are lost when you go back. Nothing else runs by itself."

## The mode, and the new process kind

`BACKPLANE_SIDE_BY_SIDE=1` is honoured only with `BACKPLANE_COPY=1` (alone: refused at start). `copy::on()` stays true, so every copy gate holds.
The mode adds **one new process kind, not a flag consulted by existing ones**:

- `proc::Kind::PersonTurn`. The only place a real agent CLI is made. Constructed only on the person's `turn.start` path in the agents actor, after the
  checks below, and only when `copy::person_turn_allowed()` (= `on()` && the flag) is true.
- `Kind::Agent` stays **stand-in-only** on a copy, in this mode too. Everything else that runs an agent CLI (`one_shot` summaries, accounts'
  `claude auth` and usage refresh, model and CLI probes, `ResumeTicker`, ride-started turns, queues, task and launch children, limit fallback,
  bots' turns) stays `Kind::Agent`, so default-deny holds by construction, not by each caller remembering. Test (child process, like the copy
  ones): on this hub `one_shot`, the accounts refresh and a probe spawn nothing real; `Kind::PersonTurn` is reachable from the `turn.start`
  handler only (a source scan fails the build on any other `Kind::PersonTurn`).
- Tickers: on a copy `register_ticker` accepts only a named list (today: the state policy ticker once verified); any other ticker fails a test.
  Observers and handlers that deliver or commit are in the table and need a named gate too.

## Checks at a person's `turn.start` (all must hold; a failure is a refusal row with its reason, nothing starts)

1. **Trigger**: the client's `turn.start` RPC (`who.client > 0`), role user, no origin, not retry-generated. A person's turn that is running
   keeps working; messages it receives while running are added to its ride only.
2. **Folder**: the thread's folder (`World::cwd`, the log's `worktree`) must exist, resolve (realpath, symlinks followed) under a project
   root named by the copy or under the copy's own home, must not be `/`, and must not be under the live home (`~/.backplane-bend`: scratch
   and bot threads live there) or the copy's `--home` control files. Test: a thread with a `worktree` under the live home, `/`, a missing
   folder and a symlink into the live home are each refused.
3. **Sessions**: a copied session id is never resumed (`--resume <session>` in `agents/rules.rs`, Codex `exec resume`): with the real `HOME` that is the
   live thread's own session file, and both hubs would append to one conversation. A person's turn here **starts fresh** (new session id).
   Lost: the agent does not know the thread's earlier conversation (the messages stay on screen; it never saw them). Offering the last messages as
   quoted text in the first prompt is a later, explicit choice.
4. **Permissions**: the runtime is forced to `approval-required` for a turn here (never `bypassPermissions` or
   `--dangerously-bypass-approvals-and-sandbox`, whatever the copied thread says: an empty or `full-access` runtime is mapped down), with approvals through GPUI's
   asks; the person may raise it per thread by hand. Approvals are therefore on the first-run path and are tested first.
5. **Same folder as the live hub**: at `turn.start` the hub reads the **live** log's tail read-only, from the copy point (the script passes
   the path as `BACKPLANE_LIVE_LOG`; in this mode only, and only because the person chose it), and refuses a folder in which a live turn is
   running (the same folder or one under/over it). Not protected, said plainly: a live turn that starts after the check; and the live hub's
   own design history, which will record the GPUI agent's edits as its thread's steps if it captures there later.
6. **Environment**: the hub itself runs with the **fake HOME** (a folder in the copy's state dir), no `DBUS_SESSION_BUS_ADDRESS`, no
   `XDG_RUNTIME_DIR`; hub-side `git`, `gh` and `kicad-cli` never see the real config or keyring. Only the `PersonTurn` child gets the real
   `HOME` (passed to the hub as `BACKPLANE_PERSON_HOME`, set by the script) and the merged `PATH`; **all `BACKPLANE_*` are stripped from its
   environment** (so no token, port or home of this hub leaks into it); cwd is the checked folder.

## What no gate covers (the person is told this at start)

A real agent has a shell and the real `HOME`. Nothing in the hub stops it from `git push`, `gh`, reading `~/.backplane-bend/token`, reaching
port 3787, or doing anything the person's logins allow. User-level hooks and MCP servers of the person's agent setup also load unless
the agents lane proves the turn runs with `--strict-mcp-config` and project-only settings (not yet checked: until it is, assume they load).
"Every copy gate still holds" is true of the **hub** only; the agent's own process is the person's agent.

## Every automatic actor, how it is held off, and its state today

Status: **held** = code and test exist; **pending** = a named owner is landing the gate; **unverified** = nobody has shown it. No pending or
unverified row may exist at the first real run.

| Actor | Where | Held by | Status | A person sees |
|---|---|---|---|---|
| Resume of running turns (restart carry) | `agents/mod.rs` recovery | agents' copy fix b9f1c1673e; `restart.continue=off` appended to the copy | pending (in review) | those threads read "interrupted" |
| Any agent start other than the person's `turn.start` (ride/wake_ride, start_queued/start_one/steer, task/launch children, work/lock results, limit fallback, `Launch.tell`, mail, bots' turns) | `agents/` | `Kind::Agent` stand-in-only; only `Kind::PersonTurn` is real | pending (agents) | a queued message waits: "starts only when you send" |
| `one_shot` summaries, accounts' `claude auth` and usage refresh, models and CLI refresh/probes | `agents/`, `settings/` | `Kind::Agent` stand-in-only | pending (test above) | summaries and usage blank on a copy |
| `ResumeTicker` | `settings/` | named-ticker list; no delivery on a copy | unverified | none |
| Routine and reminder ticker, `uber_init` | `bots/routines.rs` | `if copy::on() { return }` in the loop and `tick_at`; child-process test both modes (landed, gpui ff07f5c4d4) | held | routines "paused on a copy" |
| Mail recovery | `bots/` `Bots::start` | same gate (landed) | held | none |
| `bots.routine_run` (a person's request) | `bots/` | stays on, but the turn it triggers is not a `turn.start` from a client, so it is `Kind::Agent` (stand-in only), never `PersonTurn` | pending (agents' rule + test) | the routine runs against a stand-in; says so |
| Owner outbox, `owner_kick` | `bots/mail/owner.rs` | returns on a copy; test proves it | held | none |
| State policies ticker (autosettle, alerts, told, summaries, liveness, star nudge) | `state/host.rs` | its commits stay in the copy's log; every out that reaches outside (push, webhook, summary model, desk alerts) dropped | unverified (state lane must show it) | threads may settle; no alerts |
| `work_run`/`work_watch` (agents' new tools), named locks | `agents/`, watch gate | `watch_argv` empty on a copy; the tools refused | pending (agents) | refused with the copy's line |
| Push, webhook, telemetry, update check | `settings/` | `Kind::Net` refused; webhook gated | held | off in Settings |
| Far sync, peers, feeds, tailnet listener, pairing | `net/` | `Kind::Net` refused, listener never started | held | no other machines |
| Deploys, restarts, updates | none in the Rust hub | no code; `deploy.*` refused | held | "not available on a copy" |
| Terminals, browser helper, `preview_*`, teach, tasks | `term/` | `Kind::Terminal`/`Browser` not started | held | tabs say "off on a copy" |
| `secret_run` shell, vault | `settings/` | `Kind::Shell` not started; no session bus, fake HOME | held | empty secrets box |
| Git writes, `file.open`, release | `files/`, `commands/` | `copy::rpc_refused` + `files::git::proc` | held | one-line refusal |
| Design history (`step`, `checkpoint`) | `agents/hist.rs` | fb25d984d1 landed: no capture on a copy | held (reviewer to confirm in the final binary) | history bar shows nothing new |
| Hub-side file writes that are not git (intent, checklist, renders, exports, `kicad_ipc`) | `files/`, `kicad/` | files' audit: each writes only inside the copy's home or is refused | pending (files) | refused or written in the copy |
| KiCad watch, KiCad IPC reads | `kicad/` | `Kind::Local`, read-only on the project | held | works |
| Heartbeats, a running turn's 1 s beat | `wire/`, `agents/` | connection- or person-turn-bound | held | n/a |

## What the person sees (client lane)

- A strip under the title with the text above ("a copy of your data; agents you start run in your real folders; their file changes are real and
  not undone by going back; nothing else runs by itself"), from `sideBySide` in `/hello` and the view info.
- A refused action shows the hub's one-line reason in the usual error row, never a silent no-op.
- An approval for each tool an agent wants, in GPUI's asks.

## Known missing

1. No sync either way: the live Backplane's later work is not here; this copy's threads never go back.
2. No routines, reminders, bots, mail, uberbot; no notifications or push.
3. No terminals, browser, `preview_*`, taught tasks, secrets box.
4. **Agents are the person's real agents**: their file edits are real, rollback does not undo them, there is no history of them in this copy, and no gate
   covers what an agent does with its shell and real `HOME` (see above).
5. No git commit/push/PR from the app, no file-open or release; no design history capture.
6. A copied thread starts a fresh agent session: the agent does not know the thread's earlier conversation.
7. No other machines, pairing, tailnet.
8. A live turn that starts in a folder after the check is not detected; don't run the same folder in both apps.

## Owners

core: `Kind::PersonTurn`, `copy::person_turn_allowed()`, the flag refusal without `BACKPLANE_COPY=1`, the named-ticker list and its test,
`sideBySide` in `/hello`. agents: the `turn.start` checks 1-6, no real start from any other path, forced approvals, fresh session, strict MCP/settings,
`work_*` refusals. bots: the three gates. state: verify and test the policy outs. files: git, non-git writes, history. settings: `ResumeTicker`,
accounts refresh. switchover: the scripts (fake HOME for the hub, `BACKPLANE_PERSON_HOME`, `BACKPLANE_LIVE_LOG`, the copy cut, idle-growth check,
logs kept), the diagnosis of the 28 s exit. reviewer: OK this document and each range.

First-run checklist (the person's command does each, a hard stop): live hub answers `/hello`; port free; 20 GB free beyond the copy; the hub's
`/proc/<pid>/environ` has `BACKPLANE_COPY=1`, `BACKPLANE_SIDE_BY_SIDE=1`, the fake `HOME`, no real `HOME`, no DBUS/XDG_RUNTIME_DIR, no
`BACKPLANE_AGENTS`; the copy's last setting is `restart.continue=off`; after 30 s idle the copy's log has grown by at most 50 events (a count).
