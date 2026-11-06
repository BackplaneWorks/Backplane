# Anonymous usage

Backplane counts installs and how many people use it. Only the hub sends:
window, web, iOS and Android never contact the telemetry service.

Alpha also sends aggregate daily counts of threads created, messages, model
token usage, bot traffic and numeric board creation/completed-order counts for product and investor reporting. This is usage
measurement, not token billing. API keys, authentication tokens, conversation
content and credentials are never part of these counters.

An identified install sends a random UUIDv4, app version, channel, OS,
architecture and UTC date. Updates optionally include the preceding version.
A daily active event counts the UTC day of a successful authenticated allowlisted UI action, at most once per id/day. Idle rollover, reconnect, keepalive, automatic view refresh and provider or background work do not count. This is a UI interaction signal, not hardware attestation. Committed sharing, consent or reset changes discard pending action evidence even before an install id exists.
There are no app-kind counts, usage times, crash uploads, projects, files,
prompts, names, addresses, hardware identifiers or time zones in these events.
The request includes a send timestamp and random nonce for authentication and
replay protection; the server does not retain that timestamp as usage data.

The installation/first-run popup explains **all** telemetry before any can
leave the hub. Builds without sending capability do not interrupt onboarding with this popup; if capability becomes available, an unanswered choice is still required. Telemetry is selected On by default, but is blocked until the
person presses **Continue with telemetry on** (a recorded choice). **Turn off
all telemetry** records Off. **Decide later**, Escape, closing the app and
unanswered/offline first runs authorize nothing. A dismissed popup remains
pending and is offered again after reconnect or a new app session. Privacy/report links
leave it unanswered. The regular onboarding tour does not repeat an already
answered telemetry step; an interrupted automatic or manual tour resumes at its existing cursor. Only the hub sends; its saved choice is shared by
web, desktop and both phone clients, so answering on one ends the question on
all synchronized clients, including after reconnect. No per-client preference
silently overrides it.

An existing explicit Off stays Off even if an older version never wrote an
answer marker. A recorded expanded-metrics decline also stays Off. Existing
complete On choices remain On; upgrades and reinstalls using the same hub
home preserve these settings. An older On choice without the aggregate
disclosure stays stored, but all sends wait for the complete popup answer.
A genuinely new hub home has no recorded choice and starts pending. Reset id
rotates identity only, never enables telemetry or resets the choice.

One hub transaction records the share and disclosure fields. The UI decision
names the preferences and monotonic choice revision it displayed: a stale On answer or outbox replay cannot
overwrite a newer Off on another client. An explicit Off always stops sharing. Repeated decisions invalidate stale On requests; bounded replay receipts prevent lost acknowledgements from applying a decision twice. A receipt names one click: the client makes a decision id per click and keeps it in its outbox text, so only a resend of that very request is recognised; two people choosing the same value from the same displayed state are two decisions, and a request without an id is never treated as a replay. A client with several choices outstanding falls back, on a refusal, to the last preferences the hub confirmed, once none is outstanding, never to another unconfirmed proposal. Nothing may contact the network while any write waits in the disk-full queue: a choice folded into the hub's state but not yet on disk authorizes no registration or usage send until it is flushed, while a waiting Off still revokes at once and its cleanup still runs. A worker's snapshot is compared with the hub's state through the same projection it was built from, so an unchanged choice never reschedules it. Older clients may still turn telemetry Off or reset the id; individual legacy On/answer-marker writes are rejected. Each click has its own decision id, kept stable only for transport retries; independent users choosing Off from the same old screen remain distinct. Every decision reply carries authoritative preferences after the append is saved, so rejected choices and failed saves do not leave an optimistic On displayed.
An offline choice reaches the sender only after it is recorded by the hub;
there is no pre-choice history backfill. These are product preference records,
not a statement about legal consent.

Settings on every client has Share anonymous usage, On/Off, Reset id and
Privacy. Opting out clears the id, private queue and acknowledgements, and disables
**every** category, including anonymous install/update pings. Unanswered
installs never expire into anonymous transmission. Old unanswered/anonymous
queue entries from previous versions are discarded rather than released. Re-enabling or Reset id makes a fresh random id and
registers it with a proof of work. The id is stored in
`<home>/telemetry/state.json`, mode 0600, in a 0700 directory, and sent only
to authenticated clients as info `telemetry.id` for the copyable Install id
text in Settings. It never enters the event log. The info is empty when
there is no id or the person opted out. State writes use an atomic rename.
The queue expires after seven days, caps at 200 events (old activity first),
and sends at most 50 events / 16 KiB. A failed request retries no sooner than
one hour, including a longer numeric Retry-After. Schema/auth/replay rejection
drops its batch. Only HTTP 202 or permanent 400/401/403/409/413 responses consume a submitted batch; other statuses retain it with backoff. One background job runs at a time and HTTP times out at 10s.

Setting `BACKPLANE_TELEMETRY=0` (or `off`) or `DO_NOT_TRACK=1` disables all
telemetry, including anonymous counts and state writes. `CI` set,
`BACKPLANE_SANDBOX` nonempty except `0`, and `--sandbox` also disable it.
Missing endpoint, key id or key closes the gate. Developer builds bake no
configuration, regardless of developer environment variables or `.env`.

Release compilation explicitly sets `BACKPLANE_TELEMETRY_BAKE=1`,
`BACKPLANE_VERSION` (the full package version), and the endpoint/key id/key.
`scripts/telemetry-header.py` writes a private compile-only prelude (`.js`),
after the JS emission, which `scripts/build-js.sh` compiles into the binary
on every target. It never enters the emitted JS, emit caches or package
files. The prelude is removed when compilation ends. Missing configuration bakes empty
values; malformed configuration fails the build without printing values.

`BACKPLANE_TELEMETRY_CHANNEL` is immutable build metadata: `release` for a
stable version, `nightly` for `-nightly.`. The helper derives it from the
stamped/package version and rejects an explicit disagreement. If both
`BACKPLANE_VERSION` and `BACKPLANE_VERSION_STAMP` are supplied, their full
versions must agree after removing the leading `v`. Changing the user's
update preference never relabels telemetry from an installed package.

Release and nightly builds use the canonical receiver
`https://backplaneworks.com/api/v1/events` directly, with the same HMAC API
and key configuration. Builds do not rely on the legacy domain redirect.

Normal runtime environment variables cannot replace baked credentials or
redirect traffic. Explicit integration tests set `BACKPLANE_TELEMETRY_TEST=1`
and supply `BACKPLANE_TELEMETRY_URL`, `_KID`, `_KEY` and `_CHANNEL`; only a
literal `http://127.0.0.1:PORT/api/v1/events` endpoint is accepted in that
mode. CI, sandbox and opt-out kill switches still win. No credentials are
printed or passed to curl on its command line: curl reads headers/body from
a private temporary config, removed after the attempt.

Before starting HTTP, the worker asks the hub whether its recorded all-category
choice permits transmission and its sharing/answer/reset snapshot is current. Any pending durable-write queue blocks network authorization, including a folded but unsaved On; a queued Off still revokes immediately. A choice made during proof-of-work
invalidates that worker's pending request. A request already in flight can
finish; opt-out clears the visible id immediately and the next plan removes
queued identified data.

Open-source clients cannot keep a secret. HMAC-SHA256 over timestamp and raw
body raises the cost of garbage submissions; server schema validation,
version/channel allowlists, replay protection, rate limits and deduplication
are the real controls. An identified install uses 18-bit SHA-256 proof of work
in a disposable native child (`--telemetry-pow UUID`); the pure verifier
independently checks the result. This is not identity authentication.
See [Privacy](https://backplane.works/privacy) for server retention/deletion.

Report an issue opens a GitHub issue form. Its URL contains only validated
version, OS and architecture and an empty What happened prompt. Reporting is
an explicit link, never an automatic upload. The tour reminds people that
Backplane is in beta.

## Alpha aggregate contract

The additive wire-v1 event is `usage`. It has the same required `day`, `id`,
`share`, `ver`, `ch`, `os` and `arch` metadata as identified activity, plus a
`counts` object with these fixed nonnegative integer fields (maximum
281474976710655 each):

| Field | Counts |
| --- | --- |
| `threads_created` | Local `ThreadCreated` events, including child threads and forks; a fork marker alone adds nothing. |
| `user_messages` | Committed user-role `MessagePosted` events, including programmatic messages in bot threads. |
| `assistant_messages` | Committed assistant-role `MessagePosted` events; streaming chunks, system messages and tool calls add nothing. |
| `bot_messages` | Newly accepted mailbox messages (`MailPosted`), plus legacy room messages while the mailbox is off. Duplicate mail ids add nothing. |
| `bot_wakes` | Committed `BotWoke` events, including routine and automatic wakeups. |
| `input_tokens` | Known, uncached input tokens in accepted provider turn reports. |
| `cache_read_tokens` | Known cache-read input tokens in those reports. |
| `cache_write_tokens` | Known cache-write input tokens in those reports. |
| `output_tokens` | Known output tokens, including reasoning already included by the provider. |
| `usage_turns` | Turns with known output-token usage; unknown provider usage is not estimated. |
| `boards_created` | Newly added local KiCad project/PCB families observed after a Backplane tool call. A matching `.kicad_pro`/`.kicad_pcb` pair counts once; a later counterpart adds nothing. Existing baseline files, edits, deletions and detected renames add nothing. |
| `boards_ordered` | Newly completed non-pilot Buy-it checkouts containing at least one board-kind part. One per order, regardless of quantity or number of designs. Quotes, unpaid/cancelled/refunded orders, mechanical-only orders and already-paid upgrade history add nothing. |

The original ten fields remain required by the receiver. The two board fields
are optional for older clients and default to zero. New clients send all twelve
fixed integer counters. Neither arbitrary field names nor design-derived enum
keys are accepted. No quantities or kind breakdowns are part of this extension.
No names, paths, design hashes, dimensions, per-design layer counts, part
numbers, BOMs, netlists or prices are transmitted. Only the existing install,
package/platform/day metadata surrounds the numerical counts.

Creation is detected from the same local design-history snapshot Backplane
already takes, with rename detection. Opening/importing a project as an initial
baseline adds nothing. A completed checkout is first observed as `paid` (or a
later successful state when polling missed `paid`). Local once-only source
markers are durable and never enter the usage wire; opting out/reset clears
counters and leaves earlier observations ineligible for backfill. A design job
also carries the privacy generation captured before it starts, so stale work
cannot count into a new sharing session.

Total model tokens are input + cache read + cache write + output. Reasoning
is a subset of output and is not added again. Costs, provider/model names,
thread/bot identifiers, message sizes and contents never enter this event.
Message categories can overlap: bot mail may also become a user-role thread
message. They must not be summed into a count of human-authored messages.
Mirrored threads from other hubs (`~` ids) are excluded. Bot traffic does not
create daily-active-human events.

Each counter is cumulative **per install id and UTC day**. The backend must
UPSERT component-wise MAX for that key, then sum those maxima for reports.
Summing incoming snapshots would inflate totals on retries. Identified usage
requires a verified install; the worker sends registration first. Legacy
telemetry travels in a separate request so an older receiver's usage schema
rejection does not discard install/update/activity events. Deploy the receiver
before enabling these alpha packages.

The hub projects seven days of counters from durable committed events, in private state outside the shared settings and sidebar memo inputs, with
incremental `obj.usage` records written in the same transaction. It never scans
conversation history to count. Reset id, changing sharing, or revoking the
expanded answer erases the current counters; re-enabling starts fresh. A
privacy-generation check immediately before HTTP rejects stale usage workers,
including off/on changes in a single append. An already in-flight request can
finish, as with the original telemetry pipeline.
That generation also participates in the local identity-reset marker, so an
off/on sequence between worker ticks rotates the id instead of attributing
fresh counters to a previous consenting session's daily maximum.

`<home>/telemetry/usage.json` holds at most seven acknowledged, id-scoped
snapshots, in the private telemetry directory with atomic mode-0600 writes.
Unchanged snapshots send nothing. Failed/ambiguous delivery keeps the snapshot
eligible with the existing one-hour/Retry-After backoff; an accepted or
permanently rejected snapshot is suppressed until its counters change. A crash
before acknowledgement persistence only causes an idempotent resend. Snapshots
older than seven days expire, so extended offline periods are undercounts.
Opt-out/unanswered usage removes this acknowledgement file on the next worker.
Environment kill switches stop all telemetry workers and state writes.

## Development checks

`test/telemetry_usage_test` covers durable daily counts, known/unknown
token usage, duplicate reports/mail, mirrored threads, privacy clearing,
worker invalidation, seven-day expiry, wire fields and incremental-store
recovery. `test/telemetry_test` covers the planner, queue, allowlist, wire bytes,
HMAC and PoW vectors and shared client actions. The telemetry laws are appended
to LAWS and proven in PROOF; existing laws are unchanged.
`bun test/tools/telemetry_e2e.ts [BINARY] [WIREDIR]` uses a generated dev key,
a random local HTTP port and temporary hub homes. It exercises signatures,
PoW, opt-in/out, activity, backoff, rejection and closed gates. It never reads
production credentials. Outputs are saved to `build/telemetry-report/`.

The alpha receiver implementation is pinned for paired testing at backend
commit `be77491e1dd8a5e6e51b198c03bc24d7492bb034`. Build the small worker with
then `bun build --compile build/usage-delivery.js --outfile build/usage-delivery`.
Run `test/tools/telemetry_usage_e2e.py` with the backend's `uv` environment,
`--backend` pointing to a clean checkout, `--revision` its exact full commit,
`--worker build/usage-delivery`, and `--receipt build/usage-integration.json`.
It starts a real receiver on loopback with generated development credentials
and a temporary database; the compiled worker sends and signs its actual
projection. It tests registration, retries and ambiguous responses, reordered
snapshots, reset, opt-out, old consent, kill switches and investor totals. The
receipt records source/backend/worker hashes and counts without identifiers
or credentials. The product hub's release build script has additional hub
entrypoint checks and is not used for this standalone worker.

Only in explicit loopback integration-test mode,
`BACKPLANE_TELEMETRY_DAY_SECS=N` shortens the day. It alternates between
today's UTC date and yesterday every N seconds, within the real server's
allowed date window. This hook is never baked. `BACKPLANE_WAKE_TICK_MS=500`
makes temporary hubs tick quickly. The protocol has no client-kind field.

For actual signed local-server integration, set `TM_SERVER_ROOT` to a clean,
pinned Backplane.Works checkout and `TM_SERVER_HEAD` to its full commit.
The test launches the existing Flask app with `uv` (or
`TM_SERVER_PYTHON`), generated development credentials and temporary database
paths. The native hub itself signs and sends; assertions inspect actual
server SQLite rows for verified installs, updates, activity and identifier-free
opted-out updates. The test records backend/source and binary hashes with
counts, without retaining credentials. Platform package ingestion is a
separate release gate; passing metadata tests or setting Actions secrets is
not evidence that a distributed package sends successfully.
