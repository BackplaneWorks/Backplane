# Parity harness (M0)

Records real client sessions against a hub and replays them against any hub on a port, comparing frames. Renderer `mobile` only (phones and GPUI); the web page is not a target.
Fixtures and stand-in agents only; every hub is checked from `/proc/<pid>/environ` before its first turn (`test/gpui/slim/check_agents.sh`).

## Scenarios (`scenarios.mjs`)
| name | covers |
|---|---|
| join | `GET /hello`, raw WebSocket join (log, live, info, changes), ping |
| badauth | refused upgrades (no token, wrong token, bad characters, empty), junk binary frame, unknown method, `view.open` without a session, unknown HTTP path |
| session:mobile | `renderer: mobile` view: open, update, project add, select, draft, send, a real stand-in turn with streamed deltas, paged history (`view.page`) |
| session:gpui | the same as the GPUI client asks (`nativeSidebar`) |
| session:gpuid | the same, offering the keyed view delta (`viewDelta {version, token}`); ops are applied with the client's own `src/web/view-delta.js`, and the rebuilt body is checked equal to a whole view |
| settings | the Settings requests: setting.set (and its refusals), telemetry.choice, the secret box with a planted value (no frame may hold it), zone.report/set, diagnostics, models.refresh, update.check, app.update, improvements, accounts.default |
| reconnect | resume from the log position and origin the client already has |

## Run
```
bun test/gpui/parity/record.mjs --bin BIN --out DIR --port N --tag A     # one fresh fixture hub per scenario
bun test/gpui/parity/compare.mjs DIR_A DIR_B                              # two recordings
bun test/gpui/parity/replay.mjs --rec DIR --bin BIN --port N              # replay a recording against a hub
sh test/gpui/parity/boat-run.sh all                                      # on Boat, via boat-stage.sh / scripts/boat-job.sh
```
A recording keeps every frame as raw bytes (base64) and as decoded JSON. Replay sends the recorded requests at their recorded offsets; server-issued values
(generation, epoch, view ids, ids the hub made, hub URL, work paths, delta tokens, and the commands the hub issued) come from the target hub.
Polls the recording made while waiting for a turn are repeated until the same observable (thread state, entry count, thread count) is reached.

## Normalisations (compare.mjs `NORMALISATIONS`)
- N1 clocks: numeric fields at, rid, now, clock, last, since-time keys -> <T>; 10-digit (17xxxxxxxx/18xxxxxxxx) and 13-digit epoch numbers inside strings -> <T>; hh:mm and durations (e.g. "Finished 18:26 · 2s") -> <time>. Reason: wall clock.
- N16 housekeeping events read the id table but never add to it, so their number and position cannot shift the ordinals of other events (an id first seen only there is <id>).
- N19 origin in a query string (the since/origin of a reconnect): the value is replaced by its form only (`<origin:decimal>`, `<origin:hex>`, `<origin:empty>`, `<origin:other>`), not dropped. Bend's origin is the decimal `at` of the log's first change; the Rust hub's is a hex hash, which `parity-edge` reports as a real difference (a client resuming across hubs reads it). So reconnect shows 1 difference until `origin_of` lands in the Rust hub; do not count reconnect as 0 before then.
- N17 time zone: every hub the harness starts (recording and replay) gets `TZ=Etc/UTC` (hubs.mjs). Reason: the hub logs its machine time zone as `tz.machine` / `tz.rule.<zone>` at start; without a pin the JS baseline (recorded on a host in Etc/UTC) and a hub on a UTC host differ by environment, not behaviour. Recordings made before this pin already carry Etc/UTC. `replay.mjs --all` prints every difference instead of the first 12.
- N13 ordinals are per client: each client numbers the ids it sees by first appearance in its own stream, so what another client saw first cannot shift them.
- N14 delta tokens: viewDelta token and base strings (the hub revision a body was kept under) -> ordinals, like ids; an empty base (a whole reply) stays empty.
- N2 ids: p/t/g/m-ids with a 10-digit time part (p1793834744-7, t..., g..., m...), stand-in session/message ids (fake-<pid>, m<pid>-n-n, tu<pid>-..), view ids (<gen>:<n>), and the greeting generation and epoch -> ordinals by first appearance per recording (ID1, ID2...). Reason: ids embed time, pids and counters.
- N3 viewRevision -> <R> (and checked non-decreasing per client). Reason: counts hub-internal changes, which coalesce with timing.
- N4 delta frames: consecutive delta frames of one thread joined into one item with "@<ms>@" stamps stripped; frame boundaries are not compared. Reason: coalescing follows read timing (200 ms pace).
- N5 changes frames: expanded to one item per event; batch boundaries are not compared. Reason: batching follows timing.
- N12 late volatile replies: volatile means a poll request and its reply, matched by client and request id, whenever the reply arrives; every other inbound frame is kept even if it arrived during a poll window (the recorder flags by arrival time, which is not a property of the frame).
- N6 dropped: hb frames, view.changed frames (their number and position follow timing; each reply carries the revision it saw), volatile frames (polls made while waiting for a turn). Reason: timing-driven counts.
- N11 harness steps: `settle` and `end` pseudo-frames (the client program waiting or closing) are control, not wire traffic, and are not compared. Reason: a replay waits by reply id, not by the recorded quiet times.
- N7 streams: per client the sequence of events, deltas, replies and view replies are compared as separate ordered streams; relative order between streams is not compared. Reason: network timing.
- N9 info frames: all info frames of a client are merged into one item (keys unioned, later values win; environment keys dropped); position and repeat count are not compared. The `other` stream (http, upgrade, close, info) is compared as a sorted set. Reason: the hub repeats info on its own timer.
- N20 multisets: hk and logInfoKeys compared with their real counts (a doubled event is a difference). N22: liveness.* and autosettle.quiet.* by presence of the key only (not values, not how many ticks). N24 guard: a stream sent as one frame where the other streams it is a difference (`streamed`). N26: keys that normalise alike are reported as a collision, never merged. Self-test: `bun test/gpui/parity/compare_selftest.mjs` (fixtures that must differ); compareDirs runs it first in every comparison and stops (exit 3) when it fails.
- N27 (TEMPORARY): in a room post by the person, `cat:""` in B (Rust) equals no `cat` in A (JS baseline); one direction, `bot.posts[*]` only; bot authors and any other value still differ. Reason: the baseline (post-#960 main c4606b5a9c) predates 166a115d99 / 43472d0cfd. Removal: when the baseline is rebuilt from a tip that has 166a115d99; compareDirs refuses N27 on a baseline whose `BASELINE` file names a commit containing 166a115d99. The baseline is also behind in other shared Bend sources (view.update's draft 726ab64438 / 0d930e939e, window 4c1f57bef6, phones' routing 9ec0943e5e), so a parity 0 against it is not yet proof against today's product: rebuilding it from the gpui tip is a gate item.
- N10 housekeeping events: a contiguous run of events that concurrent hub jobs append (SettingSet keys machine.id, accounts.*, allowance.*, projects.missing, tz.*, autosettle.*, liveness.*, turn.from/speed.*, restart.*, provider.*, fallback.*, tok.*, summary.*, told.*, alert.*, account.*, star.*; ActivityLogged of kind git or spawn; WorktreeSkipped) is taken out of the event stream and compared as a multiset (canonical order, real counts, except liveness.* by presence: N20, N22; no order, and not their position among the other events); every other event stays in order. Reason: start-up, git probes and settle/liveness bookkeeping finish in varying order.
- N15 counters: numeric seq and changes (the hub change count at the instant a view was drawn) -> <N>. Reason: housekeeping events land between draws.
- N8 info frames: home paths, ports, machine ids, tokens, pids -> placeholders. Reason: environment.

## Gate (2026-11-05, Boat large sandbox, hub binary build/gpui/backplane-serve 0.14.0)
Self-diff (two recordings of the JS hub): 0 differences in 6 scenarios. Replay of recording A against the JS hub: 0 differences in 6 scenarios.
Raw: build/gpui/results/parity/ (self-diff.txt, replay-diff.txt, recA/, recB/, replayA/). Harness wall time on Boat: 173 s.
Not covered: web renderer (out of scope), phone-app traffic as such (the mobile renderer is exercised by two profiles), segmented replies beyond `view.page`.

## Gate against the current JS hub (2026-11-05)
Hub: build/gpui/backplane-serve-current (src/ at the base commit 7c9b3c223c, built on Boat with the composite compiler; see its README).
Self-diff (two recordings): 0 differences in 6 scenarios. Replay of recording A against a fresh instance: 0 differences in 6 scenarios. 179 s on Boat.
Raw: build/gpui/results/parity-current/. This recording (recA there) is the baseline for the Rust hub; the earlier one (results/parity) came from an older JS build.

Rust hubs: pass the presenter as flags in `PARITY_HUB_ARGS` (space separated), e.g. `--policy-bin build/gpui/slim/bp-policy-page --window-bin build/gpui/hub/new/window --commit <sha>`; without them the hub answers with its canned presenter and a replay differs in shown/window/overlay and view ids. Check `/proc/<pid>/cmdline` before trusting a run. The JS hub takes no such flags (leave it unset).

Counts before commit `parity: remove the 40-difference cap` were capped at about 42 per scenario, so totals such as 156, 177, 151 and 127 were floors, not true counts. A result of 0 was always exact.

## Scenario `private` (private lines)

The fixture log holds DeviceRegistered/DeviceDropped (with tokens) and ride.*, launch.input.*, fallback.input.* values, all with `PRIV...` sentinels. A raw full join, a resumed join (`since=3`), two live `setting.set` writes and a gpui view run against it. `privateLeaks()` (lib.mjs) searches the raw bytes of every inbound frame for a sentinel: the JS recording fails if one is found, and a replay prints "PRIVATE LINES REACHED A CLIENT" and counts each frame. Run on both hubs with `sh test/gpui/parity/boat-private.sh` (staged JS hub, Rust hub, policy and window) or `boat-private-build.sh` (builds the Rust hub from the tree first). It is in record.mjs's scenario list, so the full parity run covers it; keep it there. A comparison with no recordings or an `--only` that matches nothing exits 2: an empty run proves nothing.
