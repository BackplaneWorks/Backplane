# Anonymous usage

Backplane counts installs and how many people use it. Only the hub sends:
window, web, iOS and Android never contact the telemetry service.

An identified install sends a random UUIDv4, app version, channel, OS,
architecture and UTC date. Updates optionally include the preceding version.
A daily active event counts the preceding UTC day, at most once per id/day.
There are no app-kind counts, usage times, crash uploads, projects, files,
prompts, names, addresses, hardware identifiers or time zones in these events.
The request includes a send timestamp and random nonce for authentication and
replay protection; the server does not retain that timestamp as usage data.

The tour asks once, before the uberbot step. Existing installs see that step
alone, once their hub's state and version have arrived. Keep sharing, Opt out,
Next and Skip answer it. Opening Privacy policy or Report an issue leaves the
step open. Unanswered installs and updates remain local for three days, then
leave as anonymous counts without an id. Daily activity requires an answer.

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
drops its batch. One background job runs at a time and HTTP times out at 10s.

Setting `BACKPLANE_TELEMETRY=0` (or `off`) or `DO_NOT_TRACK=1` disables all
telemetry, including anonymous counts and state writes. `CI` set,
`BACKPLANE_SANDBOX` nonempty except `0`, and `--sandbox` also disable it.
Missing endpoint, key id or key closes the gate. Developer builds bake no
configuration, regardless of developer environment variables or `.env`.

Release compilation explicitly sets `BACKPLANE_TELEMETRY_BAKE=1`,
`BACKPLANE_VERSION` (the full package version), and the endpoint/key id/key.
`scripts/telemetry-header.py` writes a private compile-only header, after C
emission. Linux/macOS use it through `build-app.sh`; Windows passes it as a
separately quoted `-include` argument in `build-windows.sh`. It never enters
emitted C, emitted-source uploads, object caches or package files. The
header is removed when compilation ends. Missing configuration bakes empty
values; malformed configuration fails the build without printing values.

`BACKPLANE_TELEMETRY_CHANNEL` is immutable build metadata: `release` for a
stable version, `nightly` for `-nightly.`. The helper derives it from the
stamped/package version and rejects an explicit disagreement. If both
`BACKPLANE_VERSION` and `BACKPLANE_VERSION_STAMP` are supplied, their full
versions must agree after removing the leading `v`. Changing the user's
update preference never relabels telemetry from an installed package.

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

## Development checks

`test/telemetry_test.bend` covers the planner, queue, allowlist, wire bytes,
HMAC and PoW vectors and shared client actions. The telemetry laws are appended
to LAWS.bend and proven in PROOF.bend; existing laws are unchanged.
`bun test/tools/telemetry_e2e.ts [BINARY] [WIREDIR]` uses a generated dev key,
a random local HTTP port and temporary hub homes. It exercises signatures,
PoW, opt-in/out, activity, backoff, rejection and closed gates. It never reads
production credentials. Outputs are saved to `build/telemetry-report/`.

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
