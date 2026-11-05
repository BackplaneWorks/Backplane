# Side-by-side: GPUI + the Rust hub on a copy of the real data, doing real work

Design for review (switchover lane, 2026-11-05). Nothing runs on real data until the reviewer OKs this, agents' and files' copy fixes
have landed, and the coordinator confirms. `BACKPLANE_COPY=1` stays as it is (tests, rehearsals, stand-ins only).

## What the person gets

- One command, run by the person in a terminal: `scripts/gpui/side-by-side.sh`. It asks for a typed `yes` (it refuses without a terminal,
  so no lane can run it), copies the live log (read only), starts the Rust hub with the current policy helper and the GPUI window.
- One line back: `scripts/gpui/side-by-side.sh rollback` (stop, delete the copy). Today's Backplane (port 3787, `~/.backplane-bend`) is never
  stopped or written, so there is nothing to restore. Lost by going back: whatever was done in the GPUI app (it lives only in the copy).
  Also true while running: what the live Backplane does after the copy point never shows up here.
- Real agents run, with the person's real provider logins, in the real project folders, **only on a turn the person starts**.
  Nothing else acts by itself.

## The mode

`BACKPLANE_SIDE_BY_SIDE=1` is honoured only together with `BACKPLANE_COPY=1` (alone it is refused at start). `copy::on()` stays true, so every
gate that exists for a copy still holds; the mode adds exactly **one** permission and nothing else:

`crate::copy::real_agents() -> bool` = `on() && BACKPLANE_SIDE_BY_SIDE=1`. Consulted by `proc.rs` (Kind::Agent gate) and the agents'
`Cli::from_env`; nowhere else. With it, a real agent process may START only for a turn whose trigger is a person's message through the
`turn.start` RPC from a client (agents' rule, below). Everything else stays default-deny. A new actor added later is denied until someone
adds it to the table below (the source scan in `proc.rs` already fails the build on a child made outside `crate::proc`).

The hub's `/hello` and the view's info carry `sideBySide: true` so the client can say so.

## Environment contract

- The hub runs with the real `HOME` and `PATH` (the providers' login files are in `HOME`), but **without** `DBUS_SESSION_BUS_ADDRESS` and
  `XDG_RUNTIME_DIR` (no `secret-tool`, no keyring: the vault is never opened; Codex keeps its file login; Claude's is a file). Its own
  `--home` is the private copy: only `events.jsonl` plus a `restart.continue=off` setting appended. No `secrets/`, no `api.env`, no tokens
  of the live hub are copied, so the vault is empty and no key is injected by the hub.
- The agent runs with the hub's environment, cwd = the thread's folder, which is **the real project folder**: the agent can edit real files,
  that is the point. The Backplane MCP tools it is given are this hub's, under the same gates (a `thread_launch`, `work_run`, deploy, push is
  refused with the copy's message).
- A thread whose turn was `running` at the copy point is refused a new turn ("this turn was running in today's Backplane when the copy was
  made"): the live hub may still be working in that folder. Other threads on the same folder as a live turn are not detected: said in "Known missing".
- The copy is made from `head -c <size at start>` of the live log cut to the last complete line (the live hub keeps appending).

## Every automatic actor, how it is held off

Default-deny: "held" means a person-visible no-op on this hub; the owner lane implements it by `copy::on()` (not by `real_agents()`).

| Actor | Where | Held by | A person sees |
|---|---|---|---|
| Resume of turns that were running (restart carry) | `agents/mod.rs` recovery | copy fix b9f1c1673e (agents, in review) + `restart.continue=off` appended; running turns are logged `interrupted` in the copy | those threads read "interrupted" |
| Real agent start for anything but a person's `turn.start` (ride/wake_ride that starts a turn, start_queued/start_one/steer from commands, task and launch children, work/lock results, limit-fallback restart, `Launch.tell` reports, mail, bots' turns) | `agents/` | agents' rule: `real_agents()` plus trigger = client RPC, role user, no origin, not a retry; otherwise the stand-in/refusal | a queued message does not start by itself: "held on a copy: starts only when you send" |
| Routine and reminder ticker, `uber_init` | `bots/routines.rs` `start_ticker`, `tick_at` | **gate missing today**: `if copy::on() { return }` at the top of the loop and of `tick_at` (bots lane has it ready) | routines list says "paused on a copy"; no reminders |
| Mail recovery (`mail_recover`) | `bots/` `Bots::start` | **gate missing today**: same one line | none; no mail headers written |
| Owner outbox (20 s) and `owner_kick` | `bots/mail/owner.rs` | gated today (test proves it); no transport exists yet | none |
| State policies minute ticker (autosettle, alerts, told, summaries, liveness, star nudge) | `state/host.rs` `register_ticker` | **to verify by the state lane**: its commits go to the copy's log only (kept: settle states are the copy's own), but its `outs` that reach outside (push, webhook, summary text-model calls, alerts to desks) must be dropped on a copy | threads may settle; no alerts or summaries from a model |
| Push to phones, webhook, telemetry, update check | `settings/webhook.rs`, push, telemetry | `Kind::Net` refused (curl); webhook gated | Settings shows them as off on a copy |
| Far sync, peers, feeds, tailnet listener, pairing over tailnet | `net/farsync.rs`, `peers.rs`, `feed.rs`, `listener.rs`, `tailnet.rs`, `botnet.rs` | `Kind::Net` refused, listener never started, `hello_with` returns at once | no other machines listed |
| Deploys, restarts, updates | none in the Rust hub | no code; `deploy.*` RPCs refused | "not available on a copy" |
| Watched work (`work_run`), named locks | watch gate | `watch_argv` empty on a copy: every watch reads as gone | none |
| Terminals, browser helper, `preview_*`, teach, tasks | `term/` | `Kind::Terminal`/`Browser` not started | terminal/browser tabs say they are off on a copy (known missing) |
| Shell for `secret_run`, vault | `settings/` | `Kind::Shell` not started; no keyring env | secrets box empty |
| Git writes, `file.open`, release | `files/`, `commands/` | `copy::rpc_refused` + `files::git::proc` | told: "a hub on a copied log runs no git that writes" |
| Design history (`step`, `checkpoint`: hidden refs and `.backplane/history.git` in the real folder) | `agents/hist.rs` via `files::git` | **files'/agents' fix pending**: no capture on a copy (it would write into the real folder, twice with the live hub) | history bar shows nothing new |
| File watchers (kicad watch), KiCad IPC | `kicad/` | read only on the project; `Kind::Local`, no install path | works |
| Heartbeats (15 s per connection), the 1 s beat of a running turn | `wire/`, `agents/` | connection- or person-turn-bound, not actors | n/a |

## What the person sees in the UI (client lane)

- A one-line strip under the title: "Copy of your data. Edits stay here. Agents run only when you send. Nothing runs by itself."
  (from `sideBySide` in the view info; GPUI first, the other clients when they connect to this hub).
- A refused action (git write, launch, deploy, a routine) shows the hub's one-line reason in the usual error row, never a silent no-op.

## Known missing (the list the person gets)

1. No sync either way: the live Backplane's later work is not here; this copy's work never goes back.
2. No routines, reminders, bots waking, mail, uberbot behaviour; no notifications or push.
3. No terminals, browser, `preview_*`, taught tasks, secrets.
4. No git commit/push/PR from the app, no file-open or release; agents can still run `git` themselves in their folder.
5. No design history capture; history bar shows what the copy already had.
6. No other machines, pairing, tailnet.
7. A turn the live Backplane is running in the same folder is not detected unless it was running at the copy point: don't run the same folder in both.
8. The policy helper on the whole real log: unmeasured (see the rehearsal report); if it does not fit 3700M the mode ships only with a trimmed copy.

## Owners and what each must land before the first real run

core: `copy::real_agents()`, the flag refusal without `BACKPLANE_COPY=1`, `sideBySide` in `/hello`. agents: the person-turn rule, the
copy recovery fix, no design-history capture. bots: gates for the three ticker/mail items. state: verify policy outs. files: git/history.
switchover: `side-by-side.sh`, copy cut, env contract, the first-run checklist below. reviewer: OK this document and each range.

First-run checklist (the person's command does it; each is a hard stop): live hub answers `/hello`; port free; `/` has 20 GB beyond the
copy; the hub's `/proc/<pid>/environ` has `BACKPLANE_COPY=1`, `BACKPLANE_SIDE_BY_SIDE=1`, no DBUS/XDG_RUNTIME_DIR; the copy's log has
`restart.continue=off` as its last setting; after 30 s idle the copy's log has grown by no more than the policy's own settings (a count, no content).
