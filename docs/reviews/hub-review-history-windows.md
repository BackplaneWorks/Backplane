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

Qualification is pending. The proposed Boat scope includes actual Bun SQLite
boundary tests, the complete small IO/fold closure, and the exact new budget
and progress laws with a false-budget control. The broad review-state
integration and its existing laws require separately stated qualification.
No full/root proof, full application compile, build, installation or live
store access is authorized. Startup, steady memory, CPU and p99 targets remain
open pending a qualified packaged artifact and copied-store measurements.
