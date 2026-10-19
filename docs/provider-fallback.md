# Provider fallback

Settings → Fallback hierarchy controls one opt-in ordered list for this hub.
It applies to every Backplane thread, bot, delegated task, routine and mail
short-run. Add installed provider/model pairs, move them up or down, or remove
them. Turning it on never changes the model already selected in a thread.

Your chosen model runs first. A confirmed usage limit selects the next
available entry after that exact provider/model. If the choice is absent,
search from the top. The list never wraps. Entries missing from the installed
catalog or held by a known limit are skipped. A working fallback stays selected;
resetting a limit never moves it back automatically. An explicit model choice
starts a new choice of intent.

A reported reset makes that provider or model eligible at the reported time.
Without a reset, its availability hold remains until cleared in Settings. This
avoids repeated launches against an exhausted account. Clearing a hold does not
move working threads; paused work can be resumed explicitly. Exhausting the
whole list preserves pending work and raises one action-needed notification.

An automatic switch creates a persistent notification naming the thread, old
and new provider/model, reason, affected active count and known reset. Other
threads affected by the same outage get their own audit row without another
alert. Notifications use the existing desktop/web/mobile alert record and
thread link, rather than pretending a successful switch is blocked work.

Login failures, cancellations, network and tool errors do not cause fallback.
A locked desktop keyring pauses affected work with one shared unlock alert.
After a confirmed unlock, at most one held recovery resumes per five-second
beat, with one probe in flight. Normal launches keep their existing behavior.
Stop cancels automatic recovery while keeping its input available for manual
Resume. No credentials are cached or moved to another storage backend.

Provider changes happen at the existing turn boundary. The same thread,
workspace, delegated task and mail run remain the owner. Model sessions are
kept separately; a model without its own saved session starts fresh and receives
recorded context. A continuation asks it to inspect completed tool results and
continue unfinished work. A prompt refused before launch is saved verbatim.
Existing persisted event variants remain unchanged; fallback state and alerts
use append-compatible settings and timeline activity records.

Validation uses fake Claude/Codex CLIs and fictional catalog entries. No real
provider tokens are consumed to manufacture limits. `test/fallback_test.bend`
covers classification, scope/reset, order, replay, handoff, notifications and
auth recovery; `test/fallback_settings_test.bend` exercises all Settings row
renderers; `test/tools/fallback_e2e.ts` runs the real hub and provider adapters.
