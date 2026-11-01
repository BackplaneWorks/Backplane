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

## Upgrades in place

A store at a codec listed in `Sdb.codec.from()` (storedb.bend) is brought to
`Sdb.codec()` at boot without a rebuild, when what the newer codec adds can be
derived from a few kinds of change (storedb.bend's "Upgrades in place";
storeio.bend's `Sto.upgrade`). The log's lines before the store's
`applied_off` that name one of `Up.names()` are picked natively
(`Disk.pick`, a superset), each is decoded, and only the changes `Up.keep`
takes (by their decoded kind) are folded in log order through the real
`M.State.apply`. Their records and meta's codec are written in one
transaction, then read back. On any failure the transaction is rolled back
and the store is rebuilt from the log as before, so no half-upgraded store
is ever served; the log says which path ran ("upgraded store.db in place
..." or "in-place upgrade failed ...; rebuilding").

6 → 7: codec 7 adds `thread.parent.<child>` from `TaskDelegated` (unless its
parent is a bot's thread then). Its last value is that of the last
`TaskDelegated`, `ThreadParentSet` or `SettingSet` of the key; which thread
is a bot's changes only with `BotSet` and `BotDeleted`. Laws `sdb_up_*`.
On a 910 MB log: 437 lines picked, 419 rows, 12 s, against a 547 s, 8 GB
rebuild.

A newer codec whose additions cannot be derived this way must not list the
old codec in `Sdb.codec.from()`; the old store is then rebuilt.

Tests: `bun test/tools/store_checkpoint_io_test.cjs` (the JS checks keep a
store at an upgradable codec, a prepared store must be current, the line
picker), `bun test/tools/store_upgrade_e2e.ts OLD_HUB
test/native/store_up_probe.bend [EVENTS | OLD_HOME]` (a hub from before the
bump makes the old store; it is upgraded in place and must equal a rebuild
of the same log, every table; an upgrade made to fail falls back to the
rebuild and leaves the old store as it was).

## far_line after a rebuild

`far_line` lists the lines that go to the owner's other machines (`/far/log`
v3 reads it). Commits and the catch-up write its rows (storeio.bend's
`Plan.change`); the full rebuild did not, so a store rebuilt before the fix
lost its far history (on a real log: 232,919 rows before the Nov 1 rebuild,
28,262 after; the Oct 24 one had already dropped 319,771). The build now
writes them (stobuild.bend's `Stb.far`) and marks meta `far_line` 1.

A store without that mark gets its rows refilled once the hub serves, by a
child (`backplane --store-far HOME`, storefar.bend) at nice 19 and ionice
idle, its JS heap sized to 1 GB: the log is folded up to `applied_off` as
the build folds it (no entries, no turn inputs, no other facts), its lines
read by `effects/storefar.js` about 1 MB a call so their bytes stay in
JavaScript (a string over 64 KB in a line over 256 KB is emptied as text:
the refill reads only ids), and each read's rows are added in one short
transaction with meta `far_line.seq`, the last line written. A refill a
restart cut off folds from the start again (a row needs the state at its
line) but writes only past that line. The mark is set only when the
numbering ends where the store's does. On the real 910 MB log: 580,601
rows, 150 s, 633 MB peak; its rows equal both surviving older stores', row
for row, over their ranges.

Peers that pulled while the history was missing skipped those lines for
good, so the share version went to view:3 (far.bend's `Fr.view.tag`): every
mirror's next pull is one snapshot of what is live, not the history.

Tests: `bun test/tools/store_far_line_e2e.ts` (a whole build equals a
catch-up, every table, with a 3 MB line and a blank one; a store emptied of
far_line is refilled to the caught-up store's feed, row for row, and not
again; a resumed refill writes nothing up to its checkpoint) and
`bun test/tools/far_line_peer_e2e.ts OLD_HUB NEW_HUB WIRE` (a peer that
skipped the renames inside the hole ends consistent after one snapshot).

## Search terms' own version

The term table is a function of the entry table alone (each row's text and
ordinal), so a change to how terms are made (`THIN.Words.of`, the search
fold) bumps `Sdb.terms.v()` (storedb.bend), not `Sdb.codec()`: no rebuild.
A store whose meta `terms` is behind (a store without the key is version 1)
keeps serving, and once the hub serves (server/storeterms.bend):

1. a child (`backplane --store-terms HOME`, nice 19, ionice idle, a 1 GB
   JS heap) clears any `term_old` a
   previous swap left, then fills `term_next` from the entries, 2,000 per
   short transaction, in ordinal order; after a restart it goes on from
   `term_next`'s last ordinal (meta `terms.next` names the version it is
   for), and says `built <last ordinal>`;
2. the hub, in one script on its own handle (between two commits), adds the
   terms of the entries committed since, renames `term` to `term_old` and
   `term_next` to `term`, and sets meta `terms`;
3. the child runs again and deletes `term_old` in batches.

Search answers from the old terms until the swap. On a real 910 MB log (337,695 entries, 21.2
million term rows) the child took 260 s; it holds about 0.6 GB, up to 1.3
GB while it makes the terms of one 5.7 MB shell output (the terms must be
the ones a commit makes, from the whole text). Its rows equal the store's
own but for 198 rows of 7 old system messages whose stored terms came from
a longer text than the one stored. Laws `sdb_terms_*`; test
`bun test/tools/store_terms_e2e.ts` (a store with another version's terms
gets terms equal to a fresh build's, row for row, and is then left alone).
