# Named locks

`lock_acquire {name: "bend", note: "native build"}` never blocks an MCP
call. It returns `{held:true}` when granted, or `{held:false, position,
holder}` while waiting. Position is one-based; zero means the holder is
reserved while waiting for an existing OS flock. End the turn after queuing.
Backplane posts one message when a queued thread is granted:

> Lock bend is yours (waited 3m). Release it with lock_release when done.

`lock_release {name}` releases only the calling thread's lock.
`lock_cancel {name}` removes only its waiter. Repeating an acquisition keeps
its original FIFO position, timestamp and note, and appends no event.
`lock_list {}` lists every named lock, with the holder's id, kind, thread,
note, since, lease, readiness and FIFO waiters. Empty locks remain listed.
Names contain 1–64 letters, digits, hyphens or underscores.

`work_run {command: "scripts/check.sh", lock: "bend", note: "checks"}`
stores its work intent before starting anything. Its row says
`waiting for lock bend (2nd)` while queued, and positions update when other
waiters leave. The work id holds the lock; the owning thread receives its
usual work outcome message, with no extra grant message. Cancelling a queued
work item prevents its launch. An ended work item releases its lock. A timeout reports uncertainty but keeps
watching and retains the lock until the command ends or is cancelled; locked
work cannot be unwatched. Locked
work is checked and watched again after a restart; the hub does not declare a live command lost or release its exclusion merely because the hub restarted.
An archived or deleted thread loses its held locks and queued requests through the normal retirement path.

All four clients display held locks on the Status plane, with the holder's
title, age and waiting count. Selecting a thread holder opens its thread.
Its Subagents panel lists its own held/waiting locks with Release/Cancel.
The rows use the existing light/dark themes.

## Shell commands

```sh
scripts/bp-lock.sh bend -- scripts/check.sh
scripts/bp-lock.sh render -- blender --background scene.blend
```

The helper uses loopback HTTP (`BACKPLANE_LOCK_URL`, or
`http://127.0.0.1:${BACKPLANE_PORT:-3787}`). `/lock/acquire`, `/lock/status`,
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

Each hub reservation also owns `/tmp/bp-NAME.lock`. A captured child holds
the POSIX flock, tied to the hub by a pipe. Release closes that pipe and
returns immediately. A guard for owned work retains its flock until the
captured process identity ends; it never sends a signal. Hub death closes the
pipe too, and the same non-killing cleanup protects surviving work and shell
helpers. Retired guards are reaped on the hub's minute tick. Existing raw
`flock /tmp/bp-bend.lock ...` users therefore exclude hub-held `bend` work,
and an existing raw flock prevents a hub grant until it is released.

`BACKPLANE_LOCK_DIR` changes the guard/fallback directory. Tests set it to a
temporary directory for both the hub and helpers; they never operate on the
real `/tmp/bp-bend.lock`. Raw callers in those tests use the same directory.

## Persistence

`LockChanged` is a new append-only Change variant with a codec round trip.
It records the complete holder, queue and readiness after a transition.
`M.Locks.fold` projects it into a name index and one record per lock in the
read model's settings map. Reads use those indexes, never timeline entries.
The event variant preserves lock history explicitly and avoids changing the
meaning of existing SettingSet or WorkSet events. Grant readiness and its
wake message are committed together, so retrying a grant produces no second
message.

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
