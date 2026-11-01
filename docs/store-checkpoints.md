# Store checkpoints and bounded views

`events.jsonl` is authoritative. The hub takes its exclusive home lock before
opening or replacing `store.db`. Startup validates the schema, codec, numbered
facts, byte offset, ordinal and log identity. A compatible store catches up from
its applied byte offset in complete-line batches; an incompatible cache is
retained and rebuilt from the log before any view becomes writable.

Every acknowledged commit now fsyncs the log and commits its facts, affected
canonical records and checkpoint together in one SQLite transaction with FULL
synchronous mode. A failed transaction does not publish the proposed state or
run its effects. The rejected append is truncated and fsynced; if rollback
fails, further writes are held. The requesting RPC receives an explicit error,
including when its socket remains alive. An interrupted, unacknowledged append
is recovered from the authoritative log on the next boot.

Recent entries, history pages, message reads and search use the existing SQL
indexes. Startup no longer warms a second full-history thin index on the hub's
event loop. The legacy full-log client path still loads wire history; this is
measurable remaining startup and memory cost. Listening is reached only after
the store and canonical state are current.

For a large existing stale cache, an offline preparer may offer a derived
checkpoint as `<home>/.store-next/{store.db,READY.json}`. The manifest contains
`version:1`, `schema:1`, `codec` (storedb.bend's `Sdb.codec()`, the one version the JS checks are given by `Disk.prepare`), `prefixBytes`, `appliedSeq`, `prefixSha256` and
`storeSha256`. Prepare only in an isolated consistent copy. Never promote a
database from a test hub that has appended recovery or stand-in events.
At boot, before opening SQLite, the hub verifies both hashes, the entire
immutable log prefix and database invariants. Invalid offers are retained as
`.store-next.rejected-*`; valid offers replace only the derived database by an
atomic rename, retaining the previous database/WAL evidence. Remaining live
tail events are then applied normally. The event log is never replaced.

The native folded-shelf projection removes only unusable swipe controls from
closed settled/archived rows. IDs, hierarchy, titles, status and counts remain;
opening a shelf renders its controls from current state. This bounds the real
Older reply without raising the native payload limit or dropping history.
The web view, all draft inputs and unknown `@` keys remain unchanged. The
reviewed `thin-view.bend` hash in `sidebar-draft-inputs.json` changes only for
this native JSON projection; the required dependency guard remains enabled.

Focused regressions:

- `bend test/store_checkpoint_test.bend`: commits, crash tails, atomic SQL
  rollback, ordinal continuity, clean reopen, torn tails and schema/log fallback.
- `bun test/tools/store_checkpoint_io_test.cjs`: hash/prefix validation,
  atomic replacement failure boundaries, retained evidence, bounded Unicode
  reads and durable rejected-append rollback. Required by every local-small lane.
- `bend test/folded_actions_test.bend`: closed/open shelves and preserved rows.
- `bun test/tools/store_checkpoint_rpc_e2e.cjs BINARY`: disposable home,
  trigger installed before SQLite opens, failed RPC completion and subsequent
  successful commit. Blocks provider CLIs; never uses the person's home or UI.

Private faithful-copy qualification and timings belong in local receipts, not
the repository. An unavailable/incompatible checkpoint can still require an
expensive truthful rebuild. This change does not make that fallback instant.
