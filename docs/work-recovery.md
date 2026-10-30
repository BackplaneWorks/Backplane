# Stalled agents and recovery

The hub records each thread's last real progress in `liveness.<thread>`:
tool/model output, a file write, or growth in owned work output. Child-thread
progress also renews its active parent's progress and lock leases. Protocol
pings, quota polling, repeated unchanged task descriptions, watchdog notices
and injected-message echoes do not renew progress.

After `watchdog.minutes` without progress (default `20`), the hub injects
“You appear stalled at …; report status or continue.” It sends that nudge
once per progress gap. After `watchdog.grace.seconds` (default `60`) without
new progress, it marks the thread **stalled** and sends one notice to its
launcher, or Bub when there is no launcher. The sidebar and Status plane
show a distinct warning with the last operation or dependency. Waiting on a
thread or unanswered user request is included. New progress clears the
warning and starts a fresh gap. `watchdog.minutes: 0` disables this watchdog.

The watchdog runs on the hub's minute tick. Stream progress is recorded at
five-second precision. File observation is an incremental, bounded metadata
scan of active working folders; it excludes hub state and internal metadata,
does not follow symlinks, and never scans an entire game folder in one turn.
An exceptionally large tree can delay noticing a deep write. Owned work's
output files are probed separately. No watchdog action kills or interrupts
work; a quiet computation may need a status check and continues running.
Automatic messages for running non-Claude agents wait for their next tool
result or natural turn boundary, even when an older queue exists. Only an
explicit steer requests a turn restart.

## Reporting deadlines

Owned work defaults to a 20-minute reporting deadline. Set
`work.timeout.seconds` to change the default. `work_run` and `work_watch`
accept `timeoutSec`; explicit `0` disables their deadline. Invalid values use
the configured default. Provider background tasks and monitors get the same
default when discovered, and can opt out with `timeoutSec: 0` in their task
start envelope. Tasks that predate this support receive a deadline when the
hub first checks them.

A deadline reports to the owning thread once. It never signals or kills the
process. Background tasks remain owned until the provider reports their end.
Watched jobs keep an unknown-outcome row after a timeout and remain observed,
including after a hub restart, so their eventual exit can still be delivered.
Locks over live work remain held.

A terminal provider quota error persists a retry marker with the thread's
provider/model and the provider's reported reset time, independently of the
optional fallback chain. The hub resumes once when that reset arrives, with
a continuation that asks the agent to check completed work before repeating
an action. It does not resume an archived/deleted thread, a running turn,
or one waiting on an approval. A fresh turn, an explicit provider change,
or Stop cancels that automatic retry. Unknown or already expired reset
evidence never creates an invented retry time.

Restart recovery already defaults to `restart.continue: on`. Interrupted
turns continue in their existing session with a warning to inspect any tool
call whose result was lost. The carry mechanism limits starts per tick and
avoids an immediate restart loop when a resumed turn made no progress. Such
a turn receives a durable delayed retry: 60 seconds, then 120 seconds, up to
20 minutes between attempts. Genuine output resets that backoff. Adopted
processes are never started a second time; Stop and disabling restart
continuation cancel pending retries. Delegated tasks retain ownership while
their continuation waits and finish only on their actual result.
