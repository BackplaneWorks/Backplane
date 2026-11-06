# Local diagnostics backend

> Paths under `src/` below (`src/core`, `src/server`, `src/web`, `src/mobile`, `src/app`) name the Bend JS tree, removed from this checkout (tag `main-bend-final`). Where the current Rust code for each is, is unverified; look under `native/host/src/` and `native/gpui/`.

`src/server/diagnostics` is the hub's bounded local recorder. The effects
in `effects/diagnostics.js` run synchronously on existing IO loop.
`effects/perf.js` feeds measured numeric values directly; log text, Chrome
traces and legacy log files are never inputs. There is no network sender,
installation identifier, project/thread/connection identifier, IP, source
path, prompt, message, exception text or environment dump in its schema.

## Integrated server API

The parent integrated the backend in `server` under the `DIAG` alias:

```python
import ./diagnostics as DIAG
```

`Main.boot` starts it immediately after the existing home lock and environment
binding, before `Agents.only(home)` and `t0 <- PS.Perf.us()`:

```python
Lock.refuse(String.is_empty(held), held, home)
H.Env.put("BACKPLANE_HOME", home)
diagnosticStatus : String <- DIAG.Diagnostics.start(Main.version())
```

`Main.version()` is the application's stamped version; the effect retains only its
numeric release portion for private exit records. The effect reads
`BACKPLANE_HOME` once and binds that home for its process
lifetime. It requires an absolute home and the hub's exclusive home lock.
Subsequent `start()` calls are idempotent and cannot switch destinations.
Only the hub calls start; helper children and browser clients do not.
The default is on, with the local on/off choice persisted in the recorder's
control byte. An invalid or torn control byte means off. Startup failures
return fixed status errors, disable collection and require a hub restart
after correcting storage; they never block startup of the actual hub.

The existing `Tick{}` branch's `do IO<Hub>` block calls:

```python
DIAG.Diagnostics.tick()
```

The existing one-second tick is required for idle retention and flushes;
there is no new timer, actor, promise, thread or continuous work loop.

Authenticated `GET /debug/diagnostics.json` returns the bounded JSON document
from `DIAG.Diagnostics.export()` as the response body, with
`Content-Type: application/json` and `Cache-Control: no-store`. This is local
maintenance/read export: it flushes pending aggregates and applies retention;
it does not upload, write an export copy or publish an issue. The route uses
the same owner authorization as other debug routes. Do not forward this body
through peer sync or telemetry, and do not substitute `/debug/log` or a trace.

Authenticated WebSocket RPC `diagnostics.control` reads parameter `action`.
The effect validates the closed set `off`, `on`, `clear`:

```python
reply : String <- DIAG.Diagnostics.control(action)
```

For example, `{"m":"diagnostics.control","p":{"action":"off"},"id":1}`.
The hub returns the parsed JSON status object as the RPC result, not an
escaped JSON string. Neither parameters nor commands can supply a storage
path. `Diagnostics.status()` is the lightweight local
status for UI refreshes; it reports the most recently measured storage
usage. `Diagnostics.flush()` forces bounded maintenance and returns status.
Neither export nor status is a mutation command. A caller must check `ok`
before reporting a durable control change as successful.

The existing actual perf call sites automatically feed:

- `Perf.span` and completed `Perf.mark`/`done`: only exact names in
  `diagnostics_operations`; startup, hub dispatch, step/commit and warmup.
- `Perf.add("hub.write_fail", n)` and `mail.append_fail`: `store_write` and
  `mail_append` errors. `hub.client_drop` gives `connection.drop`.
- `Perf.set`: only `hub.clients`, `hub.agents`, `hub.busy` gauges.
- Completed `hub.join`, `hub.mobile_join`, `hub.leave` spans also count
  `connection.join`, `connection.mobile_join`, `connection.leave`. These
  count handling of those messages; they do not infer reconnects or success.
- `Log.go` passes its severity number to `Perf.severity` after the existing
  log-level gate: 0 becomes `log_error`, 1 becomes `log_warn`; no other
  arguments reach diagnostics. `Perf.emit` text is never inspected.

For a confirmed connection lifecycle result not already measured, use:

```python
DIAG.Diagnostics.event("connection", "reconnect", 1)
```

Emit that only at an actual successful reconnection boundary; a join is not
proof of reconnect. Other closed connection details: `join`, `mobile_join`,
`leave`, `drop`, `connect`, `disconnect`, `failed`. Other explicit error
details: `disk_full`, `io`, `timeout`, `protocol`, `agent_exit`. The recorder
does not accept arbitrary names or inspect exception messages. Do not pass
client-supplied numeric diagnostics into this hub-only API.

## Export schema

The document is `{version:1, status:{...}, events:[...]}`. Event fields:

| event | fields |
| --- | --- |
| `latency` | `v:1`, `at`, closed `name`, `count`, `us`, `maxMs`, `meanMs` |
| `sample` | `v:1`, `at`, closed `name`, `count`, `value` |
| `error` | `v:1`, `at`, closed `code`, `count` |
| `connection` | `v:1`, `at`, closed `name` (`connection.reconnect`, etc.), `count` |

`at` is epoch seconds at the start of the aggregate; `us` is the sum of
measured microseconds, not a single duration. `meanMs = us / count / 1000`.
There is no claimed p90: the recorder does not maintain a percentile sample.
Latency totals saturate at Number.MAX_SAFE_INTEGER, counts and gauges at
U32 max. Internal monotonic sequence numbers order records across restarts
and are removed on export. The exact metric-name allowlist lives in the
effect, not a dynamic prefix match; see `diagnostics_operations` and
`diagnostics_gauges`. For example, `hub.rpc` is allowed, `hub.rpc.<user text>`
and arbitrary `hub.out.*` are rejected. Error codes are the set documented
above plus `store_write`, `mail_append`, `log_error`, `log_warn`.

Status contains `version`, `ok`, `started`, `enabled`, `error`, `stored_bytes`,
`allocated_bytes`, `cap_bytes`, `retention_seconds`, `pending`, `dropped`.
`pending`/`dropped` count aggregate records, not the number of measurements
inside them. Error is one of `""`, `full`, `io`, `unsafe_storage`, `capacity`,
`invalid_action`. No OS exception message is included. `ok:false` is material:
for example, off applies immediately in memory even if its disk write fails.

Publication boundary: validate the closed names/codes again and copy only
numeric fields and allowlisted names above. Inspect the chosen issue draft
before publication. The export function itself never accesses GitHub.

## Bounds, controls and recovery

Owned storage is `<home>/diagnostics/`: exactly `control` (one byte) and
eight `segment-0.jsonl` through `segment-7.jsonl` files of at most 64 KiB
each. There are no temp, staging, export, backup or legacy files. The hard
cap is **1 MiB for the sum of all owned file sizes**, including control and
every rotated segment, rather than a cap per file. `stat.blocks * 512` is
also summed and reported as allocated bytes (falling back to size when the
host provides no block count). The ordinary maximum payload is 512 KiB +
one byte, leaving space under the cap for allocation rounding. A whole
segment plus 8 KiB is reserved before writes. Usage is measured before and
after appends; unexpected allocated-block growth rolls the append back.
Directory entries, filesystem metadata, snapshots and filesystem-level
copy-on-write history are outside these file-byte/block measurements.

Every measurement is an in-memory update of one closed aggregate key. At
most 52 keys exist (32 latency, 9 error, 8 connection, 3 gauge); arbitrary
input names cannot grow this set. Flushes are at most once per 30 seconds
from the tick, or on explicit export/flush. A flush performs at most eight
64 KiB reads and one bounded batch append. Exports have a 1 MiB UTF-8 limit
and create no disk copy. Explicit authenticated export/flush requests still
need the route's usual request throttling; there is no arbitrary work queue.

Retention is seven days, applied at startup, flush, export and idle ticks.
Any segment containing an expired or future-dated record is removed whole,
possibly discarding some newer rows; expiration is therefore conservative.
Pending aggregates expire too. A torn final row is truncated to its last
complete newline; invalid complete rows discard the whole segment. The
export re-reads and reconstructs every record through the closed schema,
including files edited after startup. Unknown fields are never copied.

`off` drops pending data immediately, stops collection and persists off;
existing retained records remain available for export/clear. `clear` drops
pending data and deletes all segments, preserving on/off. `on` persists on
before collecting. The control byte is truncated, rewritten and fsynced;
an interrupted/ENOSPC write leaves a missing/invalid byte disabled on restart.
If truncation or fsync itself fails, `ok:false` explicitly means persistence
was not confirmed. Successful control writes are fsynced; diagnostic batches
are deliberately not fsynced, so a power loss may lose recent diagnostics.

ENOSPC/quota/other IO errors never escape into the measured operation. A
partial append is rolled back to its prior measured size; if rollback fails
the affected segment is removed. Pending data is dropped, collection pauses,
and maintenance retries after 60 seconds without queue growth or a log storm.
Storage failures never become an excuse to write a free-form error log here.
Owner-only directory/file modes are 0700/0600. Symlinks, hardlinks and unknown
directory entries fail closed; no arbitrary legacy log is opened/truncated.
This assumes the hub's existing home lock and an owner-controlled home;
it is not a filesystem sandbox against a concurrent process with that same
owner's privileges.

## Focused validation

```sh
node --test test/tools/diagnostics_backend_test.cjs
bun test ./test/tools/diagnostics_backend_test.cjs
```

Both use disposable homes. The JS suite runs the production effects and
real perf functions with a deterministic clock, real rotation/restart files,
fault-injected IO and Linux `/dev/full` for actual kernel ENOSPC. It covers
total measured storage, block accounting, retention, torn tails, publication
sanitization, controls, full-disk failure/backoff, absence of measurement-path
IO and symlink/legacy-file exclusion. program checks the actual
effect registration/marshalling, severity-only logging and clear/off behavior.

## Local hub exit record (#590)

After the hub owns its home lock and starts diagnostics, an `exit` row records
UTC Unix seconds (`at`), the process ID (`pid`), the numeric application version,
the actual process exit code, a closed exception kind, and up to eight stack
frame coordinates. Stack coordinates are `[line, column]` pairs in call order.
Error messages, function names, source file names, paths, URLs, accounts,
credentials, prompts and CAD data are never copied into the row. Frames without
numeric coordinates are omitted. An ordinary exit has `exception: "none"` and
an empty stack. Unknown or unstamped versions are `"unknown"`.

These rows use the existing eight rotating 64 KiB segments, measured 1 MiB total
logical/allocated disk cap, 512-byte record limit, private permissions and
seven-day retention. They add no file or timer. Off disables exit recording;
clear removes the existing rows. A subsequently exiting enabled hub can write
a new row. Exit rows remain private and are excluded from the report-export
endpoint, even when the reporter selects optional diagnostic measurements.
No exit record is uploaded automatically.

A non-handling `uncaughtExceptionMonitor` listener captures only the sanitized
exception shape; it neither handles the exception nor changes its exit status.
The scheduler's existing rejected-run path captures the same shape before its
existing `process.exit(1)`. A synchronous exit listener makes one bounded,
best-effort append. Every recording failure is contained. It does not set
`process.exitCode`, add an exception/rejection handler, retry, delay termination
with asynchronous work, or change the existing stderr behavior.

This cannot run after SIGKILL, native runtime abort, power loss, or a crash
before diagnostics initialization. Those need a separate supervisor or OS crash
facility; no signal handler is installed to change normal signal termination.
The record reports only an exit code delivered by the runtime, never an inferred
code for those cases. A full or unavailable disk can also prevent the record.

`node --test test/tools/hub_exit_record_test.cjs` runs actual Node and Bun child
processes in temporary homes. It checks fatal and explicit exits, the production
scheduler rejection hook, exact exit status, stack redaction, private storage,
retention, disabled collection, short writes, ENOSPC and exclusion from report
export. `test/tools/diagnostics_backend_test.cjs` retains the existing cap,
rotation and storage fault checks. These tests do not start a hub, contact a
network service, change credentials or use an installed home.
