# Parity gate by cause

Owner: slim. Baseline since 2026-11-05: the post-#960 JS hub (build/gpui/backplane-serve-960), recorded twice on Boat with TZ=Etc/UTC, self-diff 0
(build/gpui/results/parity-960s/recA, recB). The old parity-current/recA lacked `cad.install` and is retired as a baseline. Replay: `REC=<dir>
VERSION=0.1.0-dev sh test/gpui/parity/boat-rust-replay-launch.sh` (builds the hub from the tree, presenter flags, Boat). Harness normalisations N1-N22 in
test/gpui/parity/README.md; arrays of different length are aligned by key (N21) and `hk`/`logInfoKeys` are sets (N20), so a count is differences, not shifted positions.

## Now: 405 lines, 4 causes (hub from the tree @ d? 2026-11-05 07:44, vs parity-960s/recA)

| Cause | Lines | Independent | Owner | What it is |
|---|---:|---:|---|---|
| C picker.models | 312 | 1 | agents (model catalog) | JS lists 5 Claude models (the default, `Fable 5.1 · claude-fable-5-1`, Opus 5.5, Sonnet 5, Haiku 4.5); Rust lists `default`. Repeated in every view, x2 for shown and overlay. |
| B housekeeping (raw `hk`) | 36 | 12 events | state (pure) + core (runs them) | JS appends at a turn's end `alert.<t>`, `autosettle.{audience,output x4,quiet,read}.<t>`, `star.nudge.v1`, `summary.at.<t>` (x2), `told.<id>`; Rust none of them. |
| D session scratch keys | 30 | 10 keys | parity-session | `drafts.@*` and `info.view.*` still present on one side only. |
| H raw deltas | 27 | 1 | agents | JS streams delta/delta_end frames in the session; the Rust hub sends none of them (stand-in turn). |
| Total | 405 | about 24 | | badauth, join, reconnect, unresolved: identical. |

## History

1621 lines at gpui 0688158f6 on the old baseline: logInfoKeys 208 (one key; the baseline was the anomaly: closed), picker 312, scratch keys 246 -> 30,
gpuid page 684 (one cause, fixed since), housekeeping 126 -> 36, deltas 21 -> 27.

## Session lane: the 22 at b7802aac97, classified (UNSETTLED: the reviewer has not signed this off)

Evidence: `build/gpui/results/parity-gate-b7802aac97/` (job j20261105150756-346184, comparator at a955d2e9c3). Not yet rerun with the comparator
2ac7925275 (N20 real counts, N22 liveness and autosettle.quiet by key, N24 `streamed` guard, N26 collisions, self-test first); mechanics reruns all scenarios.

| Lines | What | Class | Owner | Closed by / to close |
|---|---|---|---|---|
| join 1 | `http.other[0].json.commit`: "unknown" (recording) vs "b7802aac97" | harness input: the gate passed COMMIT; the JS recording hub reports "unknown" | harness | run the replay without COMMIT (as the runs with join 0), no normalisation |
| 3 x 6 | `hk` only in JS: alert.<t>, summary.at.<t> x2, told.<id> (4) and autosettle.read.<t>, star.nudge.v1 (2) | real hub difference | state; agents | alert/summary.at/told: at b7802aac97 nothing in production sets `"_end"` on a TurnChanged; the marker lives only behind `MARK_ENDS` on `gpui-agents`, so the policy never runs `Ended.out`. Agents' flip is what closes them (to be checked when that range comes to the reviewer; no replay yet). autosettle.read: written by code nothing calls yet (`autosettle::read` via `Policy::read`, state/host.rs:50 `StateHost::read`); I found no caller in hub.rs, main.rs, agents/ or session/ (a search, not a proof). star.nudge.v1: `state/star.rs` has no caller outside the state module (the reviewer's search) |
| 3 x 1 | `deltas`: `delta_end` for the turn only in Rust | real hub difference | agents | the JS stand-in turn sends no delta_end for it |

Not in the 22, but seen in the offline comparison of the 29 run (j20261105083919, hub 6db1d10b8) with the new comparator: `liveness.<t>` and `autosettle.quiet.<t>` missing in Rust (JS wrote the quiet
mark 3 times). They were missing AT 6db1d10b8; their writers landed since (079a3959c0 liveness, fe92a51285 / 4c6b5df827 policy hooks and quiet marks: all in b7802aac97 and not in 6db1d10b8), so their
status at the tip is NOT known: `autosettle.quiet` is most likely written, `liveness` is unchecked because the old N22 dropped it. The rerun with 2ac7925275 decides; they are not open differences until it shows them.

What took the groups out between the 29 run and the 22 (named commits, not counts):
- fab.* and `@rp.req`/`@telemetry.req` (10 lines per scenario): present in the replay at 9eb3df875 (j20261105094114: 56/72/56), absent at b7802aac97. Fixed by code between
  6db1d10b8-era and b7802aac97, most likely 2517ab3dbe (a recorded fab.look joins the view of the next served request: the client sends it with the view.open, the hub answered while the
  old view was current and the read was dropped) with files' replied_late for the marks. NOT isolated: the hub and the policy binary both changed between the two runs (bp-policy-page then, bp-policy-bp now),
  and the offline comparison only proves the comparator did not remove them.
- autosettle.{audience,output x4,quiet} (6 of the 12 hk lines; `autosettle.read` is still in today's 6): state's Policy hooks fe92a51285, 4c6b5df827 (and liveness 079a3959c0), already in 9eb3df875;
  the 6 hk lines left at 9eb3df875 are the same 6 as now.
- deltas, 7 -> 1: N24 (741bcbf2df) joins a thread's text until delta_end. A NORMALISATION hid the frame-shape differences; the guard `streamed` (2ac7925275) now catches a hub that
  sends one frame where the other streams.

The 22 MAY be an undercount. This run used comparator a955d2e9c3 (hk as a set, liveness dropped). With 2ac7925275 real hk counts and `liveness` are checked for the first time; until the rerun
they are unchecked. The rerun of all scenarios with the new comparator, wide ones included, is what can settle this file; the milestone is 0 across all of them.

## Wide scenarios (UNSETTLED): rooms, from gate2 (job j20261105160055-804575, tip 2ac7925275, comparator 2ac7925275)

rooms 13 lines, classified. Baseline drift is named as such: the JS baseline (post-#960 main c4606b5a9c, two sandbox edits, not an ancestor of gpui; merge-base 7c9b3c223c)
lacks 6 gpui commits in src/core, mobile, server, web and gpui lacks 11 main commits since; replacing it with a baseline emitted from the gpui tip is a gate item.

| Lines | What | Class | Owner / status |
|---|---|---|---|
| 8 | `bot.posts[0..1].cat`: Rust `"cat":""` on the person's room posts (2 posts x 2 views x shown/overlay) | baseline drift: 166a115d99 / 43472d0cfd (2026-11-04) added the field after c4606b5a9c | N27 (temporary, landed c387657cc): `cat:""` in B equals none in A, one direction, `bot.posts[*]`, person only; remove with a rebuilt baseline |
| 2 | `hk` only in JS: `account.thread.<id>.claude` for the two bot threads | REAL, not drift: the writer is in src/core/accounts.bend and hub.bend at the merge-base 7c9b3c223c, and neither side changed `account.thread` since (git log -S empty on both) | bots (sticky account at thread creation) |
| 1 | `hk` only in JS: `star.nudge.v1` | REAL, not drift: src/core/star-nudge.bend and server.bend have it at the merge-base, unchanged since | state (star.rs has no caller) |
| 2 | view.open of room `ghost`: JS `STALE_VIEW`, Rust a normal view (`replies[0]`, `views[4]`) | REAL: the policy flagged `stale`, the host did not map it | session: fixed in 427447a427 (landed c387657cc), to be shown by the rerun |
