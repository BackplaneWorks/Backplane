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
queued input, pending ask or approval, wake targeting real running work, held or queued
lock, or unread personal reply. A required report must already have reached
the launcher. Delegated results use TaskDelivered; local launch reports use
a receipt committed with the inbox message or durable Mailroom header.
Remote reports are acknowledged only after the receiving hub accepts them.
A muted report has no delivery obligation. A worker the person starts
chatting in retains unread reply protection.

A delegated task's initial prompt historically has the `User` role even
when another thread supplied it. For a task-backed child, the policy
recognizes only the exact seed ID reserved by `Mcp.delegate.go`: child
`t<time>-<seq>` uses seed `m<time>-<seq+1>`. New delegations also save that
identity as `autosettle.seed.<child>`. A parent link alone grants no
exemption. Any distinct human message, including one in the same second,
keeps unread protection; a later worker message cannot erase it.

Improvement-button jobs use their validated `improvement-<32 hex>` identity
and exact `improvement-request-<same key>` seed as background-job provenance,
so an unopened report uses the worker grace. This does not exempt subsequent
human messages. Automatic restart continuations use the existing Mailroom
origin classifier and do not turn a worker into a personal conversation.
Ordinary personal conversations still retain unread protection.

Wake subscriptions are dependencies, not evidence that their targets remain
alive. A transient reverse index follows thread-wake chains to real work,
including background/watched work, asks, queues, delegated results and locks.
Ended work and cycles of idle subscriptions cannot keep a completed thread
active forever. Unknown wake kinds remain conservative. Passive file/PR
subscriptions allow the completed owner to settle after its normal grace;
the next status tick retires inactive subscriptions of a settled owner.
Subscriptions targeting real work are preserved.

Work targets have immutable IDs. Once a subscription is at least one worker grace old and its target is
absent from both background and watched-work indexes, it retires even
while the subscriber remains active. A new
subscription gets the full grace for registration races. Timed-out work
whose outcome is still unknown retains its index row and its subscription.
Active file, PR and thread subscriptions are preserved; passive subscriptions
still do not count as owned running work. Setting the worker timer to zero
disables this orphan-work cleanup too.

Read receipts identify the actual reply, so a later reply in the same
second is still unread. Focused desktop desk.look and explicitly opening
the latest conversation on a phone acknowledge it. Background polling,
search and old history pages do not. Unread replies never expire merely
because time passed. Finishing work, answering an ask, dropping a wake or
releasing a lock starts a new grace period.

`src/core/autosettle` is the pure live policy, shared by the hub's
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
using the same policy and never writes settlement to a running hub.
`test/autosettle_order_test` compares accumulator output against
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
`PROOF` run passed the repository's strict `verdict` gate with
only its unsafe/foreign-code dependency notice; it did not report a failed
law. Logs are in Boat job `j20261103041641-536529`, with separate order
and required-regression runs `j20261103042031-563596` and
`j20261103042155-574148`.

After rebasing onto main `1209f0bde`, the server check, settling/sidebar/order
suites and diff-cap render test passed in `j20261103044800-751935`. The
full proof exceeded Boat's 16 GiB machine; the identical source passed
`w-t1793673649-1323966-1346575`, 8m08s, peak 16,454,660 KiB). Its verdict
contained only the unsafe/foreign-code notice. The rebase also moved
main's two handwritten diff-count labels into the message catalogs so
regeneration preserves them.

Post-deployment audit of copied snapshot 1358120 found nine old threads
blocked only on unread replies. Five were personal UI-test conversations.
Four were completed delegated workers whose tasks were already delivered;
their plain seed prompts had been mistaken for human input. Launcher
report delivery was not their blocker. The corrected policy makes those
four eligible in the same copy while preserving the five personal holds.
No live state was changed. Regression tests use the real delegation
producer, cover delivered/undelivered tasks and genuine human follow-ups,
and add two pure seed-identity laws.
