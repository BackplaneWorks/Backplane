# Named locks

`lock_acquire {name: "build", note: "native build"}` never blocks an MCP
call. It returns `{held:true}` when granted, or `{held:false, position,
holder}` while waiting. Position is one-based; zero means the holder is
reserved while waiting for an existing OS flock. End the turn after queuing.
Backplane posts one message when a queued thread is granted:

> Lock build is yours (waited 3m). Release it with lock_release when done.

`lock_release {name}` releases only the calling thread's lock.
`lock_cancel {name}` removes only its waiter. Repeating an acquisition keeps
its original FIFO position, timestamp and note, and appends no event.
`lock_list {}` lists every named lock, with the holder's id, kind, thread,
note, since, lease, readiness and FIFO waiters. Empty locks remain listed.
Names contain 1–64 letters, digits, hyphens or underscores.

`work_run {lock: "build"}` stores its work intent and its place in the queue
in one commit before starting anything. Its row says `waiting for lock build
(2nd)`, and moves up as waiters leave. Its timeout starts at its grant, not
when it queued. The work id holds the lock; the owning thread receives its
usual work outcome message, with no extra grant message. Cancelling a queued
work item prevents its launch. An ended work item releases its lock. A timeout reports uncertainty but keeps
watching and retains the lock until the command ends or is cancelled; locked
work cannot be unwatched. Locked
work is checked and watched again after a restart; the hub does not declare a live command lost or release its exclusion merely because the hub restarted.
An archived or deleted thread loses its held locks and queued requests through the normal retirement path.

The desktop shows held locks on the Status plane; the phones show them at the
top of the home list (`locks.list`, the `locks` push), with the holder's title,
age and waiting count. Selecting a thread holder opens its thread. A thread's
panel (the desktop's Subagents, the phones' Background work) lists its own
held and waiting locks with Release and Cancel, and its jobs with Stop.
The rows and what each button sends are the hub's (`present/panels.rs`).

## Shell commands

```sh
scripts/bp-lock.sh render -- blender --background scene.blend
```

The helper uses loopback HTTP to the hub of its home (`BACKPLANE_LOCK_HOME`,
which the hub gives the agents and jobs it starts, else `BACKPLANE_HOME`, else
`~/.backplane`; never `BACKPLANE_HOME` itself, which a test hub would take for its own): the
port from `BACKPLANE_LOCK_URL`, `BACKPLANE_PORT` or the home's `hub.lock`,
and the hub's pairing token from the home's `token` file, which the hub
needs on every lock request. `/lock/acquire`, `/lock/status`,
`/lock/renew`, `/lock/release` and `/lock/cancel` accept `{name, holder, note}`.
An external holder begins `pid:`; helpers add a per-invocation nonce to avoid
PID reuse. Acquisition with `wait:true` long-polls for up to 25 seconds on
the connection, without blocking the hub. A waiting helper then checks status
and renews its lease. A running helper renews every 30 seconds; an unrenewed
lease expires after 90 seconds, checked on the hub's 15-second heartbeat.
No MCP tool long-polls.

Normal exits release/cancel the request. The renewer exits when its parent
helper disappears. If renewal fails, it warns and keeps retrying while the
captured command continues. The native guard keeps exclusion until that
command's PID and start identity are gone, including after hub/helper death.
External commands should keep their work in the foreground. Use `work_run`
for hub-owned jobs.

If no hub answers the initial request, the helper uses `flock` on
`/tmp/bp-NAME.lock`. On macOS without flock(1), Perl uses the same flock(2).
Once registered with a hub, a lost connection warns and retries; it does not
switch to a different lock service mid-command. Arguments after `--` are
passed literally.

## Raw flock interop

Each hub reservation also owns `/tmp/bp-NAME.lock`. The hub holds the flock on
an open file, not blocking: an existing raw flock keeps the head of the queue
at position 0 until it is gone, retried on each tick, and `lock_list` names
the outside pid in `outside`. A job inherits the descriptor, so a job that
outlives the hub keeps the flock until the command ends. Release is `LOCK_UN`
on that file, so a stray process that inherited it holds nothing back while
the hub lives. A thread's or a helper's flock goes with the hub and is taken
again at the next tick; a raw flock taken in between is named in `outside`.

`BACKPLANE_LOCK_DIR` changes the guard/fallback directory. Tests set it to a
temporary directory for both the hub and helpers; they never operate on the
real `/tmp/bp-build.lock`. Raw callers in those tests use the same directory.

## Persistence

`LockChanged` records the complete holder, queue and readiness after a
transition; the fold keeps it as `lock.<name>`, with the names in `locks.names`.
A grant writes `ready` first and only then sends its message or launches its
job, so a retry finds nothing to do and a grant is never sent twice. A hub that
dies between the two leaves a thread holder untold: its lease warning tells it.
A holder found not ready at start is granted then.

## Progress leases

Thread and owned-work reservations have a progress lease, default 20 minutes
(`locks.lease.minutes`). Genuine tool output, a file write or growing work
output renews the owner's reservations. A periodic hub tick or a watchdog
message is not a heartbeat. Queued parties keep their FIFO place, and a new
holder gets a fresh lease when promoted.

After a holder's progress lease expires, the hub sends it one warning.
A running turn or outstanding owned work keeps its lock, even after that
warning: lease handling never stops a process or transfers its exclusion.
When the holder has no running turn or owned work and the warning is at
least 60 seconds old, the hub releases the reservation and notifies the
former holder and waiters. New real progress renews the lease and permits
a fresh warning only if that new lease also expires. Legacy thread records
without a lease receive one on their first check.
