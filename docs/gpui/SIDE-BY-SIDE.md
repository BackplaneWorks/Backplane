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

Status: **held** = a landed commit and a test that fails if the hold is removed (named in the last column); **pending** = a named owner has not landed it
on `gpui` yet; **unverified** = no commit and test found; **not held** = the actor runs on a copy today. Checked against `gpui` at `2942e8fdb5` (2026-11-05).
No pending, unverified or not-held row may exist at the first real run. Rows that stay open are listed under the table.

| Actor | Where | Held by | Status | Commit and test | A person sees |
|---|---|---|---|---|---|
| Resume of running turns (restart carry) | `agents/mod.rs` recovery | copy fix; `restart.continue=off` appended to the copy by the launcher | held | `fb25d984d1`: `a_hub_on_a_copy_writes_no_design_history_and_resumes_no_turn` (child process) | those threads read "interrupted" |
| A real agent start for anything but a person's `turn.start` (ride, queue, steer, task/launch children, limit fallback, `Launch.tell`, mail, bots' turns) | `agents/` | `Kind::Agent` stand-in-only; the person's turn is a separate `Kind::PersonTurn` (`proc.rs`), reachable only from the `turn.start` path (`agents/person.rs`: the checks, the door, forced `approval-required`, `--strict-mcp-config`, a fresh session, the person's environment) | held | `2942e8fdb5`: `agents::tests::side_by_side_runs_only_the_persons_turn_with_the_persons_agent_and_guards` (copied ride dropped, bot delivery starts nothing, live-folder conflict refuses, argv and environment guards) | a queued message waits: "starts only when you send" |
| `one_shot` summaries, accounts' `claude auth`/usage refresh, model and CLI probes | `agents/textgen.rs:59`, `settings/` | `Kind::Agent` stand-in-only (`Cli::resolve` refuses a foreign agent on a copy) | pending | no test of `one_shot` itself on a copy is on `gpui` (the proc test above covers the resolver); agents' test is in the unlanded range | summaries and usage blank on a copy |
| `ResumeTicker` (provider-resume records) | `settings/driver.rs:227`, registered `settings/mod.rs:84` | nothing: no `copy::` check in `driver.rs` | **not held** | it delivers a resume message (a turn start) for any `provider.resume.*` record the copied log holds; under `Kind::Agent` that is a stand-in today, but it commits events on its own. Needs a one-line gate and a test (settings lane) | none |
| Routine and reminder ticker, `uber_init`, mail recovery | `bots/` | `copy::on()` return in the loop and `tick_at` | held | `ff07f5c4d4`: `unprompted_work_runs_live_and_not_on_a_copy_hub` (child process, both modes) | routines "paused on a copy" |
| `bots.routine_run` (a person's request) | `bots/` | its turn is not a client `turn.start`: `Kind::Agent`, never `PersonTurn` | pending | rides on the agents' rule above | the routine runs against a stand-in |
| Owner outbox, `owner_kick` | `bots/mail/owner.rs` | returns on a copy | held | `c1c14a1472`: `a_hub_on_a_copy_walks_no_owner_outbox` | none |
| State policies ticker (autosettle, alerts, told, liveness, star nudge, summaries) | `state/host.rs` | passive mode: no tick, boot or summary action on its own; the launcher exports `BACKPLANE_PASSIVE=1` and refuses to start unless the hub's environment has it | held (by the variable) | `89c618243f`: `passive_mode_ticks_nothing`; `86cd8a705b`: `passive_mode_ticks_nothing_and_asks_for_no_summary`, `a_passive_start_commits_nothing_on_a_legacy_home`; `d6147c6451`: no summary model call on a copy. `main.rs` still calls `statehost::start` (bend's line deriving it from the side-by-side flag is not in), so the hold rests on the variable the launcher sets | threads may settle; no alerts |
| Legacy watched work and named locks (`watch_argv`, adoption) | watch gate | every watch reads as gone on a copy | held | `636f1df382` and `copy::tests::watch_gate_none_on_a_copy_and_the_list_kept_off`, `copy_e2e` | none |
| New `work_run`/`work_watch` tools (agents) | `agents/` | off and not re-armed on a copy; the tools are refused | held | `2942e8fdb5`: `agents::tests::a_hub_on_a_copy_rearms_no_work_and_the_tools_are_refused` | refused with the copy's line |
| `wake_on`/`wake_cancel` | `agents/` | no tail, no fire, both refused on a copy | held | `865d7c7c76` (child-process test) | refused with the copy's line |
| Alert-driven outs: webhook, push, island, desk alerts | `net/`, `settings/webhook.rs` | no out leaves on a copy | held | `0de535dc3c`: `no_alert_driven_out_leaves_on_a_copy_and_the_same_probe_posts_without_the_flag`; `7dbce7696c`: `every_child_process_is_made_through_proc` | alerts off in Settings |
| Telemetry, update check | `hub.rs` | `Kind::Net` refused | unverified | no commit or test names them | off on a copy |
| Far sync, peers, feeds, tailnet listener, pairing | `net/` | `Kind::Net` refused, listener never started | held | `proc::tests::a_copy_run_does_nothing_outside_its_home`, `every_outbound_socket_is_on_the_allowlist` (`0950d1591e`) | no other machines |
| Deploys, restarts, updates | none in the Rust hub | no handler | unverified | absence only: on `d66a6b1c67` the only `deploy.*` is a state fold of a setting (`state/fam.rs`); no test asserts a copy refuses a `deploy.*` request | "not available on a copy" |
| Terminals, browser helper, `preview_*`, teach, tasks | `term/` | `Kind::Terminal`/`Browser` not started | held | `term/tests.rs`: `a_copy_hub_runs_no_chat_terminal_but_logs_the_entry`, `on_a_copy_the_terminal_and_the_browser_answer_with_the_copy_refusal` | tabs say "off on a copy" |
| `secret_run` shell, vault, keyring | `settings/` | `Kind::Shell` not started; the keyring is never read; the launcher gives the hub no session bus | held | `e57000d05d`: `a_copy_never_touches_the_real_keyring`; the proc test above | empty secrets box |
| Git writes, `file.open`, release | `files/`, `commands/` | default-deny: only listed reads run; `copy::rpc_refused` | held | `c0837b2b31`: `files::git::a_copy_refuses_every_command_not_listed_as_a_read` | one-line refusal |
| Design history (`step`, `checkpoint0`, `checkpoint`) | `agents/hist.rs` | no capture on a copy | held | `fb25d984d1` (same test as the first row) | history bar shows nothing new |
| Hub-side file writes that are not git: `mech.render` into the project, `review.tick` writing `.backplane/checklist.md`, intent, exports, `kicad_ipc` writes | `files/`, `kicad/` | the call-site audit table in `COPY-GUARD.md`: told or refused on a copy; `kicad.export10` writes only `<home>/exports` | partly: held for `kicad_ipc`, pending for the rest | `c0837b2b31` (audit); `kicad::ipc::a_copy_runs_only_the_listed_kicad_reads` covers the IPC writes. No test names `mech.render`, `review.tick` or the intent write | refused, or written in the copy |
| KiCad watch, KiCad IPC reads, git reads | `kicad/`, `files/` | reads run with `GIT_OPTIONAL_LOCKS=0` (a read never rewrites an index) | held | `2dc771d3ec` (test in `files/git.rs` asserting the variable) | works |
| Heartbeats, a running turn's 1 s beat | `wire/`, `agents/` | connection- or person-turn-bound, not actors | n/a | n/a | n/a |

Open rows at `2942e8fdb5`, and who closes them: **not held** `ResumeTicker` (settings: a one-line `copy::on()` gate and a child-process test; the coordinator's
hold condition 5 (`51abfb678d`) already covers provider resume, and `settings/driver.rs` still has no `copy::` check); **pending** `one_shot`/accounts/probes (agents:
a test that they spawn nothing real on a copy) and `bots.routine_run` (agents/bots: the side-by-side test above shows bot delivery starts nothing, but no test names
`routine_run`); **partly pending** the non-git writes (files: a test each for `mech.render`, `review.tick`, intent); **unverified** telemetry and update check, deploys
and restarts (net/settings: a test that a copy refuses them). The state row depends on `BACKPLANE_PASSIVE=1`, which only the launcher sets until bend's `main.rs` line lands.

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
