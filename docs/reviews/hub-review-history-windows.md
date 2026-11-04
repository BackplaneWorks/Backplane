# Selected-thread review history windows

The review reader previously materialized every stored review action in one
query before applying the pure Bend fold. `Db.review_page` now seeks the
existing `(thread, seq)` index with an exclusive `after` and inclusive fixed
`through` sequence. The predicate remains exactly the selected thread,
`kind='act'`, `role='review'`, in ascending sequence order.

Each transient page has at most 128 rows and 1 MiB charged UTF-8 column bytes
plus 256 bytes per row. An indivisible oversized action travels alone and
advances the cursor once. Metadata lookahead remains in SQLite; an explicit
exhaustion flag distinguishes a byte-limited short page from EOF. Statements
are finalized and the page savepoint released on every path. No payload or
statement cache is introduced. Errors return no partial page or advanced
cursor. Index absence fails closed.

The small typed `ReviewWindow.fold` applies a caller's pure fold per page and
returns no accumulated value on any later failure or nonadvancing cursor.
The review adapter uses the existing `M.State.post` and resident/queued-op
merge. The complete baseline remains in the private render state; this only
bounds temporary I/O materialization. Canonical history eviction, durable
provider positions, recovery hydration and vault masking are unchanged.

Narrow qualification passed on frozen source `bab3fe485`, Boat job
`j20261104031852-353487`: seven actual Bun SQLite boundary cases, the complete
unchanged small IO/fold/db/test type closure, and four exact new canonical
budget, progress and failure-discard laws. A separate false-budget theorem
was rejected. Proof RSS peaked at 120,960 KiB; the controller peaked at
134,348,800 bytes. Workers had hard 3.7 GB/no-swap scopes and the controller
had a hard 512 MiB/no-swap scope. All OOM, limit and swap activity counters
remained zero. [Receipts](../../extra/hub-review-qualification/SUMMARY.json)
also retain prior failed attempts. Existing pure folds, merge and skip
policies received a source-preservation audit. The broad review-state
adapter, its two existing merge-law closures and the tiny emitted-program
runtime test remain unqualified; this PR is not ready to merge yet.
No full/root proof, full application compile, build, installation or live
store access is authorized. Startup, steady memory, CPU and p99 targets remain
open pending a qualified packaged artifact and copied-store measurements.
