# Anonymous usage

Backplane counts installs and how many people use it. Only the hub sends:
window, web, iOS and Android never contact the telemetry service.

Alpha also sends aggregate daily counts of threads created, messages, model
token usage and bot traffic for product and investor reporting. This is usage
measurement, not token billing. API keys, authentication tokens, conversation
content and credentials are never part of these counters.

An identified install sends a random UUIDv4, app version, channel, OS,
architecture and UTC date. Updates optionally include the preceding version.
A daily active event counts the UTC day of a successful authenticated allowlisted UI action, at most once per id/day. Idle rollover, reconnect, keepalive, automatic view refresh and provider or background work do not count. This is a UI interaction signal, not hardware attestation. Committed sharing, consent or reset changes discard pending action evidence even before an install id exists.
There are no app-kind counts, usage times, crash uploads, projects, files,
prompts, names, addresses, hardware identifiers or time zones in these events.
The request includes a send timestamp and random nonce for authentication and
replay protection; the server does not retain that timestamp as usage data.

The tour asks once, before the uberbot step. Existing installs see that step
alone, once their hub's state and version have arrived. Keep sharing, Opt out,
Next and Skip answer it. Opening Privacy policy or Report an issue leaves the
step open. Unanswered installs and updates remain local for three days, then
leave as anonymous counts without an id. Daily activity requires an answer.

Existing opt-ins see the expanded alpha disclosure once; the new counters
require its separate `telemetry.usage.asked=1` answer. Existing opt-outs stay
off without another prompt. The old install/update/activity protocol and
consent continue to work. Developer builds with no baked configuration never
collect the new counters. No pre-alpha or pre-consent history is backfilled.

Settings on every client has Share anonymous usage, On/Off, Reset id and
Privacy. Opting out clears the id and queued activity; only id-less install
and update counts remain. Re-enabling or Reset id makes a fresh random id and
registers it with a proof of work. The id is stored in
`<home>/telemetry/state.json`, mode 0600, in a 0700 directory, and sent only
to authenticated clients as info `telemetry.id` for the copyable Install id
text in Settings. It never enters the event log. The info is empty when
there is no id or the person opted out. State writes use an atomic rename.
The queue expires after seven days, caps at 200 events (old activity first),
and sends at most 50 events / 16 KiB. A failed request retries no sooner than
one hour, including a longer numeric Retry-After. Schema/auth/replay rejection
drops its batch. Only HTTP 202 or permanent 400/401/403/409/413 responses consume a submitted batch; other statuses retain it with backoff. Anonymous ambiguous retries can count twice because they have no deduplication identifier. One background job runs at a time and HTTP times out at 10s.

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

Before starting HTTP, the worker asks the hub whether its sharing, consent
and reset snapshot is still current. A choice made during proof-of-work
invalidates that worker's pending request. A request already in flight can
finish; opt-out clears the visible id immediately and the next plan removes
queued identified data.

Open-source clients cannot keep a secret. HMAC-SHA256 over timestamp and raw
body raises the cost of garbage submissions; server schema validation,
version/channel allowlists, replay protection, rate limits and deduplication
are the real controls. An identified install uses 18-bit SHA-256 proof of work
in a disposable native child (`--telemetry-pow UUID`); the pure Bend verifier
independently checks the result. This is not identity authentication.
See [Privacy](https://backplane.works/privacy) for server retention/deletion.

Report an issue opens a GitHub issue form. Its URL contains only validated
version, OS and architecture and an empty What happened prompt. Reporting is
an explicit link, never an automatic upload. The tour reminds people that
Backplane is in beta.

## Alpha aggregate contract

The additive wire-v1 event is `usage`. It has the same required `day`, `id`,
`share`, `ver`, `ch`, `os` and `arch` metadata as identified activity, plus a
`counts` object with exactly these nonnegative integer fields (maximum
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

`test/telemetry_usage_test.bend` covers durable daily counts, known/unknown
token usage, duplicate reports/mail, mirrored threads, privacy clearing,
worker invalidation, seven-day expiry, wire fields and incremental-store
recovery. `test/telemetry_test.bend` covers the planner, queue, allowlist, wire bytes,
HMAC and PoW vectors and shared client actions. The telemetry laws are appended
to LAWS.bend and proven in PROOF.bend; existing laws are unchanged.
`bun test/tools/telemetry_e2e.ts [BINARY] [WIREDIR]` uses a generated dev key,
a random local HTTP port and temporary hub homes. It exercises signatures,
PoW, opt-in/out, activity, backoff, rejection and closed gates. It never reads
production credentials. Outputs are saved to `build/telemetry-report/`.

The alpha receiver implementation is pinned for paired testing at backend
commit `be77491e1dd8a5e6e51b198c03bc24d7492bb034`. Build the small worker with
`bend test/native/telemetry_usage_delivery.bend -o build/usage-delivery.js`
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
