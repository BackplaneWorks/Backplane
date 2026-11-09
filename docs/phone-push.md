# Phone push

A phone is alerted by the uberbot, and by a thread's own alert record unless a desk in use is showing that thread (below, 0.14's rule). Code: `native/host/src/push/`; tests: `native/host/src/bots/tests/push.rs`; simulator check: `test/e2e/push/`.

## What may push

A push starts only from a `PushOrdered` event, and only `push::Source` can build one. Its fields are private. The ways to get one:

- the uberbot's `notify_person` (`needs_you`, `done`, or `reminder`/`digest` in a routine's turn);
- the uberbot's own open ask, since it cannot call a tool while it waits;
- a reminder that came due, when the person scheduled it or the uberbot set it with `remind_person`.

Nothing else holds one. A worker thread, another bot, a routine's turn, the mailroom, a webhook and a turn end write the `alert.<thread>` record, which reaches a desk and, as below, the phones. Their news goes to the uberbot as mail, which decides:

| Source | Reaches the uberbot as |
| --- | --- |
| Another bot's `notify_person` | triage mail (`act`) |
| A worker's ask | triage mail (`act`): it stays open at its source |
| A managed thread's end | triage mail (`fyi`) |
| An unmanaged thread's end | triage mail (`fyi`), which starts no turn |

**A thread's alert (0.14's rule, `src/server/push.bend`: "an alert is a push of its own, raised when a thread's alert record is written, unless a desk in use is showing
the thread").** A thread's `alert.<thread>` record with a kind (done, fail, ask, merged, a note, a reminder: the record a desk is told by) orders a push of its own
(`push::alert`, how `alert`), unless a focused desk in use shows the thread and the desk holds that kind (`settings/alerts.rs`, `Desk.seen` and `holds`, the same test the desk
uses). It says what 0.14's phone said: the record's own text (an ask, a merge, a note), else the last reply's first line, "Finished" with none, "Failed" for a failure;
a needs_you carries the ask's id so its answer clears it. The person's alert settings (`notify.settled`, `notify.needs`, which leave a quiet record) and everything under
Respect apply. The uberbot's own orders are unchanged.

The worker checks each order against the world again (`push::admit`), `setting.set` refuses `push.book`, and a test fails if any other file names the event or a source.

## Respect

In order:

1. **Repeat.** `needs_you` and `done` text said in the last day is refused against the told records. A reminder or digest may repeat.
2. **Quiet hours.** `notify.quiet` (`HH:MM-HH:MM`, default `22:00-07:00`, `off` to turn off), in the person's zone, America/New_York when the hub knows none. An order waits for the end. `notify.quiet.urgent` `on` lets `needs_you` pass.
3. **Burst.** After a push, others wait `push.burst.s` (120) and go as one.
4. **Mute.** `device.prefs {token, mute}`. A muted device hears only what `notify.quiet.urgent` lets through.

A collapsed push links its items' thread when they share one, else the uberbot's. Held items are kept in `push.book`, so a restart keeps them.

## What a phone gets

`aps.thread-id` groups by thread; `thread`, `ask`, `key` and `kind` ride beside it. The body is the first line, at most 180 characters. A tap opens the thread, at the approval when there is one.

The badge counts open asks plus the waiting items not about an ask. An item leaves when its ask closes or, with no ask, when the person writes in its thread or any device opens it (`push.seen`). Opening an approval does not answer it. Every device then gets a silent clear, and a phone drops anything stale on connect and when it comes to the front (`push.state`).

## Carriers

- **iOS:** APNs (`push/apns.rs`). `push.apns.key` (a path; the key itself is never stored), `push.apns.keyid`, `push.apns.team`. The topic is the bundle id the app registered, and a debug build registers `sandbox`. See `push-retries.md`.
- **Android:** no push service. The app hears the same alert and clear on its socket (data/1 `notify`, `clear`), so only while its process lives.

The Live Activity is not ported.
