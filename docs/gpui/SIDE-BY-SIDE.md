# Side-by-side: GPUI + the Rust hub on a copy of the real data, doing real work

Design, revision 2 (switchover lane, 2026-11-05), after the reviewer's NOT OK. Nothing runs on real data until the reviewer OKs this
document, agents', files', bots' and state's fixes have landed and been verified, a hub built from them opens a real-size log, and the
coordinator confirms. `BACKPLANE_COPY=1` stays as it is (tests, rehearsals, stand-ins only).

## Blockers before any design is OK

1. **The hub must open a real-size log, and today it cannot.** Measured (synthetic log of the real event count, 1,691,621 events, 1.09 GB; the real
   copy is 1.48 GB; hub = `hub/prov`, helper = `bp-policy-bp`, scope `MemoryMax=3700M`, swap 0): the policy helper's RSS grows linearly, about
   100 MB/s (0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5 GB at 5 s steps) from the first second, while the hub stays at 66 MB; the kernel OOM-killer ended the scope at
   37 s (journal: `oom-kill`, 3.6 GB peak), and the hub exited with no error line in `hub.log`. The helper had not finished folding when it hit the cap, so
   its full need is above 3.7 GB (an earlier uncapped run reached 7.5 GB). The first run on the real copy ended at 28 s with a 1.6 GB scope peak and **no** OOM
   line in the journal, so its end was something else, still unexplained; it is not reproduced by the synthetic log. Two things for core/slim before any
   design passes: the helper's whole-log fold has to fit (page it, trim it, or fold only a window), and the hub must say why when its helper dies
   (today it exits silently). Raw: `build/gpui/results/switchover/` (`diag28.log`, `logs/*/rss.tsv`).
2. **The unexplained 28 s stop on the real log.** The first run on the real copy ended after 28 s with a 1.6 GB scope peak and no out-of-memory line in the
   journal; the synthetic log does not reproduce it (it reproduces an OOM kill at 37 s instead). The real log has something the synthetic one lacks, or the
   hub/helper fails on it: not understood, so a real run cannot be promised to open.
3. **Bub.** Bub (the person's uberbot) and everything it does on its own (routines, reminders, hand-offs, reports back to the person) does not run on this hub: the
   ticker, `uber_init` and mail are held off on a copy. Threads Bub launched keep their history but nothing reports to Bub. That is by design here, and it is a
   difference the person will feel.
4. Every row marked **pending** or **unverified** in the actor table below is held, by code, and verified by a test.

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

`BACKPLANE_SIDE_BY_SIDE=1` is honoured only with `BACKPLANE_COPY=1` (alone: refused at start). `side` mode also exports `BACKPLANE_PASSIVE=1` (state's flag: the policies do nothing on their own); the hub will derive it from `BACKPLANE_SIDE_BY_SIDE` once that lands, and the script's first-run check requires both in the hub's environment. `copy::on()` stays true, so every copy gate holds.
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
   **Delivery is a door too** (`agents/mod.rs`: a live process whose key matches takes the next message on its stdin): after a person's turn a Claude
   process stays up, so check 1 applies at every delivery: in this mode only a person's `turn.start` may write to a `PersonTurn` process or end it at
   turn end; `Launch.tell`, a queued message, a wake or a routine run never write to it (they go to a stand-in or wait). Test: after a person's turn, each
   non-person delivery leaves the process's stdin untouched and starts nothing.
2. **Folder**: the thread's folder (`World::cwd`, the log's `worktree`) must exist, resolve (realpath, symlinks followed) under a project
   root named by the copy or under the copy's own home, must not be `/`, and must not be under the live home (`~/.backplane-bend`: scratch
   and bot threads live there) or the copy's `--home` control files. Test: a thread with a `worktree` under the live home, `/`, a missing
   folder and a symlink into the live home are each refused. The same rule applies to every extra directory the turn is given (`--add-dir`, the
   log's `a.dirs`): each must pass, or the turn is refused.
3. **Sessions**: a copied session id is never resumed (`--resume <session>` in `agents/rules.rs`, Codex `exec resume`): with the real `HOME` that is the
   live thread's own session file, and both hubs would append to one conversation. A person's turn here **starts fresh** (new session id).
   Lost: the agent does not know the thread's earlier conversation (the messages stay on screen; it never saw them). Offering the last messages as
   quoted text in the first prompt is a later, explicit choice.
4. **Permissions**: the runtime is forced to `approval-required` for a turn here (never `bypassPermissions` or
   `--dangerously-bypass-approvals-and-sandbox`, whatever the copied thread says: an empty or `full-access` runtime is mapped down), with approvals through GPUI's
   asks; the person may raise it per thread by hand. Approvals are therefore on the first-run path and are tested first.
5. **Same folder as the live hub**: at `turn.start` the hub reads the **live** log's tail read-only, from the copy point (the script passes
   the path as `BACKPLANE_LIVE_LOG`; in this mode only, and only because the person chose it), and refuses a folder in which a live turn is
   running (the same folder or one under/over it). The check must know the folder of threads created after the copy point, so the tail read folds
   `ThreadCreated` and `WorktreeSet` as well as turns, and still refuses a thread whose turn was running at the copy point. A live log the check cannot read
   (missing, unreadable, torn beyond its last whole line, a fold error) means **refuse**. Not protected, said plainly: a live turn that starts after the check; and the live hub's
   own design history, which will record the GPUI agent's edits as its thread's steps if it captures there later.
6. **Environment**: the hub itself runs with the **fake HOME** (a folder in the copy's state dir), no `DBUS_SESSION_BUS_ADDRESS`, no
   `XDG_RUNTIME_DIR`; hub-side `git`, `gh` and `kicad-cli` never see the real config or keyring. Only the `PersonTurn` child gets the real
   `HOME` (passed to the hub as `BACKPLANE_PERSON_HOME`, set by the script) and the merged `PATH`; **all `BACKPLANE_*` are stripped from its
   environment** (so no token, port or home of this hub leaks into it); cwd is the checked folder.

## What no gate covers (the person is told this at start)

A real agent has a shell and the real `HOME`. Nothing in the hub stops it from `git push`, `gh`, reading `~/.backplane-bend/token`, reaching
port 3787, or doing anything the person's logins allow. **Requirement, not an assumption:** a `PersonTurn` runs with `--strict-mcp-config` (only this hub's own MCP config, passed by argument, so stripping `BACKPLANE_*`
does not break approvals) and with user settings not loaded (`--setting-sources project,local`; the login method goes by `--settings` as today): the person's
**user-level hooks, MCP servers and permission rules do not load**; the **project's own** `.claude` hooks and settings (in the real folder) **do** load, since
they belong to that project. The agents lane proves login still works that way. A test reads the argv.
"Every copy gate still holds" is true of the **hub** only; the agent's own process is the person's agent.

## Every automatic actor, how it is held off, and its state today

Status: **held** = code and test exist; **pending** = a named owner is landing the gate; **unverified** = nobody has shown it. No pending or
unverified row may exist at the first real run.

| Actor | Where | Held by | Status | A person sees |
|---|---|---|---|---|
| Resume of running turns (restart carry) | `agents/mod.rs` recovery | agents' copy fix b9f1c1673e; `restart.continue=off` appended to the copy | held (fb25d984d1 landed) | those threads read "interrupted" |
| Any agent start other than the person's `turn.start` (ride/wake_ride, start_queued/start_one/steer, task/launch children, work/lock results, limit fallback, `Launch.tell`, mail, bots' turns) | `agents/` | `Kind::Agent` stand-in-only; only `Kind::PersonTurn` is real | pending (agents) | a queued message waits: "starts only when you send" |
| `one_shot` summaries, accounts' `claude auth` and usage refresh, models and CLI refresh/probes | `agents/`, `settings/` | `Kind::Agent` stand-in-only | pending (test above) | summaries and usage blank on a copy |
| `ResumeTicker` | `settings/` | named-ticker list; no delivery on a copy | unverified | none |
| Routine and reminder ticker, `uber_init` | `bots/routines.rs` | `if copy::on() { return }` in the loop and `tick_at`; child-process test both modes (landed, gpui ff07f5c4d4) | held | routines "paused on a copy" |
| Mail recovery | `bots/` `Bots::start` | same gate (landed) | held | none |
| `bots.routine_run` (a person's request) | `bots/` | stays on, but the turn it triggers is not a `turn.start` from a client, so it is `Kind::Agent` (stand-in only), never `PersonTurn` | pending (agents' rule + test) | the routine runs against a stand-in; says so |
| Owner outbox, `owner_kick` | `bots/mail/owner.rs` | returns on a copy; test proves it | held | none |
| State policies ticker (autosettle, alerts, told, summaries, liveness, star nudge) | `state/host.rs` | its commits stay in the copy's log; every out that reaches outside (push, webhook, summary model, desk alerts) dropped | unverified (state lane must show it) | threads may settle; no alerts |
| `work_run`/`work_watch` (agents' new tools), named locks | `agents/`, watch gate | `watch_argv` empty on a copy; the tools refused | pending (agents) | refused with the copy's line |
| Outs driven by the `alert.*` records (push, webhook, desk notices, island) | `state/turnend.rs` writes the records; `settings/webhook.rs`; test `net/copy_outs.rs` | The records stay in the copy's own log. The one sender in Rust is `webhook::deliver`: its own `copy::on()` check, and the process gate refuses `Kind::Net` curl; nothing calls it yet. APNs push, island sync and desk notices (`notify` frames) are not ported: no code sends them. | held; child-process test in both modes (flag: a local listener sees nothing; no flag: the same probe posts and it sees the POST) | no alerts |
| Push, webhook, telemetry, update check | `settings/` | `Kind::Net` refused; webhook gated | held | off in Settings |
| Far sync, peers, feeds, tailnet listener, pairing | `net/` | `Kind::Net` refused, listener never started | held | no other machines |
| Deploys, restarts, updates | none in the Rust hub | no code; `deploy.*` refused | held | "not available on a copy" |
| Terminals, browser helper, `preview_*`, teach, tasks | `term/` | `Kind::Terminal`/`Browser` not started | held | tabs say "off on a copy" |
| `secret_run` shell, vault | `settings/` | `Kind::Shell` not started; no session bus, fake HOME | held | empty secrets box |
| Git writes, `file.open`, release | `files/`, `commands/` | `copy::rpc_refused` + `files::git::proc` | held | one-line refusal |
| Design history (`step`, `checkpoint`) | `agents/hist.rs` | fb25d984d1 landed: no capture on a copy | held (reviewer to confirm in the final binary) | history bar shows nothing new |
| Hub-side file writes that are not git (intent, checklist, renders, exports, `kicad_ipc`) | `files/`, `kicad/` | files' audit: each writes only inside the copy's home or is refused | done (files): `mech.render`, `review.tick`, `fab.intent` and the project home intent op are refused on a copy (`copy::HOME_ONLY`); `kicad_ipc` runs only `Get*`/`Ping`; exports write only `<home>/exports`. Commit `d6ff126ce2`; tests `files::boundary_tests::a_copy_writes_no_render_checklist_or_intent_into_a_project` (child with `BACKPLANE_COPY=1`; project byte for byte unchanged), `kicad::ipc::tests::a_copy_runs_only_the_listed_kicad_reads`, `files::git::tests::a_copy_run_writes_nothing_outside_its_home`; audit table in COPY-GUARD.md | refused or written in the copy |
| KiCad watch, KiCad IPC reads | `kicad/` | `Kind::Local`, read-only on the project | held | works |
| Heartbeats, a running turn's 1 s beat | `wire/`, `agents/` | connection- or person-turn-bound | held | n/a |

## What the person sees (client lane)

- A strip under the title with the text above ("a copy of your data; agents you start run in your real folders; their file changes are real and
  not undone by going back; nothing else runs by itself"), from `sideBySide` in `/hello` and the view info.
- A refused action shows the hub's one-line reason in the usual error row, never a silent no-op.
- An approval for each tool an agent wants, in GPUI's asks.

## Known missing

1. No sync either way: the live Backplane's later work is not here; this copy's threads never go back.
2. No routines, reminders, bots, mail, **Bub (the uberbot)**; no notifications or push.
3. No terminals, browser, `preview_*`, taught tasks, secrets box.
4. **Agents are the person's real agents**: their file edits are real, rollback does not undo them, there is no history of them in this copy, and no gate
   covers what an agent does with its shell and real `HOME` (see above).
5. No git commit/push/PR from the app, no file-open or release; no design history capture.
6. A copied thread starts a fresh agent session: the agent does not know the thread's earlier conversation.
7. No other machines, pairing, tailnet.
8. **The hub has not opened the real log yet** (the helper does not fit 3700M; the first real run stopped unexplained at 28 s): everything in this document is for a
   hub that can.
9. A live turn that starts in a folder after the check is not detected; don't run the same folder in both apps.

## Owners

core: `Kind::PersonTurn`, `copy::person_turn_allowed()`, the flag refusal without `BACKPLANE_COPY=1`, the named-ticker list and its test,
`sideBySide` in `/hello`. agents: the `turn.start` checks 1-6 and the delivery rule, `--strict-mcp-config`, project-only settings, no real start from any other path, forced approvals, fresh session, strict MCP/settings,
`work_*` refusals. bots: the three gates. state: verify and test the policy outs. files: git, non-git writes, history. settings: `ResumeTicker`,
accounts refresh. switchover: the scripts (fake HOME for the hub, `BACKPLANE_PERSON_HOME`, `BACKPLANE_LIVE_LOG`, the copy cut, idle-growth check,
logs kept), the diagnosis of the 28 s exit. reviewer: OK this document and each range.

First-run checklist (the person's command does each, a hard stop): live hub answers `/hello`; port free; 20 GB free beyond the copy; the hub's
`/proc/<pid>/environ` has `BACKPLANE_COPY=1`, `BACKPLANE_SIDE_BY_SIDE=1`, the fake `HOME`, no real `HOME`, no DBUS/XDG_RUNTIME_DIR, no
`BACKPLANE_AGENTS`; the copy's last setting is `restart.continue=off`; after 30 s idle the copy's log has grown by at most 50 events (a count).
