# Provider fallback

Settings → Agents → Failover order is one ordered list of providers (or
provider/model pairs) for this hub. It is on unless turned off, and starts as
Claude, Codex, Antigravity, Grok, each on its own default model. It applies to
every Backplane thread, bot, subthread, delegated task and routine. Move
entries up or down, remove them, add installed ones, or go back to the default
order. Turning it on never changes the model already selected in a thread.

At a confirmed usage limit:

1. Same provider first: the work moves to another connected account of the
   same provider with usage left, the one with the most left first, until every
   one of them has run out for this request.
2. Then the next provider: the first entry of the order, from the top, that is
   installed (or has an account connected), is not held, and has an account
   with usage left. A provider out earlier that has reset since is taken again.
3. Everything out: one timeline line names every provider's accounts and the
   earliest reset among all of them. The thread continues by itself at that
   time, on whatever is free then. A limit that named no reset is tried again an
   hour after it was met; a turn is the test.
4. Coming back: between turns, a thread the order moved down goes back up to
   the highest provider above it that has an account free again, and to the
   account it left once that account has reset.

Each switch writes one timeline line ("Switched to Codex (Spare): Claude
(Personal) ran out (resets 15:40)", "Back on Claude (Personal): it has reset").
The conversation is handed over the same way as a person's switch; parallel
agents on one account are fine. Turning the order off keeps a thread on its
provider: it moves between that provider's accounts and otherwise waits for
the reset.

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
provider tokens are consumed to manufacture limits. `test/fallback_test`
covers classification, scope/reset, order, replay, handoff, notifications and
auth recovery; `test/fallback_settings_test` exercises all Settings row
renderers; `test/tools/fallback_e2e.ts` runs the real hub and provider adapters.
