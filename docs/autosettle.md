# Automatic settling

The hub settles clean, completed, quiet threads automatically. Settlement
keeps history and removes the finished green status; new messages or a new
turn reopen the thread. Failed and interrupted turns stay visible until a
later turn completes successfully. Pins, manual bring-back and snoozes keep
their existing meaning.

Workers (launched by any thread or bot, delegated tasks, and private runs)
use `settle.idle.minutes`, default **15 minutes**. Personal conversations use
`settle.personal.minutes`, default **240 minutes**. Both are editable in
Settings > Threads; zero disables that timer. `settle.done=on` now selects
the short worker grace for read, completed personal conversations. It never
bypasses the safety gates. The former `settle.days` setting is obsolete.

A turn must have completed, with no running or undelivered owned work,
queued input, pending ask or approval, wake subscription, held or queued
lock, or unread personal reply. A required report must already have reached
the launcher. Delegated results use TaskDelivered; local launch reports use
a receipt committed with the inbox message or durable Mailroom header.
Remote reports are acknowledged only after the receiving hub accepts them.
A muted report has no delivery obligation. A worker the person starts
chatting in retains unread reply protection.

Read receipts identify the actual reply, so a later reply in the same
second is still unread. Focused desktop desk.look and explicitly opening
the latest conversation on a phone acknowledge it. Background polling,
search and old history pages do not. Unread replies never expire merely
because time passed. Finishing work, answering an ask, dropping a wake or
releasing a lock starts a new grace period.

`src/core/autosettle.bend` is the pure live policy, shared by the hub's
minute planner and the one-time startup upgrade pass. The marker
`autosettle.version=1` commits with the pass. It applies the same policy to
existing threads and indexes legacy delivered reports once. Ordinary ticks
never scan the mail archive. Old Hub.settle/Shelve helpers remain only for
their historical law fixtures; production uses Shelve.auto.tick/fix.
The commit boundary checks every automatic settlement producer, including
private-run cleanup and merge notifications. A merge records its evidence
without bypassing the settling rule.

The tests include synthetic blockers, recovery, receipts and sidebar DOM
status classes. `test/tools/autosettle-dry-run.ts` evaluates a copied store
using the same Bend policy and never writes settlement to a running hub.
`test/autosettle_order_test.bend` compares accumulator output against
bounded recursive references, including receipt ordering and the upgrade
marker. The benchmark exercises 40 eligible decisions among 10,000 threads,
476 historical tasks and up to 20,000 archived reports. Its policy budget
is 750 ms; its full-tick budget is the indexed baseline plus 1,000 ms.

Qualification on 2026-11-03, based on main `f27e5be7`: the 10,000-thread
policy took 47–80 ms without archived reports and 28–53 ms with 20,000
reports, exercising 40 decisions every round. Paired full ticks were
1.760–3.175 seconds versus the previous indexed planner's 3.784–6.674
seconds. These measurements cover the planner, not the entire hub CPU
incident. The copied-store upgrade dry run at applied sequence 1341095
would settle **9 threads**. The live store was used only as a read-only
SQLite backup source; the policy evaluated the separate copy.

All six focused runtime suites and the required eight-file regression
gate passed. The ten new pure laws report `ALL PROOFS CHECK`. The full
`PROOF.bend` run passed the repository's strict `bend_verdict` gate with
only its unsafe/foreign-code dependency notice; it did not report a failed
law. Logs are in Boat job `j20261103041641-536529`, with separate order
and required-regression runs `j20261103042031-563596` and
`j20261103042155-574148`.

After rebasing onto main `1209f0bde`, the server check, settling/sidebar/order
suites and diff-cap render test passed in `j20261103044800-751935`. The
full proof exceeded Boat's 16 GiB machine; the identical source passed
on `h@backplane` under its `bend` lock (work
`w-t1793673649-1323966-1346575`, 8m08s, peak 16,454,660 KiB). Its verdict
contained only the unsafe/foreign-code notice. The rebase also moved
main's two handwritten diff-count labels into the message catalogs so
regeneration preserves them.
