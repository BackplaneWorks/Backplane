# Anonymous usage

Backplane counts installs and how many people use it. Only the hub sends:
the desktop window, iOS and Android never contact the telemetry service.

Alpha also sends aggregate daily counts of threads created, messages, model
token usage, bot traffic and numeric board creation/completed-order counts
for product and investor reporting. This is usage measurement, not token
billing. API keys, authentication tokens, conversation content and
credentials are never part of these counters.

## What is sent

Nothing else is ever put in a request.

- **install**: a random UUIDv4 install id, app version, channel (`release` or
  `nightly`), OS (`linux`, `darwin`, `windows`), architecture (`x64`,
  `arm64`), UTC date, and the id's proof of work.
- **update**: the same, plus the previous version.
- **active**: the same identity and platform fields. It counts the UTC day of
  a successful authenticated allowlisted UI action, at most once per id per
  day. The allowlist is `turn.start`, `thread.create`, `bots.say`,
  `bots.post`, `project.new`, `project.add` and `thread.fork`. Idle
  rollover, reconnects, keepalives, automatic view refreshes, provider work
  and background work do not count. This is a UI interaction signal, not
  hardware attestation.
- **usage**: the same identity and platform fields, plus twelve fixed
  integer counters for one UTC day (see the alpha aggregate contract below).

There are no app-kind counts, usage times, crash uploads, projects, files,
prompts, names, paths, addresses, hardware identifiers or time zones in these
events. A request carries a send timestamp and a random nonce for
authentication and replay protection. The server does not keep the timestamp
as usage data.

The wire format is v1:
`{"v":1,"ts":<unix s>,"nonce":<16 hex>,"events":[…]}`.
It is signed with HMAC-SHA256 over `ts + "." + body` and sent with these
headers:

- `X-Backplane-Key: <key id>`
- `X-Backplane-Timestamp: <ts>`
- `X-Backplane-Signature: sha256=<hex>`

Each event's keys are exactly `e`, `day`, `id`, `share` (always `true`),
`ver`, `prev` (updates only), `ch`, `os`, `arch`, `pow` (installs only) and
`counts` (usage only). This is the format the 0.14 hub sent, unchanged.

## Consent

Telemetry is **opt-out**, but nothing leaves the hub before the person could
have seen what is sent.

- **New installs.** Setup's first step has a telemetry row that says exactly
  what is sent, with **On** preselected. Pressing **Off** there turns all
  telemetry off. Finishing setup with it on (Done, End setup or Open the
  uberbot) counts as the choice: the hub records `telemetry.notice = setup`
  and only then starts sending. Finish later, closing the app, or a setup
  that never finishes sends nothing.
- **Existing installs** (setup finished, or the home has projects from
  before setup): after updating to a release with the sender, every client
  shows a one-time notice. It says what is sent, that telemetry is on, and
  offers a one-tap **Turn off**, a **Settings** link (Privacy) and a close.
  Telemetry starts once the notice has been drawn on any client: that
  client sends `telemetry.notice.act {action: shown}` and the hub records
  `telemetry.notice = shown`. It stays up until it is closed or answered,
  and a closed notice never returns. A build that cannot send shows no
  notice.
- **Off stays Off.** An explicit Off is final against upgrades, the notice
  and setup. That covers `telemetry.share = off`, the expanded answer
  `telemetry.usage.asked = 0`, and an earlier setup decline
  `onboard.usage = off`. Only the person's own later **On** changes it, in
  Settings, Privacy or setup's row, through the token-bound
  `telemetry.choice`. An existing install with an Off never sees the notice
  and sends nothing.
- Pressing **On** in Settings beside the full list also counts as having seen
  it (`telemetry.notice = chosen`).
- **Reset id** rotates the identity only. The next pass makes a fresh id; the
  old id's queue, acknowledgements and counters are dropped and the new id
  registers again. It never enables telemetry or changes the choice.

Settings, Privacy on every client has **Share anonymous usage** with these
controls:

- On and Off.
- Reset id.
- Privacy and Report an issue.
- The install id, as copyable text, from the info `telemetry.id`. It is sent
  only to authenticated clients, never enters the event log, and is empty when
  there is no id or telemetry is off.

`setting.set` cannot write `telemetry.notice` or re-enable telemetry. Older
clients can still turn it off or reset the id. A choice's decision ids,
receipts and revision tokens work as before, so a stale On from another
client cannot overwrite a newer Off.

Opting out, and any state where sending is not allowed, deletes the id, the
outbox, the acknowledgements and the counters (`<home>/telemetry/*`). Nothing
is backfilled when telemetry is turned on again. Committing an Off or a reset
deletes them at once, so an Off and an On between two passes still start a
new identity. These are product preference records, not a statement about
legal consent.

## Sender

`native/host/src/settings/telemetry_send.rs` sends. It runs one pass a minute
and at once after a choice, a notice change or the day's first UI action.
One pass runs at a time. Each pass does this:

1. Reads the choice. If sending is not allowed, it deletes the telemetry
   files and stops.
2. Plans: a new id if there is none or the reset marker moved, an update if
   the version changed, and an active event for a day with a UI action.
3. Solves the install's 18-bit SHA-256 proof of work (`bp-pow1:<id>:<hex>`)
   off the async runtime.
4. Saves the outbox.
5. Asks the hub again right before each HTTP request whether sending is still
   allowed and the id is still current. An Off or a reset made meanwhile wins.
6. Sends up to 50 ready events in at most 16 KiB.
7. Sends the usage snapshots, separately and only once the install is
   registered. An older receiver's rejection of usage then never costs the
   install, update and activity events.

**Outbox.** `<home>/telemetry/state.json` is in a 0700 directory with mode
0600, and is written by atomic rename. It holds the id, the queue and the
retry time, and survives restarts, so offline events are sent when the network
comes back. The field names are the 0.14 state file's, so an old file still
reads; anonymous or unanswered entries from earlier versions are dropped,
never released.

The queue is de-duplicated: one install per id, one update per version, one
active per day. It expires after seven days and caps at 200 events, dropping
the oldest activity first. Only HTTP 202 or a permanent 400/401/403/409/413
consumes a batch. Anything else, including no answer, keeps it and waits at
least an hour, or longer if a numeric `Retry-After` says so. The receiver also
de-duplicates installs, updates and actives by id, so a resend after a crash
counts once.

**HTTP.** The hub runs `curl --disable` with a private config and body file
in the telemetry directory, removed after each attempt. Nothing is on its
command line. It is limited to HTTPS for the release endpoint, with a 10 s
timeout and a 16 KiB response limit. A hub on a copy of a log
(`BACKPLANE_COPY`) starts no sender.

## Endpoint, key and builds

Release builds talk only to `https://backplaneworks.com/api/v1/events`. The
release workflow (`.github/workflows/release.yml`, build step) compiles the
endpoint and the key in, on tag pushes only, using `option_env!`:

| Compile-time variable | Source |
| --- | --- |
| `BACKPLANE_TELEMETRY_URL` | the literal endpoint above |
| `BACKPLANE_TELEMETRY_KID` | Actions secret `BACKPLANE_TELEMETRY_KID` (`rel-…`) |
| `BACKPLANE_TELEMETRY_KEY` | Actions secret `BACKPLANE_TELEMETRY_KEY` (64 lowercase hex) |

A missing secret builds a hub that sends nothing, with a warning. A malformed
one fails the build without printing the value. Dry runs (workflow_dispatch)
and every developer build bake nothing, so they can never send, whatever their
environment says. The hub also checks the configuration at start (the
canonical endpoint, a `rel-` key id, 64 hex digits); anything else closes the
gate. The channel comes from the package version: `-nightly.` means
`nightly`, otherwise `release`.

Normal runtime environment variables cannot replace the baked values or
redirect traffic. Only `BACKPLANE_TELEMETRY_TEST=1` accepts
`BACKPLANE_TELEMETRY_URL`, `_KID`, `_KEY` and `_CHANNEL` from the
environment, and only for a literal `http://127.0.0.1:PORT/api/v1/events`.
`BACKPLANE_TELEMETRY_TICK_MS` shortens the pass interval only in that mode.

**Kill switches** disable all telemetry, including state writes, and win over
everything:

- `BACKPLANE_TELEMETRY=0` (or `off`)
- `DO_NOT_TRACK=1`
- `CI` set
- `BACKPLANE_SANDBOX` nonempty except `0`
- `--sandbox`

Open-source clients cannot keep a secret: the key is in the binary. The HMAC
raises the cost of garbage submissions. The receiver's real controls are
schema validation, per-key version and channel allowlists, replay protection
(nonces), rate limits, de-duplication and the proof of work. See
[Privacy](https://backplane.works/privacy) for server retention and deletion.

Report an issue opens a GitHub issue form. Its URL contains only the
validated version, OS and architecture, and an empty What happened prompt.

## Alpha aggregate contract

The additive wire-v1 event is `usage`. It has the same required `day`, `id`,
`share`, `ver`, `ch`, `os` and `arch` metadata as identified activity, plus a
`counts` object with these fixed nonnegative integer fields (maximum
281474976710655 each):

| Field | Counts |
| --- | --- |
| `threads_created` | Committed `ThreadCreated` events, including child threads and forks. |
| `user_messages` | Committed user-role `MessagePosted` events, including programmatic messages in bot threads. |
| `assistant_messages` | Committed assistant-role `MessagePosted` events; streaming text, system messages and tool calls add nothing. |
| `bot_messages` | Committed mailbox messages (`MailPosted`); a repeated mail id adds nothing. |
| `bot_wakes` | Committed `BotWoke` events, including routine and automatic wakeups. |
| `input_tokens` | Known, uncached input tokens in `TurnUsage` reports. |
| `cache_read_tokens` | Known cache-read input tokens in those reports. |
| `cache_write_tokens` | Known cache-write input tokens in those reports. |
| `output_tokens` | Known output tokens, including reasoning already included by the provider. |
| `usage_turns` | Turns with known output-token usage; unknown provider usage is not estimated. |
| `boards_created` | KiCad project/PCB families (a `.kicad_pro`/`.kicad_pcb` pair counts once) that first appear in a design commit Backplane records at a turn's end, with no file of that family in the previous commit. A later counterpart, edits and deletions add nothing; a commit that removes families as well counts the difference (a rename adds nothing); a repository's first commit is a baseline and adds nothing. |
| `boards_ordered` | Buy-it orders whose manifest holds a board kind (pcb, flex, hdi, aluminum, copper_core, rogers, ptfe), counted once, the first time the order is seen `paid` or later. Pilot test payments, quotes, unpaid, expired and mechanical-only orders add nothing. |

The receiver requires the original ten fields; the two board fields are
optional there and default to zero. This client sends all twelve. Field names
are fixed: no names, paths, design hashes, dimensions, layer counts, part
numbers, BOMs, netlists, prices, costs, provider or model names, thread or bot
identifiers, message sizes or contents.

Total model tokens are input + cache read + cache write + output. Reasoning
is a subset of output and is not added again. Message categories can overlap:
bot mail may also become a user-role thread message. They must not be summed
into a count of human-authored messages. Mirrored threads from other hubs
(`~` ids) are excluded. Bot traffic does not create daily-active events.

`native/host/src/settings/telemetry_usage.rs` counts events as they commit,
and only while sending is allowed. It never reads conversations back. A
privacy change stops counting until the next pass has checked the choice
again. An Off or a Reset id erases the counters, and turning it on again
starts from zero.

The counters are kept for seven UTC days, today included, in
`<home>/telemetry/counts.json`. That file is private, written atomically on
each pass, and scoped to the id's reset marker. A crash between passes loses
at most that minute's counts, an undercount.

Each counter is cumulative **per install id and UTC day**. The receiver
UPSERTs component-wise MAX for that key. `<home>/telemetry/usage.json` holds
the acknowledged snapshots, so an unchanged day sends nothing. A failed or
ambiguous delivery keeps the snapshot eligible with the same backoff. A crash
before the acknowledgement is saved only causes an idempotent resend.

## Development checks

`cargo test` in `native/host` covers the wire and the consent paths:

- `settings::telemetry` covers the choice, the gate and the notice.
- `settings::telemetry_send` covers the wire fields and order, the envelope,
  the HMAC, the proof of work, the validators and the planner. The planner
  tests check at most one active per day, the reset rotating only the id, the
  backoff and consumed statuses, expiry and the cap, batch limits and reading
  a 0.14 state file.
- Its end-to-end tests start a hub on a temporary home and a loopback
  receiver that checks the signature as the real one does. They cover each
  consent path:
  - a new install that keeps it on;
  - a new install that turns it off in setup;
  - an existing install upgraded with no answer (the notice shows, then it
    sends);
  - an existing install with an explicit Off, or an earlier decline (it stays
    off and sends nothing).
- They also cover the notice's Turn off, outbox replay across a restart, and
  that no content, names, paths or keys ever appear in a request.

The same tests in `canonical` and `order` cover the board counts. The desktop's
`screens::tmnotice` and the phones' notice tests cover the client side.

The receiver is the Backplane.Works app (`telemetry.py`, `POST
/api/v1/events`). Its accepted format is the 0.14 wire v1 plus the optional
board counters. Production keys live in its `TELEMETRY_KEYS`: each key id has
a secret, the allowed channels and the allowed versions. A release whose
version the key does not allow is answered `403` and its batch dropped, so
the key's `versions` must cover the release (for example `0.15.*` or `*`).
