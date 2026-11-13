# Hub resilience

What keeps a hub up, and how to bring one back. Four layers, each for what the one inside it cannot do:

| Layer | Where | What it does |
|---|---|---|
| The hub | `native/host/src/guard.rs`, `store.rs` | Starts on bad state: quarantines a bad record or checkpoint, goes on without a part that panics or hangs. Says how it is on `/healthz`. |
| The guard | `native/guard` (`backplane`) | A small program that shares no code with the hub. Starts the hub, watches it, walks the ladder below, answers on the hub's port while it is down. Same on Windows, Linux and macOS. |
| The watcher | `scripts/fresh-watch.sh` | Follows `CURRENT`, relaunches when no turn is running, passes over builds in `BAD`. |
| The outside watchdog | `scripts/hub-watchdog.sh` | A systemd user timer that shares nothing with the others. For when the guard or the watcher is what died. |

## One command

    backplane hub status              what it is doing, what failed last
    backplane hub restart [--now]     restart it
    backplane hub safe [--now]        restart with agents, routines and bots paused
    backplane hub normal [--now]      restart out of safe mode
    backplane hub rollback [--now]    mark this build bad, go back to the last good one
    backplane hub use BUILD [--now]   run this build; takes its bad mark off
    backplane hub update BUILD        run this build on probation
    backplane hub stop                stop hub and guard (only at the machine)
    backplane hub log                 where the logs are, the last incidents

Without `--now` it refuses while turns run and says how many. `--home DIR`, else `$BACKPLANE_HOME`, else the hub started with `--default`. If no guard is running, `restart` starts one from `<home>/guard/launch.json`.

From another machine: `--url http://HOST:PORT --token-file F` with `status`, `restart`, `safe`, `normal`, `rollback`. On this desktop, `scripts/fresh.sh restart|safe|rollback` does the same for the fresh hub.

In the window: the palette's Restart hub, Restart in safe mode, Roll back hub, and the strip at the top when the hub needs you. They use plain HTTP, never a view or the socket, so they work when those are what broke. In the phone apps: the Hub screen.

## The ladder

After a hub ends without being asked to:

1. The same build again, pausing 1, 2, 4 s.
2. After 3 failures in 600 s: mark it bad, set the checkpoints aside, start the last build that started cleanly on this home.
3. If that fails as well, the home is at fault, not a build: safe mode, skipping the step the failed start stopped at.
4. If safe mode fails: rescue. The guard answers on the hub's own addresses, and tries again every two minutes.

The machine's own failures (exit 75: port or lock held, disk full) never mark a build bad. A refusal (exit 78, or 2: a copy gate, a bad flag) goes straight to rescue. A build is **good** after `/healthz` has said 200 for 60 s from its first ready, with no fold still running.

An **update** (`backplane hub update`, or the hub's updater through the same request) is on probation: one failure before it is good goes straight back, and `CURRENT` moves only once it is good. It must also be the build it was staged as: when `builds/<id>/EXPECT` (`{"version","commit"}`, written by staging from the signed manifest) is there, the first ready `/healthz` must say that version and commit, or the start fails with `answered as v/c, expected v/c` and goes back. The commit is compiled into the hub (`BACKPLANE_COMMIT` at build time: `dev-build.sh`, `release.yml`); the guard never sets it, so the check proves which build runs. An update that comes up without a part the build before had running (`degraded` on `/healthz`: a start step panicked or ran out of time) fails probation the same way; a part that was already down is the home's, and does not count. One request waits at a time: a second is refused (409, "another request is waiting") rather than replacing the first.

The first start after a failed one holds cut turns back for as long as it takes to prove itself (`BACKPLANE_CARRY_WAIT`), so a build that dies at once does not start every agent each time.

## Safe mode

The hub serves, the log and the views work, and nothing acts by itself: no agent starts (a turn is refused with the reason), no turn carries on, no routine, wake, mail run or policy. Tailscale and pairing stay up, so the person can reach it to bring it back. Places in the carry line are kept for the normal start that follows.

## Bad state

- **A line that is not JSON** stays in the log, skipped as before, and is noted in `quarantine/bad-lines.jsonl`.
- **A line that panics a fold** is quarantined: `Store::fold` catches it, every later read gives `{"$":"Quarantined"}` in its place, a copy goes to `quarantine/lines.jsonl`. The log keeps its bytes. Delete a line from `lines.jsonl` to take it back.
- **A checkpoint** that is corrupt fails its sum (`index.bin`, `model.bin`) or its fingerprint, and the log is folded instead. A fallback, a rollback or an update moves `ckpt/` to `quarantine/ckpt-<time>` first.
- **A part of the start** (`files`, `settings`, `term`, `agents`, `bots`, `commands`) that panics, or takes longer than `BACKPLANE_STEP_BUDGET` (120 s), costs that part alone. `/healthz` says `degraded` and names it.
- **`devices-v1.json`** that cannot be read stays as it is. The hub comes up on the hub token and the devices `Auth` could parse, and says `pairing` is not running.
- **A full disk**: writes fail with an error and the log is cut back to its last whole line. The hub still starts and serves what it has (`log writes` is degraded). The guard keeps 32 MB of ballast in `guard/` and gives it back when under 64 MB is free.
- **A full temp folder** counts as a full disk. On this desktop `/tmp` is a 30 GB RAM tmpfs shared by every agent; when it fills, agents' writes fail with ENOSPC or EDQUOT. A start that fails on either is the machine's fault (exit 75), never the build's. Every 30 s the guard looks at the home's disk and the temp folder; under 1 GiB free, or under 5 % and under 10 GiB, is told once (log, incident, desktop) and again when there is room. While the temp folder is low and the home's disk is not, each hub start (and every agent under it) gets `TMPDIR=<home>/tmp`. `status.json` lists what is low in `low`.

## Health

`GET /healthz` needs no credential and tells nothing private. It is answered before anything else in the hub, and by the bound listener while the hub is still starting.

    200 {"status":"ok"|"degraded", "safe":false, "control":1, "guarded":true, "folding":0, "hung":[], "degraded":[]}
        + "version", "commit" for a caller with a credential (the guard sends the home's token); never to the open network
    503 {"status":"starting", "phase":"step:agents"}          the listener is up, the start is not over
    503 {"status":"hung", "hung":["writer"]}                  something inside stopped answering for 90 s
    503 {"status":"down", "rescue":1, "control":1, "why":…}   the guard, answering for a hub that is down

The hub watches its writer, read model and agents' world from outside them, every 10 s. The guard probes every 5 s; 6 misses end the hub with SIGABRT. A start is stopped when `guard/phase` has not moved for 300 s (a long fold keeps it moving) or after 900 s in all.

## Remote control

`POST /hub/control {"action":…, "build"?, "machine"?}` and `GET /hub/status`; on a socket, `hub.control` and `hub.status`. `action`: restart, normal, safe, rollback, use, update. Allowed: the hub token, a live paired device, or a signed request from a linked machine whose `peer.own.<id>` is `pair` or `owner`. `machine` forwards the request to that linked machine.

The same routes answer in rescue, with the same credentials, on the same addresses, so a phone or a paired machine that knew the hub reaches something that can bring it back.

So that a remote restart cannot lock the person out:

- `stop` is refused over the network.
- A rollback or update goes only to a build whose hub has these routes: anything older could not be reached again.
- Three restarts in five minutes from other devices is the limit, so a stuck client cannot hold the hub down. The machine's own CLI has no limit.
- Every outcome of a restart stays reachable: the ladder ends in rescue, never in nothing. Safe mode keeps the network up.
- A restart changes no token, pairing or `tailscale serve` entry.

## Alerts

Every step of the ladder writes a `guard:` line in the hub's log, a line in `guard/incidents.jsonl` and a desktop notification. Once the hub is up, clients get the news: `notice` in `/hub/status`, the `hub.notice` info frame and `notice` in the data/1 hello. After a restart that has held for 30 s the uberbot gets a turn of its own to pick up what the restart halted.

## Files

`<home>/guard/`: `status.json` (what the guard is doing; `lastRequest` holds the answer to a request), `request` (a request: whole, renamed into place), `incidents.jsonl`, `good`, `control` (builds with the control routes), `launch.json`, `phase`, `panic.txt`, `listeners`, `peers.json`, `ballast`, `lock`.

Beside the builds (`--builds DIR`: `builds/<id>/`, `CURRENT`, the same on a release install): `BAD` (id, tab, why), `.fresh-bad` (the newest bad id), `.pins/<port>` (builds nothing may prune). The hub gets `BACKPLANE_BUILDS`.

A guard is replaced only when it next starts, never in place: nothing watches it, so a broken one must not be able to replace a working one. An old guard must cope with a newer hub, so what is between them only grows: `/healthz` (or `/hello`), the exit codes, these files.

## Publishing

`scripts/dev-build.sh` refuses to publish a build that fails `scripts/smoke-start.sh`: on a throwaway real-shaped home (`test/guard/make-home.py`) the build before writes its checkpoints; this hub starts under this guard, serves, restarts twice; then the build before must start on what it wrote. A panic, a part not running, safe mode, a quarantined line or a lost line fails it.

## The outside watchdog

    scripts/hub-watchdog.sh install | uninstall | status | check

Every 30 s: `/hello` on the fresh hub, and whether `fresh-watch.sh` lives. A hub still starting is left to its guard. After 3 failed checks: `fresh.sh stop`, `fresh.sh start`; if it still fails, `dev-cut.sh back` and the same again. A dead watcher is started again. Each restart waits twice as long as the last, from 2 minutes up to an hour, and 10 minutes of health resets that. Every action goes to `~/.backplane-gpui-fresh/watchdog.log`. It also looks at the hub's disk and the temp folder by the guard's rule, and logs and notifies once when either is nearly full; restarting does not free space, so it does not restart for that. A person's `fresh.sh stop` leaves `.stopped`, and the watchdog leaves a stopped hub stopped. On a healthy hub it also reads the event log written since its last look for a turn start that failed on the accounts ("could not start claude: Accounts are not available", a profile to reconnect): the hub retries those itself, so any is a guard failure; it is logged, put on the desktop once, and the hub is asked to start those turns again (`POST /dev/account-retry`, behind the token). `test/hub-watchdog-account.sh` checks it against a stand-in hub. One check runs at a time under `.watchdog/run.lock`, taken with `flock -o` so nothing a restart starts (the hub, the window, the watcher, every agent) carries it: twice a leaked lock left the watchdog silently doing nothing for a day (Nov 8–9, Nov 11–12). A check that finds the lock taken for 5 minutes (`WATCHDOG_STUCK`) logs it with the holders and puts it on the desktop. `test/hub-watchdog-restart.sh` kills a stand-in hub three times and checks it comes back each time.

Memory: the hub's scope and the window's carry `ManagedOOMPreference=avoid`, and every build or test that takes a build slot (`scripts/cargo-q`, `dev-build.sh`, `main-build.sh`) runs in a transient scope of its own in `bpbuild.slice` (`MemoryMax` 60 %), so under memory pressure systemd-oomd kills a build, not the hub (it killed the whole hub scope, builds and all, on Nov 12 at 01:25 and 02:16). `-j` is each slot's share of free memory. `test/buildslot-scope.sh` checks it.

Agents and jobs (Linux, `native/host/src/memscope.rs`): every agent process the hub starts and every `work_run` command runs in a transient scope of its own in `bpagent.slice` (`MemoryMax` 40 % each, `BACKPLANE_SCOPE_MEM_MAX`; the slice 75 %, `BACKPLANE_SLICE_MEM_MAX`), with `OOMPolicy=continue`: what an agent starts that eats memory is killed alone by the cgroup's OOM killer, inside its scope, and the agent, its other commands, the hub and every other thread go on. The thread is told ("Killed for memory: <process> (pid, size) … reached the memory cap of this thread's commands"), at once for an agent's child, in the job's end for a `work_run`. What runs there also gets the hub's `oom_score_adj` + 300, so a machine-wide OOM takes it before the hub or the window. Before this, agents ran in the hub's own scope (OOMPolicy=stop by default), and the kernel killing one 45 GB test client (Nov 13 04:42) made systemd stop the hub and every agent with it. The hub's own scope now has `OOMPolicy=continue` too (`fresh.sh`, and the window's launcher `native/gpui/src/launch.rs` when a user systemd answers). Off with `BACKPLANE_NO_SCOPE`, on a copy, in tests, and wherever no user systemd answers. `test/hub/memscope_e2e.mjs` checks it on a throwaway hub in a test slice: a child and a job that each eat 1.5 GB under a 400 MB cap are killed alone, another thread's turn finishes, the hub answers throughout, and the thread is told both times. macOS has no equivalent (no cgroups or per-tree memory caps; only a sampler that kills would do, not built). On Windows the equivalent is a Job Object memory limit (`JOB_OBJECT_LIMIT_JOB_MEMORY`) on each agent's job; agents already run in a Job Object there, without a limit (not done).

## Testing

    test/guard/faults.sh BUILD_DIR [FAULT...]

Real hubs on temp homes, each broken one way: a corrupt log, corrupt checkpoints, a poison record, a panicking build as `CURRENT`, a part that panics or hangs, every build dying, a port in use, a hung start, a frozen hub, writes failing, remote restarts, rescue, a remote rollback that would lock out, an update that fails, an update that answers as another commit, a nearly full temp folder, a killed guard. After each, every byte of the log is still there. `BACKPLANE_FAULT=panic:<step>`, `hang:<step>`, `die:<step>` and `panic-line:<text>` make a real hub fail in a real place.

## Not done

- `native/host` does not build on Windows yet (`libc::flock`, Unix signals, `FileExt`). The guard does, and is checked there.
- Restarting the machine itself: a guard is started by whatever starts the hub. On this desktop the watchdog's timer brings it back after a boot.
