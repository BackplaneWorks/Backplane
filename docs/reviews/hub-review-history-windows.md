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

Narrow qualification passed on frozen source `9ac6c179d`, Boat job
`j20261104033233-478688`: seven actual Bun SQLite boundary cases, the complete
unchanged small IO/fold/db/test type closure, and four exact new canonical
budget, progress and failure-discard laws. A separate false-budget theorem
was rejected. Proof RSS peaked at 128,768 KiB; the controller peaked at
136,159,232 bytes. Workers had hard 3.7 GB/no-swap scopes and the controller
had a hard 512 MiB/no-swap scope. All OOM, limit and swap activity counters
remained zero. [Receipts](../../extra/hub-review-qualification/SUMMARY.json)
also retain prior failed attempts. Existing pure folds, merge and skip
policies received a source-preservation audit. The broad review-state
adapter and its two existing merge-law closures remain unqualified;
this PR is not ready to merge yet.
The first executable smoke test caught a foreign-ABI mistake: the exhaustion
flag must be a primitive JS boolean. The corrected boundary suite explicitly
checks its representation. Pre-fix receipts are retained as historical only.

The corrected tiny executable smoke test passed all seven checks on Boat
job `j20261104033316-483868`, source `b10f4fe99`. The real Bend compiler emitted
only `test/review_window_test.bend` through the merged `heavy-boat.sh` route,
under 3.7 GB/no swap with an actual native flock and unchanged 3 GiB admission
floor. Compiler-route peak child RSS was 141,528 KiB. The actual Bun runtime
ran under a separate 512 MiB/no-swap scope and verified the three-page SQLite
fold, pinned inclusive sequence, empty result, complete partial-fold discard,
cursor progress and statement/savepoint cleanup. No new proof ran; exact
unchanged small inputs and all four canonical blocks were checked against the
qualified receipt. [Runtime receipts](../../extra/hub-review-qualification/RUNTIME.json)
record tool/source hashes, raw exit, checks and resource counters. No OOM or
limit events occurred. This does not qualify the broad adapter or a package.
One admitted normal-Boat attempt at the complete review integration closure,
job `j20261104032247-386085`, returned worker-launcher exit -15 after about
16.6 seconds. The starter marker verifies the 3.7 GB worker cap; there is no
final verdict or resource receipt, so the termination cause is unknown.
[The incomplete attempt](../../extra/hub-review-qualification/INTEGRATION-INCOMPLETE.json)
is retained. No broad retry or larger cap is planned; integration waits for
the CI owner's qualification route.

The subsequent owner-controlled exact `418050be7` gate reused the existing
normal Boat VM without a new start, job `j20261104034430-579852`. Its positive
worker reached the original guarded proof executor and was OOM-killed at the
unchanged 3,700,000,000-byte cap with swap disabled. Systemd and the own-VM
journal establish that cause; there was no final law/resource verdict and
the negative control did not run. The controller's 37 MiB RSS was not a proof
peak. [The incident receipt](../../extra/hub-review-qualification/INTEGRATION-418-OOM.json)
and raw journal are retained. Neither existing law produced a counterexample.

After explicit writer release, `State.window.file` moves with its identical
signature and body into `window-file.bend`, which imports only Base/model.
Thin-view keeps an argument-preserving delegate, and ReviewHistory switches
its import to the production leaf. All ReviewHistory function bodies remain
unchanged. The gate checks the exact extraction and reconstructs the old
thin-view source hash to detect unrelated edits. Its compiler packet contains
the whole actual revised adapter/test import closure; no renderer substitute
or declaration slicing is used. The two existing canonical merge laws and
proofs remain unchanged, with a separately corrupted-merge negative control.

`review_history_adapter_test.bend` exercises the production adapter at
`M.State` over 260 stored actions, with None/Some/empty results, partial-fold
discard, resident queued duplicate payload precedence, sequence 522 excluding
future 524, full review text/order and skip behavior. It imports no renderer.
The actual executable must return zero, contain no `FAIL`, match all 31 expected
checks including fixture multiplicities, and end with the close check.
The old legacy renderer test is preserved. Both new qualification stages remain
pending on the new frozen source; no claim is made that this closure fits yet.

No full/root proof, full application compile, build, installation or live
store access is authorized. Startup, steady memory, CPU and p99 targets remain
open pending a qualified packaged artifact and copied-store measurements.
