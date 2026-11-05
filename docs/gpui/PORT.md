# Porting the hub to Rust

Decision (2026-11-04): the backend moves to Rust. Bend keeps only the small, pure
rules worth keeping in it: policy, validation and state-transition rules. Everything
that changes over time goes to Rust: the event store, sockets, files, processes,
broadcast and caches.

Scope: the `gpui` branch and the Backplane GPUI project only. `main` and the live
Backplane stay untouched, and nothing merges to main without the person's OK.

Why, from `NATIVE-BEND.md`: native Bend is 1.4-2.7x faster than Bend on JS and uses far
less memory, but stays 3.6-95x slower than plain TypeScript on hub hot paths, and the
whole hub cannot be emitted to C (212 JS-only effect defs; the emit exceeds 8 GiB).

## Target

```
GPUI, iOS, Android  ──  today's protocol, unchanged  ──  backplane-hub (Rust, native/host)
                                                         ├ event store, windowed history
                                                         ├ broadcast, fan-out, view sessions
                                                         ├ threads, sidebar status
                                                         ├ agent processes
                                                         ├ files, git, KiCad IPC
                                                         └ calls: Bend rules; presenter
```

`events.jsonl` keeps its format: persisted events stay decodable forever, and the Rust
hub reads a log the Bend hub wrote, and the reverse.

## Native clients only

Decision (2026-11-04): no web page. The clients are GPUI, SwiftUI and Compose. The Rust
hub serves no web client and has no `renderer: web`, so `src/web/view.bend` and the
web host are out of the port. The web UI remains only as the reference for how the
native apps should look, and Electron remains only as the baseline to measure against.

## The presentation layer

The hub draws what clients show: GPUI and the phones get a screen model (`Render.mobile`)
from client.bend, screen.bend and hubs.bend, about 22,000 lines, far from small.

It is pure, so the plan is staged:

1. **Presenter sidecar (transitional).** Rust owns all state and asks `bp-policy`
   (native Bend, no JS) for a picture.
2. **Screen model to Rust.** Row by row and screen by screen, each compared with the Bend
   output on recorded traffic, the sidebar first. This also removes the O(threads^2)
   sidebar render.

Measured so far on the presenter: `-O0` only (114 MB, an optimized build exceeds the cap
until the C is split into units with `cc-split.py`); render is O(threads^2) in shared
Bend code, and the JS hub has it too (1,000 threads: 503 ms).

The phones still run `bridge.js`, Bend's client compiled to JS inside the app. That is
separate from the hub, and it stays until a later decision.

## Milestones

Each milestone ends with its gate passing and its numbers reported. Later milestones
start only when the one before passes, except where noted.

| # | Work | Gate |
|---|---|---|
| M0 | Parity harness; baselines; law map; store copy | Harness replays recorded traffic against today's hub and reports zero differences against itself |
| M1 | Protocol: HTTP, authenticated WebSocket, join, view open/update/action, CBOR frames | Recorded sessions match the JS hub; phone and GPUI clients connect to the Rust hub on the store copy |
| M2 | Event store and windowed history | Same log, same pages; torn tail, restart and 1M-event tests pass |
| M3 | Broadcast and fan-out | Keyed deltas match; slow clients bounded; 20-agent load valid |
| M4 | Threads and sidebar status | Sidebar and settle state match on recorded logs; 300 and 6,666 threads stay fast |
| M5 | Agent process management | Stand-in turns match event for event; interrupt, resume and restart tested |
| M6 | Files and KiCad IPC | Files tab, viewers and `kicad_ipc` match; plots served to all clients |
| M7 | 24 h run | 24 h with 20 simulated agents on the store copy, with no rising memory floor |

## Measurement

Every milestone reports against the JS hub on the same store copy and machine:

- Startup to usable
- Steady memory (24 h at M7; a shorter simulated run before that, labelled as such)
- CPU under the 20-agent load
- Click and popup latency: sidebar click, thread switch, composer popup
- Build time and peak build memory; binary size

The live hub is only observed, read-only, through `/proc`. Load tests use simulated
agents only, never real ones.

## Laws

`LAWS.bend` holds 3,248 laws and stays as it is: humans own it, and no law is weakened
or deleted. Each law has one of these statuses, listed for every law below:

- **bend**: the logic stays in Bend and Rust calls it. The law still checks that code.
- **rust**: the logic moved. A named Rust property test states the same claim.
- **unported**: the logic has not moved. The law still stands on the Bend code.
- **client**: client-side Bend in the web and phone bundles, unchanged.
- **window**: native-window code GPUI replaces. The entry says what GPUI does instead,
  or that it has no equivalent yet.
- **retired**: web-client-only, gone with the web page. The entry names the law's reason;
  a law that also guards a native client is not retired.

First classification, by the file whose defs a law names (within about 10% until each
law is checked; detail in `PORT-CLASSIFICATION.md`): 1,470 stay in Bend, 963 move to Rust,
581 are client-side and 221 are native-window code. Some client-side laws become
`retired` now that the web page is gone.

A function stays in Bend only if it is pure, takes and returns kilobytes rather than a
state, is off the per-event and per-view hot paths, and has a law or security property
that justifies it. Those rules run in one persistent process that can do no IO. Bend's
`State.replay` stays as a differential oracle: every recorded log must give the same
canonical state digest in Bend and in Rust.

Porting a piece of logic changes its laws from `unported` to `bend` or `rust` in the same
commit. `scripts/gpui/law-map.py` fails when ported code has a law with neither, or when
a named test does not exist. A law with no honest Rust statement stays in Bend, or the
port waits for the person's decision.

## Build and test rules

- Builds run on Boat (8 GiB cap, zero swap) or the Mac, never on the desktop.
- The hub runs on a copy of the event log on its own port, with phone access over the
  tailnet. It never touches `~/.backplane-bend` or port 3787.
- The copy holds real conversations: it stays on this machine, out of git and off
  Boat. Parity recordings sent to Boat come from fixtures only.

<!-- law-map: generated below by scripts/gpui/law-map.py -->

Laws in LAWS.bend: 3248. bend 1287, rust 0, unported 1350, client 483, window 128. Data file: `scripts/gpui/law-map.tsv`; ported sources: none yet. `seed` is the Bend stream's file-level classification (`docs/gpui/PORT-CLASSIFICATION.md`), accurate to about 10% until each law is checked; `file` is the source file the law names most.

| law | status | seed | file | note |
|---|---|---|---|---|
| `string_eq_refl` | unported | none | `-` |  |
| `settle_at_window` | unported | seed R +-10% | `src/core/model.bend` |  |
| `settle_fresh` | unported | seed R +-10% | `src/core/model.bend` |  |
| `settle_monotone` | unported | seed R +-10% | `src/core/model.bend` |  |
| `settle_never_running` | unported | seed R +-10% | `src/core/model.bend` |  |
| `settle_never_pending` | unported | seed R +-10% | `src/core/model.bend` |  |
| `settle_respects_unsettle` | unported | seed R +-10% | `src/core/model.bend` |  |
| `replay_snoc` | unported | seed R +-10% | `src/core/model.bend` |  |
| `sidebar_split_count` | unported | seed R +-10% | `src/core/model.bend` |  |
| `sidebar_split_at_count` | unported | seed R +-10% | `src/core/model.bend` |  |
| `settle_never_snoozed` | unported | seed R +-10% | `src/core/model.bend` |  |
| `snooze_wakes` | unported | seed R +-10% | `src/core/model.bend` |  |
| `key_after` | unported | seed R +-10% | `src/core/model.bend` |  |
| `key_mid_above` | unported | seed R +-10% | `src/core/model.bend` |  |
| `key_mid_below` | unported | seed R +-10% | `src/core/model.bend` |  |
| `project_keyed_first` | client | seed C +-10% | `src/core/client.bend` |  |
| `project_unkeyed_below` | client | seed C +-10% | `src/core/client.bend` |  |
| `status_approval_first` | unported | seed R +-10% | `src/core/model.bend` |  |
| `status_input_before_work` | unported | seed R +-10% | `src/core/model.bend` |  |
| `status_working_before_queued` | unported | seed R +-10% | `src/core/model.bend` |  |
| `status_failed_before_queued` | unported | seed R +-10% | `src/core/model.bend` |  |
| `status_approval_top` | unported | seed R +-10% | `src/core/model.bend` |  |
| `phase_approval_first` | unported | seed R +-10% | `src/core/model.bend` |  |
| `phase_input_before_work` | unported | seed R +-10% | `src/core/model.bend` |  |
| `phase_working_before_tasks` | unported | seed R +-10% | `src/core/model.bend` |  |
| `phase_monitoring_before_complete` | unported | seed R +-10% | `src/core/model.bend` |  |
| `phase_complete` | unported | seed R +-10% | `src/core/model.bend` |  |
| `phase_failed_before_queued` | unported | seed R +-10% | `src/core/model.bend` |  |
| `phase_silent_only_idle` | unported | seed R +-10% | `src/core/model.bend` |  |
| `scene_merge_self` | window | seed D +-10% | `src/app/viewer.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `fade_start` | window | seed D +-10% | `src/app/viewer.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `fade_done` | window | seed D +-10% | `src/app/viewer.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `look_flip_none` | window | seed D +-10% | `src/app/viewer.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `look_flip_hides` | window | seed D +-10% | `src/app/viewer.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `look_flip_shows` | window | seed D +-10% | `src/app/viewer.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `look_light_same_alpha` | window | seed D +-10% | `src/app/viewer.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `look_grounds_not_layers` | window | seed D +-10% | `src/app/viewer.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `theme_dark_as_is` | window | seed D +-10% | `src/app/theme.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `look_flip_twice` | window | seed D +-10% | `src/app/viewer.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `ops_hidden_draws_nothing` | window | seed D +-10% | `src/app/viewer.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `pick_hidden_skipped` | window | seed D +-10% | `src/app/viewer.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `semver_cmp_refl` | bend | seed K +-10% | `src/core/semver.bend` |  |
| `update_never_same` | bend | seed K +-10% | `src/core/semver.bend` |  |
| `update_off_managed` | bend | seed K +-10% | `src/core/semver.bend` |  |
| `update_appimage_managed` | bend | seed K +-10% | `src/core/semver.bend` |  |
| `semver_pre_before_release` | bend | seed K +-10% | `src/core/semver.bend` |  |
| `semver_nightly_later` | bend | seed K +-10% | `src/core/semver.bend` |  |
| `semver_nightly_next_day` | bend | seed K +-10% | `src/core/semver.bend` |  |
| `attach_clean_ok` | bend | seed K +-10% | `src/core/attach.bend` |  |
| `drop_non_file_ignored` | bend | seed K +-10% | `src/core/attach.bend` |  |
| `drop_uri_list_example` | bend | seed K +-10% | `src/core/attach.bend` |  |
| `task_terminal_absorbing` | unported | seed R +-10% | `src/core/model.bend` |  |
| `task_never_requeued` | unported | seed R +-10% | `src/core/model.bend` |  |
| `task_delivered_once` | unported | seed R +-10% | `src/core/model.bend` |  |
| `delegation_bounded` | unported | seed R +-10% | `src/core/model.bend` |  |
| `setting_default` | unported | seed R +-10% | `src/core/model.bend` |  |
| `nui_with_own_vw` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `stale_board_ignored` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `resume_is_replay` | unported | seed R +-10% | `src/core/model.bend` |  |
| `post_running_delivers` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `deliver_never_waits` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `deliver_held_go` | unported | seed R +-10% | `src/core/model.bend` |  |
| `deliver_held_steer` | unported | seed R +-10% | `src/core/model.bend` |  |
| `deliver_held_steer_k` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `deliver_claude_running` | unported | seed R +-10% | `src/core/model.bend` |  |
| `resend_stored_once` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `has_msg_unnamed_skips` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `mcp_last_newest` | unported | seed R +-10% | `src/core/model.bend` |  |
| `task_finish_not_ended_same` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `inbox_running_delivers` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `worktree_pending_local` | unported | seed R +-10% | `src/core/model.bend` |  |
| `worktree_pending_ends` | unported | seed R +-10% | `src/core/model.bend` |  |
| `worktree_waits` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `worktree_by_default` | unported | seed R +-10% | `src/core/model.bend` |  |
| `worktree_revives` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `worktree_kept_while_busy` | unported | seed R +-10% | `src/core/model.bend` |  |
| `worktree_kept_out` | unported | seed R +-10% | `src/core/model.bend` |  |
| `worktree_kept_unless_ours` | unported | seed R +-10% | `src/core/model.bend` |  |
| `worktree_kept_fresh` | unported | seed R +-10% | `src/core/model.bend` |  |
| `worktree_dropped_on_merge` | unported | seed R +-10% | `src/core/model.bend` |  |
| `worktree_dropped_when_aged` | unported | seed R +-10% | `src/core/model.bend` |  |
| `worktree_dropped_when_hidden` | unported | seed R +-10% | `src/core/model.bend` |  |
| `merge_settles` | unported | seed R +-10% | `src/core/model.bend` |  |
| `merge_once` | unported | seed R +-10% | `src/core/model.bend` |  |
| `merge_needs_branch` | unported | seed R +-10% | `src/core/model.bend` |  |
| `gh_pick_saved` | bend | seed K +-10% | `src/core/gh.bend` |  |
| `gh_pick_owner` | bend | seed K +-10% | `src/core/gh.bend` |  |
| `gh_pick_active` | bend | seed K +-10% | `src/core/gh.bend` |  |
| `gh_order_first` | bend | seed K +-10% | `src/core/gh.bend` |  |
| `gh_without_drops` | bend | seed K +-10% | `src/core/gh.bend` |  |
| `alert_needs_running` | unported | seed R +-10% | `src/core/model.bend` |  |
| `alert_never_running` | unported | seed R +-10% | `src/core/model.bend` |  |
| `alert_not_on_stop` | unported | seed R +-10% | `src/core/model.bend` |  |
| `alert_none_on_connect` | unported | seed R +-10% | `src/core/model.bend` |  |
| `notice_fail_always` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `notice_thread_kept` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `notice_ask_kept` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `notice_room_once` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `notice_chatter_quiet` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `notice_earlier_bot_quiet` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `notice_person_opens` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `notice_window_opens` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `hubs_no_id` | unported | none | `-` |  |
| `q_kind_names` | bend | seed K +-10% | `src/core/eq.bend` |  |
| `hubs_delta_elsewhere` | unported | seed R +-10% | `src/core/json.bend` |  |
| `hubs_delta_other_thread` | unported | seed R +-10% | `src/core/json.bend` |  |
| `hubs_redraw_changes_elsewhere` | unported | seed R +-10% | `src/core/json.bend` |  |
| `hubs_redraw_delta_elsewhere` | unported | seed R +-10% | `src/core/json.bend` |  |
| `hubs_redraw_delta_other_thread` | unported | seed R +-10% | `src/core/json.bend` |  |
| `alerts_none_for_delta` | unported | seed R +-10% | `src/core/json.bend` |  |
| `island_doing_mine_same` | client | seed C +-10% | `src/core/client.bend` |  |
| `island_doing_mine_text` | client | seed C +-10% | `src/core/client.bend` |  |
| `hubs_screen_unfocused_thread_null` | unported | seed R +-10% | `src/core/json.bend` |  |
| `hubs_unpaired_quiet` | unported | none | `-` |  |
| `island_idle_empty` | unported | seed R +-10% | `src/core/model.bend` |  |
| `device_token_never_sent` | bend | seed K +-10% | `src/core/codec.bend` |  |
| `device_drop_never_sent` | bend | seed K +-10% | `src/core/codec.bend` |  |
| `device_blank_ignored` | unported | seed R +-10% | `src/core/model.bend` |  |
| `device_register_once` | unported | seed R +-10% | `src/core/model.bend` |  |
| `fuzzy_empty_matches` | bend | seed K +-10% | `src/core/fuzzy.bend` |  |
| `find_empty_keeps` | client | seed C +-10% | `src/core/client.bend` |  |
| `find_shut_shows_all` | client | seed C +-10% | `src/core/client.bend` |  |
| `find_always_on` | client | seed C +-10% | `src/core/client.bend` |  |
| `urg_approval_top` | client | seed C +-10% | `src/core/client.bend` |  |
| `urg_input_over_working` | client | seed C +-10% | `src/core/client.bend` |  |
| `urg_done_over_working` | client | seed C +-10% | `src/core/client.bend` |  |
| `urg_pin_within` | client | seed C +-10% | `src/core/client.bend` |  |
| `active_fold_shut` | client | seed C +-10% | `src/core/client.bend` |  |
| `menu_renames_settled` | client | seed C +-10% | `src/core/client.bend` |  |
| `menu_renames_open` | client | seed C +-10% | `src/core/client.bend` |  |
| `project_back_no_thread` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `rename_blank_nothing` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `menu_settled_brings_back` | client | seed C +-10% | `src/core/client.bend` |  |
| `caret_shows_when_moved` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `tailnet_no_owner_no_trust` | bend | seed K +-10% | `src/core/tailnet.bend` |  |
| `tailnet_link_token` | bend | seed K +-10% | `src/core/tailnet.bend` |  |
| `seek_made` | window | seed D +-10% | `src/gfx/tiles.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `tiles_again_exact` | window | seed D +-10% | `src/gfx/tiles.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `tile_keep` | window | seed D +-10% | `src/gfx/tiles.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `frame_again_exact` | window | seed D +-10% | `src/gfx/tiles.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `drawn_get_run` | window | seed D +-10% | `src/gfx/tiles.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `plan_fill_again` | window | seed D +-10% | `src/gfx/tiles.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `frame_par_exact` | window | seed D +-10% | `src/gfx/tiles.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `tl_again_exact` | window | seed D +-10% | `src/app/layout.bend` | GPUI layout, native/gpui/src/main.rs (equivalence unchecked) |
| `tl_keep` | window | seed D +-10% | `src/app/layout.bend` | GPUI layout, native/gpui/src/main.rs (equivalence unchecked) |
| `tl_memo_exact` | window | seed D +-10% | `src/app/layout.bend` | GPUI layout, native/gpui/src/main.rs (equivalence unchecked) |
| `tl_align` | window | seed D +-10% | `src/app/layout.bend` | GPUI layout, native/gpui/src/main.rs (equivalence unchecked) |
| `tl_lines_exact` | window | seed D +-10% | `src/app/layout.bend` | GPUI layout, native/gpui/src/main.rs (equivalence unchecked) |
| `link_no_script` | bend | seed K +-10% | `src/core/links.bend` |  |
| `link_no_file` | bend | seed K +-10% | `src/core/links.bend` |  |
| `link_path_not_web` | bend | seed K +-10% | `src/core/links.bend` |  |
| `link_file_abs_kept` | bend | seed K +-10% | `src/core/links.bend` |  |
| `menu_file_open_first` | client | seed C +-10% | `src/core/client.bend` |  |
| `link_plain_line` | bend | seed K +-10% | `src/core/links.bend` |  |
| `lightbox_zoom_min` | bend | seed K +-10% | `src/core/lightbox.bend` |  |
| `lightbox_zoom_max` | bend | seed K +-10% | `src/core/lightbox.bend` |  |
| `lightbox_scroll_max` | bend | seed K +-10% | `src/core/lightbox.bend` |  |
| `lightbox_ease_start` | bend | seed K +-10% | `src/core/lightbox.bend` |  |
| `lightbox_ease_done` | bend | seed K +-10% | `src/core/lightbox.bend` |  |
| `lightbox_idle` | bend | seed K +-10% | `src/core/lightbox.bend` |  |
| `lightbox_opens_still` | bend | seed K +-10% | `src/core/lightbox.bend` |  |
| `svg_close_returns` | bend | seed K +-10% | `src/core/svg.bend` |  |
| `svg_move_starts` | bend | seed K +-10% | `src/core/svg.bend` |  |
| `svg_absolute_ignores_pen` | bend | seed K +-10% | `src/core/svg.bend` |  |
| `svg_implicit_lineto` | bend | seed K +-10% | `src/core/svg.bend` |  |
| `svg_numbers_split` | bend | seed K +-10% | `src/core/svg.bend` |  |
| `icons_used_exist` | bend | seed K +-10% | `src/core/icons.bend` |  |
| `icons_all_draw` | bend | seed K +-10% | `src/core/icons.bend` |  |
| `tools_icons_exist` | bend | seed K +-10% | `src/core/tools.bend` |  |
| `tools_labelled` | bend | seed K +-10% | `src/core/tools.bend` |  |
| `viewer_auto_opens` | client | seed C +-10% | `src/core/client.bend` |  |
| `viewer_auto_hand_shut` | client | seed C +-10% | `src/core/client.bend` |  |
| `auto_view_none` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `tools_stop_first` | bend | seed K +-10% | `src/core/tools.bend` |  |
| `hotkey_alt_digit` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `hotkey_ctrl_digit` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `hint_alt_starts` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `hint_missed_release_ends` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `wrap_keeps_last_space` | window | seed D +-10% | `src/gfx/text.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `edit_select_all` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `edit_type_replaces` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `delete_asks_first` | client | seed C +-10% | `src/core/client.bend` |  |
| `row_delete_asks_first` | client | seed C +-10% | `src/core/client.bend` |  |
| `row_toggle_unsettles` | unported | seed R +-10% | `src/core/model.bend` |  |
| `row_toggle_settles` | unported | seed R +-10% | `src/core/model.bend` |  |
| `picker_drops_stale` | client | seed C +-10% | `src/core/client.bend` |  |
| `tl_frame_exact` | window | seed D +-10% | `src/app/layout.bend` | GPUI layout, native/gpui/src/main.rs (equivalence unchecked) |
| `fold_keeps_entries` | client | seed C +-10% | `src/core/fold.bend` |  |
| `fold_count` | client | seed C +-10% | `src/core/fold.bend` |  |
| `fold_runs_only_tools` | client | seed C +-10% | `src/core/fold.bend` |  |
| `fold_never_error` | client | seed C +-10% | `src/core/fold.bend` |  |
| `fold_never_message` | unported | seed R +-10% | `src/core/model.bend` |  |
| `fold_user_opened` | client | seed C +-10% | `src/core/fold.bend` |  |
| `fold_user_shut` | client | seed C +-10% | `src/core/fold.bend` |  |
| `fold_default_live` | client | seed C +-10% | `src/core/fold.bend` |  |
| `fold_run_opened` | client | seed C +-10% | `src/core/fold.bend` |  |
| `fold_run_shut` | client | seed C +-10% | `src/core/fold.bend` |  |
| `fold_run_default_shut` | client | seed C +-10% | `src/core/fold.bend` |  |
| `fold_run_live_shut` | client | seed C +-10% | `src/core/fold.bend` |  |
| `fold_roles_message_shown` | client | seed C +-10% | `src/core/fold.bend` |  |
| `work_message_shown` | client | seed C +-10% | `src/core/fold.bend` |  |
| `work_live_shut` | client | seed C +-10% | `src/core/fold.bend` |  |
| `btw_command` | client | seed C +-10% | `src/core/client.bend` |  |
| `btw_not_a_word` | client | seed C +-10% | `src/core/client.bend` |  |
| `btw_question` | client | seed C +-10% | `src/core/client.bend` |  |
| `btw_never_logged` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `kicad_ipc_needs_fork` | client | seed C +-10% | `src/core/client.bend` |  |
| `kicad_missing_said` | client | seed C +-10% | `src/core/client.bend` |  |
| `kicad_missing_prompt` | bend | seed K +-10% | `src/core/prompt.bend` |  |
| `claude_kistack_plugin` | bend | seed K +-10% | `src/core/prompt.bend` |  |
| `runtime_no_kistack_block` | bend | seed K +-10% | `src/core/prompt.bend` |  |
| `mech_render_path` | bend | seed K +-10% | `src/core/mech.bend` |  |
| `mech_part_renders` | bend | seed K +-10% | `src/core/mech.bend` |  |
| `mech_key_path` | bend | seed K +-10% | `src/core/mech.bend` |  |
| `mech_default_assembly` | bend | seed K +-10% | `src/core/mech.bend` |  |
| `mech_default_first` | bend | seed K +-10% | `src/core/mech.bend` |  |
| `mech_after_kept` | bend | seed K +-10% | `src/core/mech.bend` |  |
| `mech_after_gone` | bend | seed K +-10% | `src/core/mech.bend` |  |
| `mech_after_none` | bend | seed K +-10% | `src/core/mech.bend` |  |
| `mech_gui_linux` | bend | seed K +-10% | `src/core/prompt.bend` |  |
| `mech_gui_mac` | bend | seed K +-10% | `src/core/prompt.bend` |  |
| `png_crc_iend` | bend | seed K +-10% | `src/core/png.bend` |  |
| `freecad_missing_said` | client | seed C +-10% | `src/core/client.bend` |  |
| `freecad_missing_prompt` | bend | seed K +-10% | `src/core/prompt.bend` |  |
| `took_seconds` | unported | seed R +-10% | `src/core/model.bend` |  |
| `took_minutes` | unported | seed R +-10% | `src/core/model.bend` |  |
| `took_hours` | unported | seed R +-10% | `src/core/model.bend` |  |
| `hub_frames_cbor` | bend | seed K +-10% | `src/core/http.bend` |  |
| `hub_plot_frames_binary` | bend | seed K +-10% | `src/core/http.bend` |  |
| `plot_delta_exact` | bend | seed K +-10% | `src/core/plot.bend` |  |
| `plot_unchanged_sends_nothing` | bend | seed K +-10% | `src/core/plot.bend` |  |
| `plot_hidden_not_sent` | bend | seed K +-10% | `src/core/plot.bend` |  |
| `plot_courtyard_hidden` | bend | seed K +-10% | `src/core/plot.bend` |  |
| `plot_tint_own_colour` | bend | seed K +-10% | `src/core/plot.bend` |  |
| `tint_code_again` | bend | seed K +-10% | `src/core/board.bend` |  |
| `plot_numbers_exact` | bend | seed K +-10% | `src/core/plot.bend` |  |
| `view_closed_wants_nothing` | client | seed C +-10% | `src/mobile/view.bend` |  |
| `view_asks_once` | client | seed C +-10% | `src/core/client.bend` |  |
| `plot_frames_binary` | bend | seed K +-10% | `src/core/http.bend` |  |
| `pick_nothing_there` | client | seed C +-10% | `src/core/client.bend` |  |
| `pick_late_ignored` | client | seed C +-10% | `src/core/client.bend` |  |
| `pick_pad_over_zone` | client | seed C +-10% | `src/mobile/view.bend` |  |
| `pick_pad_over_zone_after` | client | seed C +-10% | `src/mobile/view.bend` |  |
| `pick_top_on_tie` | client | seed C +-10% | `src/mobile/view.bend` |  |
| `view_3d_draws_board` | client | seed C +-10% | `src/mobile/view.bend` |  |
| `view_3d_key` | client | seed C +-10% | `src/mobile/view.bend` |  |
| `view_sheet_key` | client | seed C +-10% | `src/mobile/view.bend` |  |
| `view_board_no_sheet` | client | seed C +-10% | `src/mobile/view.bend` |  |
| `view_board_version_key` | client | seed C +-10% | `src/mobile/view.bend` |  |
| `view_bare_key` | client | seed C +-10% | `src/mobile/view.bend` |  |
| `pick_off_skipped` | client | seed C +-10% | `src/mobile/view.bend` |  |
| `pcb3d_path_vars` | bend | seed K +-10% | `src/core/pcb3d.bend` |  |
| `pcb3d_path_unknown_kept` | bend | seed K +-10% | `src/core/pcb3d.bend` |  |
| `mesh_first_never_fades` | window | seed D +-10% | `src/gfx/mesh.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `scene3_prompt_parts_no_fade` | window | seed D +-10% | `src/gfx/face.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `scene3_late_parts_fade` | window | seed D +-10% | `src/gfx/face.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `scene3_fade_ends` | window | seed D +-10% | `src/gfx/face.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `scene3_stale_parts_dropped` | window | seed D +-10% | `src/gfx/face.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `cam_fit_turns_about` | window | seed D +-10% | `src/gfx/mesh.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `scene3_board_pivot` | window | seed D +-10% | `src/gfx/mesh.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `menu_rows_apart` | bend | seed K +-10% | `src/core/menu.bend` |  |
| `menu_chips_apart` | bend | seed K +-10% | `src/core/menu.bend` |  |
| `menu_chips_above_rows` | bend | seed K +-10% | `src/core/menu.bend` |  |
| `menu_rows_fit` | bend | seed K +-10% | `src/core/menu.bend` |  |
| `menu_scroll_in_list` | bend | seed K +-10% | `src/core/menu.bend` |  |
| `menu_width_fits` | bend | seed K +-10% | `src/core/menu.bend` |  |
| `menu_label_clear` | bend | seed K +-10% | `src/core/menu.bend` |  |
| `menu_press_first` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `menu_right_press_first` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `menu_press_nothing` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `menu_press_elsewhere` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `menu_press_rows_inside` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `menu_press_box_inside` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `menu_press_chips_inside` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `badge_icons_exist` | bend | seed K +-10% | `src/core/badge.bend` |  |
| `badge_auto_tint` | bend | seed K +-10% | `src/core/badge.bend` |  |
| `badge_unknown_color` | bend | seed K +-10% | `src/core/badge.bend` |  |
| `badge_set_color` | bend | seed K +-10% | `src/core/badge.bend` |  |
| `badge_default_icon` | bend | seed K +-10% | `src/core/badge.bend` |  |
| `badge_unknown_icon` | bend | seed K +-10% | `src/core/badge.bend` |  |
| `mpick_closed_draws_nothing` | client | seed C +-10% | `src/core/client.bend` |  |
| `mpick_filter_asks_nothing` | client | seed C +-10% | `src/core/client.bend` |  |
| `mpick_opens_home` | client | seed C +-10% | `src/mobile/picker.bend` |  |
| `mpick_tilde` | client | seed C +-10% | `src/mobile/picker.bend` |  |
| `title_user_rename_wins` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `title_applies_when_placeholder` | bend | seed K +-10% | `src/core/agent.bend` |  |
| `title_empty_keeps` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `text_model_default` | bend | seed K +-10% | `src/core/agent.bend` |  |
| `text_model_unknown_provider` | bend | seed K +-10% | `src/core/agent.bend` |  |
| `text_model_light` | bend | seed K +-10% | `src/core/agent.bend` |  |
| `text_model_mismatch` | bend | seed K +-10% | `src/core/agent.bend` |  |
| `text_model_mismatch_codex` | bend | seed K +-10% | `src/core/agent.bend` |  |
| `text_model_mismatch_claude` | bend | seed K +-10% | `src/core/agent.bend` |  |
| `text_model_kept` | bend | seed K +-10% | `src/core/agent.bend` |  |
| `view_other_stays_shut` | client | seed C +-10% | `src/core/client.bend` |  |
| `browser_page_own` | bend | seed K +-10% | `src/core/bot.bend` |  |
| `hier_resolve_absolute` | bend | seed K +-10% | `src/core/hier.bend` |  |
| `hier_resolve_beside` | bend | seed K +-10% | `src/core/hier.bend` |  |
| `hier_loop_listed_once` | bend | seed K +-10% | `src/core/hier.bend` |  |
| `hier_root_is_top` | bend | seed K +-10% | `src/core/hier.bend` |  |
| `hier_up_after_open` | bend | seed K +-10% | `src/core/hier.bend` |  |
| `draft_kept` | client | seed C +-10% | `src/core/client.bend` |  |
| `draft_keep_thread` | client | seed C +-10% | `src/core/client.bend` |  |
| `draft_keep_no_thread` | client | seed C +-10% | `src/core/client.bend` |  |
| `draft_keep_no_scratch` | client | seed C +-10% | `src/core/client.bend` |  |
| `draft_keep_no_path` | client | seed C +-10% | `src/core/client.bend` |  |
| `ui_seq_counts_unknown` | client | seed C +-10% | `src/core/client.bend` |  |
| `ui_seq_known_unchanged` | client | seed C +-10% | `src/core/client.bend` |  |
| `ui_seq_log_counts_unknown` | client | seed C +-10% | `src/core/client.bend` |  |
| `reset_keeps_viewer` | client | seed C +-10% | `src/core/client.bend` |  |
| `reset_keeps_hand` | client | seed C +-10% | `src/core/client.bend` |  |
| `reset_new_origin_drops_viewer` | client | seed C +-10% | `src/core/client.bend` |  |
| `reset_keeps_card` | client | seed C +-10% | `src/core/client.bend` |  |
| `reset_asks_card_again` | client | seed C +-10% | `src/core/client.bend` |  |
| `reset_drops_results` | client | seed C +-10% | `src/core/client.bend` |  |
| `far_feed_after_stops` | bend | seed K +-10% | `src/core/far.bend` |  |
| `draft_restore_fills` | client | seed C +-10% | `src/core/client.bend` |  |
| `draft_restore_keeps_typing` | client | seed C +-10% | `src/core/client.bend` |  |
| `interrupt_idle_nothing` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `interrupt_claude_input` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `interrupt_codex_stops` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `interrupt_grok_stops` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `interrupt_muse_stops` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `muse_run_turn` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `msp_mode_supervised_asks` | bend | seed K +-10% | `src/core/msp.bend` |  |
| `msp_mode_full_allows` | bend | seed K +-10% | `src/core/msp.bend` |  |
| `archive_running_stops` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `archive_idle_no_turn` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `archive_closes_approval` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `delete_running_stops` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `interrupt_closes_approval` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `interrupt_keeps_plan` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `interrupt_ends_child_task` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `gone_closes_approval` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `retire_stop_tells` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `archive_running_tells` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `delete_running_tells` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `archive_idle_tells_nothing` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `island_sync_on_start` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `island_sync_on_archive` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `island_sync_on_delete` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `island_sync_not_twice` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `island_sync_quiet` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `retire_answers_empty` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `claude_result_after_stop_quiet` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `claude_result_after_stop_interrupted` | unported | seed R +-10% | `src/core/model.bend` |  |
| `claude_result_running_completes` | unported | seed R +-10% | `src/core/model.bend` |  |
| `claude_delta_after_stop_quiet` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `claude_delta_running_streams` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `stopped_turn_quiet` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `recover_interrupts_running` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `recover_closes_asks` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `recover_keeps_order_ask` | unported | seed R +-10% | `src/core/model.bend` |  |
| `recover_leaves_idle` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `recover_empty_nothing` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `carry_continues` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `carry_leaves_idle` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `carry_off_nothing` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `carry_once` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `carry_twice_once` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `carry_skips_archived` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `ask_makes_pending` | unported | seed R +-10% | `src/core/model.bend` |  |
| `answer_clears_pending` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `settle_waits_for_children` | unported | seed R +-10% | `src/core/model.bend` |  |
| `settle_children_done` | unported | seed R +-10% | `src/core/model.bend` |  |
| `tag_thread` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `tag_pid` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `tag_untagged` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `tag_old_process_unheard` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `tag_untagged_heard` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `follow_default_claude` | client | seed C +-10% | `src/core/client.bend` |  |
| `follow_default_codex` | client | seed C +-10% | `src/core/client.bend` |  |
| `follow_setting_queue` | client | seed C +-10% | `src/core/client.bend` |  |
| `follow_setting_steer` | client | seed C +-10% | `src/core/client.bend` |  |
| `follow_alt_same` | client | seed C +-10% | `src/core/client.bend` |  |
| `follow_stop_idle` | client | seed C +-10% | `src/core/client.bend` |  |
| `follow_stop_typed` | client | seed C +-10% | `src/core/client.bend` |  |
| `follow_stop_running` | client | seed C +-10% | `src/core/client.bend` |  |
| `follow_stop_interrupts` | client | seed C +-10% | `src/core/client.bend` |  |
| `queue_mode_delivers` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `steer_claude_in_place` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `steer_codex_restarts` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `queue_joined` | unported | seed R +-10% | `src/core/model.bend` |  |
| `queue_edit_in_place` | unported | seed R +-10% | `src/core/model.bend` |  |
| `queue_remove_one` | unported | seed R +-10% | `src/core/model.bend` |  |
| `queue_move_first` | unported | seed R +-10% | `src/core/model.bend` |  |
| `queue_move_unknown` | unported | seed R +-10% | `src/core/model.bend` |  |
| `queue_steer_one_leaves_rest` | unported | seed R +-10% | `src/core/model.bend` |  |
| `queue_row_running` | client | seed C +-10% | `src/core/client.bend` |  |
| `queue_row_held` | client | seed C +-10% | `src/core/client.bend` |  |
| `hand_newest_fit` | unported | seed R +-10% | `src/core/model.bend` |  |
| `hand_no_holes` | bend | seed K +-10% | `src/core/handoff.bend` |  |
| `hand_cut_says` | bend | seed K +-10% | `src/core/handoff.bend` |  |
| `handoff_never_runs_alone` | unported | seed R +-10% | `src/core/model.bend` |  |
| `handoff_with_message_runs` | unported | seed R +-10% | `src/core/model.bend` |  |
| `handoff_goes_first` | unported | seed R +-10% | `src/core/model.bend` |  |
| `fork_records_source` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `fork_waits_with_context` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `back_needs_fork` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `switch_between_turns` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `switch_same_nothing` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `switch_initial_choice_quiet` | unported | seed R +-10% | `src/core/model.bend` |  |
| `config_hold_keeps_queue_stopped` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `config_running_not_held` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `switch_new_session` | unported | seed R +-10% | `src/core/model.bend` |  |
| `switch_keeps_sessions` | unported | seed R +-10% | `src/core/model.bend` |  |
| `switch_keeps_sessions_both` | unported | seed R +-10% | `src/core/model.bend` |  |
| `switch_hands_new_provider` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `switch_back_missed_nothing` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `switch_takes_model_effort` | unported | seed R +-10% | `src/core/model.bend` |  |
| `switch_effort_offered` | unported | seed R +-10% | `src/core/model.bend` |  |
| `switch_model_between_turns` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `mcp_launch_in_project` | unported | seed R +-10% | `src/core/model.bend` |  |
| `mcp_other_project_refused` | bend | seed K +-10% | `src/core/mcp.bend` |  |
| `mcp_interrupt_idle_nothing` | bend | seed K +-10% | `src/core/mcp.bend` |  |
| `mcp_done_idle` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `mcp_not_done_running` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `wait_at_least_a_second` | bend | seed K +-10% | `src/core/mcp.bend` |  |
| `wait_at_most_ten_minutes` | bend | seed K +-10% | `src/core/mcp.bend` |  |
| `models_cache_json` | bend | seed K +-10% | `src/core/boot.bend` |  |
| `models_cache_text` | bend | seed K +-10% | `src/core/boot.bend` |  |
| `models_quiet` | bend | seed K +-10% | `src/core/boot.bend` |  |
| `store_state_is_replay` | bend | seed K +-10% | `src/core/store.bend` |  |
| `store_load_append` | bend | seed K +-10% | `src/core/store.bend` |  |
| `link_from_id` | unported | seed R +-10% | `src/core/model.bend` |  |
| `link_from_plain` | unported | seed R +-10% | `src/core/model.bend` |  |
| `tl_memos_hit` | window | seed D +-10% | `src/app/layout.bend` | GPUI layout, native/gpui/src/main.rs (equivalence unchecked) |
| `tl_memos_bounded` | window | seed D +-10% | `src/app/layout.bend` | GPUI layout, native/gpui/src/main.rs (equivalence unchecked) |
| `window_state_private` | unported | seed R +-10% | `src/core/model.bend` |  |
| `window_state_seq` | unported | seed R +-10% | `src/core/model.bend` |  |
| `window_adopt` | client | seed C +-10% | `src/core/client.bend` |  |
| `todo_write_whole_list` | unported | seed R +-10% | `src/core/json.bend` |  |
| `todo_line_active_form` | bend | seed K +-10% | `src/core/todo.bend` |  |
| `todo_task_created` | bend | seed K +-10% | `src/core/todo.bend` |  |
| `todo_task_result_number` | bend | seed K +-10% | `src/core/todo.bend` |  |
| `todo_task_update` | unported | seed R +-10% | `src/core/json.bend` |  |
| `todo_task_delete` | unported | seed R +-10% | `src/core/json.bend` |  |
| `todo_codex_completed` | unported | seed R +-10% | `src/core/json.bend` |  |
| `todo_acp_plan` | unported | seed R +-10% | `src/core/json.bend` |  |
| `todo_shown_while_running` | bend | seed K +-10% | `src/core/todo.bend` |  |
| `todo_shown_while_left` | bend | seed K +-10% | `src/core/todo.bend` |  |
| `todo_hidden_when_done` | bend | seed K +-10% | `src/core/todo.bend` |  |
| `todo_rows_around_open` | bend | seed K +-10% | `src/core/todo.bend` |  |
| `todo_rows_last` | bend | seed K +-10% | `src/core/todo.bend` |  |
| `todo_rows_short` | bend | seed K +-10% | `src/core/todo.bend` |  |
| `todos_start_shut` | client | seed C +-10% | `src/core/client.bend` |  |
| `todos_click_opens` | client | seed C +-10% | `src/core/client.bend` |  |
| `todos_shut_click_asks_open` | client | seed C +-10% | `src/core/client.bend` |  |
| `todos_shut_no_steps` | bend | seed K +-10% | `src/core/todo.bend` |  |
| `todo_write_sets_list` | unported | seed R +-10% | `src/core/json.bend` |  |
| `usage_keeps_window` | unported | seed R +-10% | `src/core/model.bend` |  |
| `usage_keeps_used` | unported | seed R +-10% | `src/core/model.bend` |  |
| `usage_text_share` | client | seed C +-10% | `src/core/client.bend` |  |
| `usage_text_tokens` | client | seed C +-10% | `src/core/client.bend` |  |
| `usage_nothing_yet` | client | seed C +-10% | `src/core/client.bend` |  |
| `usage_same_nothing` | unported | seed R +-10% | `src/core/model.bend` |  |
| `usage_change_logged` | unported | seed R +-10% | `src/core/model.bend` |  |
| `view_drag_rotates` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `view_drag_ctrl_pans` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `view_drag_shift_zooms` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `view_drag_alt_rolls` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `view_key_top` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `view_key_iso` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `view_key_ctrl_digit_free` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `view_key_arrow_turns` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `view_key_shift_arrow_quarter` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `view_key_alt_arrow_rolls` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `view_key_ctrl_arrow_pans` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `view_key_z_out` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `view_key_shift_z_in` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `view_key_other_none` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `sign_stale_rejected` | bend | seed K +-10% | `src/core/sign.bend` |  |
| `sign_window_300` | bend | seed K +-10% | `src/core/sign.bend` |  |
| `seen_add_has` | bend | seed K +-10% | `src/core/sign.bend` |  |
| `invite_roundtrip` | bend | seed K +-10% | `src/core/sign.bend` |  |
| `cron_next_after` | bend | seed K +-10% | `src/core/cron.bend` |  |
| `cron_due_fires_once` | bend | seed K +-10% | `src/core/cron.bend` |  |
| `space_caps` | bend | seed K +-10% | `src/core/space.bend` |  |
| `space_progress_max` | bend | seed K +-10% | `src/core/space.bend` |  |
| `space_message_prefix` | bend | seed K +-10% | `src/core/space.bend` |  |
| `space_page_locked` | bend | seed K +-10% | `src/core/space.bend` |  |
| `space_link_only_ours` | bend | seed K +-10% | `src/core/space.bend` |  |
| `google_scopes_nonempty` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_b64url_roundtrip_example` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_code_of_example` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_denied_no_code` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_state_home_example` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_hop_other` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_hop_mine` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_hop_once` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_hop_tailnet_only` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_hop_no_path` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_hop_origin_header_injection_denied` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_hop_query_header_injection_denied` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_hop_wrapping_port_denied` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_hop_raw_fragment_denied` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_loopback_phone` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_loopback_none` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_loopback_not_host` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_loopback_not_userinfo` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_loopback_not_path` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_loopback_no_port` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_header_no_newline_example` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_send_asks` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_create_asks` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_expiring_not_fresh` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_own_client_wins` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_shared_scopes_all` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_shared_covers_all` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_own_covers_all` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_accts_pick_first` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_accts_put_replaces` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_accts_put_new` | bend | seed K +-10% | `src/core/google.bend` |  |
| `google_accts_drop_named` | bend | seed K +-10% | `src/core/google.bend` |  |
| `bot_hop_step` | bend | seed K +-10% | `src/core/bot.bend` |  |
| `bot_hop_bounded` | bend | seed K +-10% | `src/core/bot.bend` |  |
| `bot_hop_busy_keeps` | bend | seed K +-10% | `src/core/bot.bend` |  |
| `bot_hop_idle_starts` | bend | seed K +-10% | `src/core/bot.bend` |  |
| `bot_hop_queue_takes` | bend | seed K +-10% | `src/core/bot.bend` |  |
| `bot_hop_wakes_keep` | bend | seed K +-10% | `src/core/bot.bend` |  |
| `bot_hop_fanout` | bend | seed K +-10% | `src/core/bot.bend` |  |
| `bot_loop_up` | bend | seed K +-10% | `src/core/bot.bend` |  |
| `bot_loop_bounded` | bend | seed K +-10% | `src/core/bot.bend` |  |
| `bot_loop_stops` | bend | seed K +-10% | `src/core/bot.bend` |  |
| `bot_held_wakes_none` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `bot_room_no_takeover` | bend | seed K +-10% | `src/core/bot.bend` |  |
| `bot_room_deleted_stays` | bend | seed K +-10% | `src/core/bot.bend` |  |
| `rooms_listed_once` | client | seed C +-10% | `src/core/botui.bend` |  |
| `hubs_rooms_listed_once` | client | seed C +-10% | `src/core/botui.bend` |  |
| `rooms_same_id_once` | client | seed C +-10% | `src/core/botui.bend` |  |
| `rooms_same_name_both` | client | seed C +-10% | `src/core/botui.bend` |  |
| `bot_mood_wait` | client | seed C +-10% | `src/core/botui.bend` |  |
| `bot_mood_input` | client | seed C +-10% | `src/core/botui.bend` |  |
| `bot_mood_sleep_still` | client | seed C +-10% | `src/core/botui.bend` |  |
| `bot_mood_rest` | client | seed C +-10% | `src/core/botui.bend` |  |
| `cat_sleep_still` | bend | seed K +-10% | `src/core/cat.bend` |  |
| `cat_error_still` | bend | seed K +-10% | `src/core/cat.bend` |  |
| `cat_away_still` | bend | seed K +-10% | `src/core/cat.bend` |  |
| `cat_away_holds_still` | bend | seed K +-10% | `src/core/cat.bend` |  |
| `cat_moods_known` | bend | seed K +-10% | `src/core/cat.bend` |  |
| `cat_look_total` | bend | seed K +-10% | `src/core/cat.bend` |  |
| `index_stale_rejected` | bend | seed K +-10% | `src/core/sign.bend` |  |
| `bearer_needs_token` | bend | seed K +-10% | `src/core/sign.bend` |  |
| `ring_form_json` | bend | seed K +-10% | `src/core/ring.bend` |  |
| `tailnet_serve_https` | bend | seed K +-10% | `src/core/tailnet.bend` |  |
| `bot_spawn_step` | unported | seed R +-10% | `src/core/bothub.bend` |  |
| `bot_spawn_bounded` | unported | seed R +-10% | `src/core/bothub.bend` |  |
| `bot_request_keep` | unported | seed R +-10% | `src/core/bothub.bend` |  |
| `bot_request_in_room` | unported | seed R +-10% | `src/core/bothub.bend` |  |
| `bot_request_fresh_id` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `bot_delegate_needs_project` | bend | seed K +-10% | `src/core/mcp.bend` |  |
| `kids_bot_child_shown` | client | seed C +-10% | `src/core/client.bend` |  |
| `bot_mood_idle_still` | client | seed C +-10% | `src/core/botui.bend` |  |
| `perf_errors_always_logged` | bend | seed K +-10% | `src/core/perf.bend` |  |
| `perf_debug_quiet_by_default` | bend | seed K +-10% | `src/core/perf.bend` |  |
| `perf_gauge_window_is_reading` | bend | seed K +-10% | `src/core/perf.bend` |  |
| `perf_pct_empty` | bend | seed K +-10% | `src/core/perf.bend` |  |
| `perf_pct_bucket_top` | bend | seed K +-10% | `src/core/perf.bend` |  |
| `perf_pct_capped_by_max` | bend | seed K +-10% | `src/core/perf.bend` |  |
| `perf_hud_first_frame` | bend | seed K +-10% | `src/core/perf.bend` |  |
| `perf_hud_off_draws_nothing` | window | seed D +-10% | `src/app/layout.bend` | GPUI layout, native/gpui/src/main.rs (equivalence unchecked) |
| `voice_add_idem` | bend | seed K +-10% | `src/core/voice.bend` |  |
| `voice_add_has` | bend | seed K +-10% | `src/core/voice.bend` |  |
| `voice_fix_same` | bend | seed K +-10% | `src/core/voice.bend` |  |
| `voice_fix_empty` | bend | seed K +-10% | `src/core/voice.bend` |  |
| `voice_unsure_bounded` | bend | seed K +-10% | `src/core/voice.bend` |  |
| `voice_keywords_bounded` | bend | seed K +-10% | `src/core/voice.bend` |  |
| `voice_put_blank` | bend | seed K +-10% | `src/core/voice.bend` |  |
| `once_distinct` | bend | seed K +-10% | `src/core/once.bend` |  |
| `once_keeps` | bend | seed K +-10% | `src/core/once.bend` |  |
| `bot_rows_once` | client | seed C +-10% | `src/core/botui.bend` |  |
| `bot_rows_shown` | client | seed C +-10% | `src/core/botui.bend` |  |
| `bot_rows_keep` | client | seed C +-10% | `src/core/botui.bend` |  |
| `bot_picks_once` | client | seed C +-10% | `src/core/botui.bend` |  |
| `bot_picks_shown` | client | seed C +-10% | `src/core/botui.bend` |  |
| `hubs_bots_once` | bend | seed K +-10% | `src/core/once.bend` |  |
| `hubs_bots_shown` | unported | seed R +-10% | `src/core/json.bend` |  |
| `reply_peer_person` | unported | seed R +-10% | `src/core/bothub.bend` |  |
| `reply_peer_bot` | unported | seed R +-10% | `src/core/bothub.bend` |  |
| `reply_peer_room` | unported | seed R +-10% | `src/core/bothub.bend` |  |
| `reply_said_newest` | unported | seed R +-10% | `src/core/model.bend` |  |
| `reply_person_wakes_none` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `far_machine_magicdns` | unported | seed R +-10% | `src/core/json.bend` |  |
| `far_machine_self` | unported | seed R +-10% | `src/core/json.bend` |  |
| `far_machine_other_port` | unported | seed R +-10% | `src/core/json.bend` |  |
| `far_machine_stranger` | unported | seed R +-10% | `src/core/json.bend` |  |
| `tailnet_domain` | bend | seed K +-10% | `src/core/tailnet.bend` |  |
| `bot_newproj_asks` | unported | seed R +-10% | `src/core/bothub.bend` |  |
| `bot_newproj_once` | unported | seed R +-10% | `src/core/bothub.bend` |  |
| `bot_newproj_decline` | unported | seed R +-10% | `src/core/bothub.bend` |  |
| `bot_newproj_no_answer` | unported | seed R +-10% | `src/core/bothub.bend` |  |
| `refs_plain_offers_nothing` | bend | seed K +-10% | `src/core/refs.bend` |  |
| `refs_plain_not_typing` | bend | seed K +-10% | `src/core/refs.bend` |  |
| `refs_far_typing` | bend | seed K +-10% | `src/core/refs.bend` |  |
| `refs_agent_plain` | unported | seed R +-10% | `src/core/model.bend` |  |
| `refs_own_bot_never` | bend | seed K +-10% | `src/core/refs.bend` |  |
| `bot_mention_once` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `sum_never_made_due` | bend | seed K +-10% | `src/core/refs.bend` |  |
| `sum_not_due_asks_nothing` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `sum_not_due_stamps_nothing` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `bg_start_progress_end` | unported | seed R +-10% | `src/core/model.bend` |  |
| `bg_added_once` | unported | seed R +-10% | `src/core/model.bend` |  |
| `bg_progress_unlisted` | unported | seed R +-10% | `src/core/model.bend` |  |
| `bg_doing` | unported | seed R +-10% | `src/core/model.bend` |  |
| `status_waits_on_subagents` | unported | seed R +-10% | `src/core/model.bend` |  |
| `status_starting_works` | unported | seed R +-10% | `src/core/model.bend` |  |
| `status_work_before_waiting` | unported | seed R +-10% | `src/core/model.bend` |  |
| `status_input_before_waiting` | unported | seed R +-10% | `src/core/model.bend` |  |
| `status_no_subagents_same` | unported | seed R +-10% | `src/core/model.bend` |  |
| `shows_other_delta` | unported | seed R +-10% | `src/core/json.bend` |  |
| `shows_own_delta` | unported | seed R +-10% | `src/core/json.bend` |  |
| `shows_changes` | unported | seed R +-10% | `src/core/json.bend` |  |
| `idx_page_own` | window | seed D +-10% | `src/app/layout.bend` | GPUI layout, native/gpui/src/main.rs (equivalence unchecked) |
| `idx_page_sent` | window | seed D +-10% | `src/app/layout.bend` | GPUI layout, native/gpui/src/main.rs (equivalence unchecked) |
| `idx_sent_filed` | unported | seed R +-10% | `src/core/model.bend` |  |
| `idx_page_short` | window | seed D +-10% | `src/app/layout.bend` | GPUI layout, native/gpui/src/main.rs (equivalence unchecked) |
| `idx_links` | client | seed C +-10% | `src/core/client.bend` |  |
| `idx_links_found` | client | seed C +-10% | `src/core/client.bend` |  |
| `select_leaves_settings` | client | seed C +-10% | `src/core/client.bend` |  |
| `select_same_leaves_settings` | client | seed C +-10% | `src/core/client.bend` |  |
| `select_same_keeps_thread` | client | seed C +-10% | `src/core/client.bend` |  |
| `room_leaves_settings` | client | seed C +-10% | `src/core/client.bend` |  |
| `made_leaves_settings` | client | seed C +-10% | `src/core/client.bend` |  |
| `rail_shown_shuts` | client | seed C +-10% | `src/core/client.bend` |  |
| `rail_shown_no_value` | client | seed C +-10% | `src/core/client.bend` |  |
| `rail_other_shows` | client | seed C +-10% | `src/core/client.bend` |  |
| `rail_shut_shows` | client | seed C +-10% | `src/core/client.bend` |  |
| `rail_shut_lights_none` | client | seed C +-10% | `src/core/client.bend` |  |
| `rail_part_lights_mech` | client | seed C +-10% | `src/core/client.bend` |  |
| `rail_palette_shuts` | client | seed C +-10% | `src/core/client.bend` |  |
| `intent_on_its_thread` | client | seed C +-10% | `src/core/client.bend` |  |
| `intent_close_puts_away` | client | seed C +-10% | `src/core/client.bend` |  |
| `intent_select_puts_away` | client | seed C +-10% | `src/core/client.bend` |  |
| `hits_chat_line_slides` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `chat_notch_tile` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `set_scroll_48` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `bots_still_cat_posed_once` | window | seed D +-10% | `src/app/bots.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `bots_away_cat_posed_once` | window | seed D +-10% | `src/app/bots.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `bots_moving_cat_follows_clock` | window | seed D +-10% | `src/app/bots.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `bots_cats_calm_on_input` | window | seed D +-10% | `src/app/bots.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `far_id_parts` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_id_here` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_change_names_link` | unported | seed R +-10% | `src/core/model.bend` |  |
| `far_sent_names_sender` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_change_bot_named` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_settings_stay_home` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_peers_stay_home` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_hooks_stay_home` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_devices_stay_home` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_rooms_stay_home` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_gap_takes_nothing` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_old_takes_nothing` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_overlap_takes_new` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_batch_again` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_route_here_stays` | unported | seed R +-10% | `src/core/json.bend` |  |
| `far_route_goes_home` | unported | seed R +-10% | `src/core/json.bend` |  |
| `far_allowed_only_methods` | unported | seed R +-10% | `src/core/json.bend` |  |
| `far_no_delete` | unported | seed R +-10% | `src/core/json.bend` |  |
| `far_where_here` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_where_far` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_bot_tabs` | client | seed C +-10% | `src/core/botui.bend` |  |
| `far_bot_tabs_here` | client | seed C +-10% | `src/core/botui.bend` |  |
| `far_row_act_mirror` | client | seed C +-10% | `src/core/botui.bend` |  |
| `far_row_act_reported` | client | seed C +-10% | `src/core/botui.bend` |  |
| `bot_row_title_far` | client | seed C +-10% | `src/core/botui.bend` |  |
| `bot_row_title_here` | client | seed C +-10% | `src/core/botui.bend` |  |
| `bot_head_remote` | client | seed C +-10% | `src/core/botui.bend` |  |
| `bot_head_remote_view` | client | seed C +-10% | `src/core/botui.bend` |  |
| `far_bot_head_tabs` | client | seed C +-10% | `src/core/botui.bend` |  |
| `far_bot_head_stays_home` | client | seed C +-10% | `src/core/botui.bend` |  |
| `bot_head_here_owns` | client | seed C +-10% | `src/core/botui.bend` |  |
| `hubs_bots_mirror_dropped` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fit_count_guard` | window | seed D +-10% | `src/gfx/text.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `fit_count_step` | window | seed D +-10% | `src/gfx/text.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `fit_chunk_progress` | window | seed D +-10% | `src/gfx/text.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `fit_wrap_unicode` | window | seed D +-10% | `src/gfx/text.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `remote_dir_failed_peer` | unported | seed R +-10% | `src/core/bothub.bend` |  |
| `remote_dir_away_seen` | unported | seed R +-10% | `src/core/json.bend` |  |
| `remote_dir_arrival_order` | unported | seed R +-10% | `src/core/bothub.bend` |  |
| `remote_dir_other_unchanged` | unported | seed R +-10% | `src/core/json.bend` |  |
| `remote_dir_obsolete_ignored` | unported | seed R +-10% | `src/core/json.bend` |  |
| `remote_dir_known_not_queued` | unported | seed R +-10% | `src/core/bothub.bend` |  |
| `remote_dir_four_slots` | unported | seed R +-10% | `src/core/bothub.bend` |  |
| `remote_dir_freed_slot` | unported | seed R +-10% | `src/core/bothub.bend` |  |
| `want_other_hub` | client | seed C +-10% | `src/core/client.bend` |  |
| `want_old_generation` | client | seed C +-10% | `src/core/client.bend` |  |
| `want_before_install` | client | seed C +-10% | `src/core/client.bend` |  |
| `machine_clears_selection` | client | seed C +-10% | `src/core/client.bend` |  |
| `timeline_hidden_settings` | client | seed C +-10% | `src/core/client.bend` |  |
| `timeline_hidden_pane` | client | seed C +-10% | `src/core/client.bend` |  |
| `hub_cache_home_short` | client | seed C +-10% | `src/core/client.bend` |  |
| `hub_cache_remote_short` | client | seed C +-10% | `src/core/client.bend` |  |
| `hub_cache_bounded` | client | seed C +-10% | `src/core/client.bend` |  |
| `far_project_named` | unported | seed R +-10% | `src/core/model.bend` |  |
| `far_project_shared` | unported | seed R +-10% | `src/core/model.bend` |  |
| `far_project_thread_shared` | unported | seed R +-10% | `src/core/model.bend` |  |
| `far_label_here` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_label_away` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_cursor_keeps_marker` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_routine_routes_home` | unported | seed R +-10% | `src/core/json.bend` |  |
| `hubs_catalog_mirror_dropped` | unported | seed R +-10% | `src/core/json.bend` |  |
| `hubs_sidebar_view_everywhere` | unported | none | `-` |  |
| `far_epoch_legacy` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_epoch_same` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_epoch_reset` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_reset_keeps_sequence` | unported | seed R +-10% | `src/core/model.bend` |  |
| `far_loaded_live_wins` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_reply_preserves_error` | unported | seed R +-10% | `src/core/json.bend` |  |
| `far_draft_kept` | client | seed C +-10% | `src/core/client.bend` |  |
| `hubs_fold_null` | unported | seed R +-10% | `src/core/json.bend` |  |
| `hubs_origin_here` | unported | seed R +-10% | `src/core/json.bend` |  |
| `hubs_origin_elsewhere` | unported | seed R +-10% | `src/core/json.bend` |  |
| `far_arrival_no_choice` | client | seed C +-10% | `src/core/client.bend` |  |
| `far_arrival_not_shared` | client | seed C +-10% | `src/core/client.bend` |  |
| `far_arrival_thread_selected` | client | seed C +-10% | `src/core/client.bend` |  |
| `far_arrival_focus_unchanged` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `hubs_origin_label_plain` | unported | seed R +-10% | `src/core/json.bend` |  |
| `hubs_origin_title_here` | unported | none | `-` |  |
| `hubs_origin_title_away` | unported | none | `-` |  |
| `bg_start_keeps_call` | unported | seed R +-10% | `src/core/model.bend` |  |
| `bg_call_title` | unported | seed R +-10% | `src/core/model.bend` |  |
| `subs_agent_at_work` | unported | seed R +-10% | `src/core/model.bend` |  |
| `subs_agent_done_shut` | unported | seed R +-10% | `src/core/model.bend` |  |
| `subs_shell_named` | unported | seed R +-10% | `src/core/model.bend` |  |
| `subs_shell_not_agent` | unported | seed R +-10% | `src/core/model.bend` |  |
| `subs_badge` | client | seed C +-10% | `src/core/subs.bend` |  |
| `subs_row_keys` | client | seed C +-10% | `src/core/subs.bend` |  |
| `subs_done_row_key` | client | seed C +-10% | `src/core/subs.bend` |  |
| `subagent_start_index` | unported | seed R +-10% | `src/core/model.bend` |  |
| `subagent_call_index_isolated` | unported | seed R +-10% | `src/core/model.bend` |  |
| `subagent_recovered_start` | client | seed C +-10% | `src/core/subs.bend` |  |
| `delegated_conversation_parent` | unported | seed R +-10% | `src/core/model.bend` |  |
| `delegated_conversation_moved` | unported | seed R +-10% | `src/core/model.bend` |  |
| `delegated_orphan_not_hidden` | unported | seed R +-10% | `src/core/model.bend` |  |
| `delegated_bot_parent_unchanged` | unported | seed R +-10% | `src/core/model.bend` |  |
| `delegated_parent_recorded` | unported | seed R +-10% | `src/core/model.bend` |  |
| `desk_phone_never` | bend | seed K +-10% | `src/core/desk.bend` |  |
| `desk_unfocused_never` | bend | seed K +-10% | `src/core/desk.bend` |  |
| `desk_look_fresh` | bend | seed K +-10% | `src/core/desk.bend` |  |
| `desk_look_expires` | bend | seed K +-10% | `src/core/desk.bend` |  |
| `desk_seen_holds` | bend | seed K +-10% | `src/core/desk.bend` |  |
| `desk_seen_nobody` | bend | seed K +-10% | `src/core/desk.bend` |  |
| `desk_none_nobody` | bend | seed K +-10% | `src/core/desk.bend` |  |
| `alert_watched_quiet` | unported | seed R +-10% | `src/core/json.bend` |  |
| `desk_due_first` | bend | seed K +-10% | `src/core/desk.bend` |  |
| `hist_mark_self` | bend | seed K +-10% | `src/core/hist.bend` |  |
| `hist_version_family` | bend | seed K +-10% | `src/core/hist.bend` |  |
| `hist_compare_family` | bend | seed K +-10% | `src/core/hist.bend` |  |
| `hist_obj_folds` | bend | seed K +-10% | `src/core/hist.bend` |  |
| `hist_sheet_same_commit` | bend | seed K +-10% | `src/core/hist.bend` |  |
| `hist_fwd_last_live` | bend | seed K +-10% | `src/core/hist.bend` |  |
| `hist_back_live_last` | bend | seed K +-10% | `src/core/hist.bend` |  |
| `hist_index_live` | bend | seed K +-10% | `src/core/hist.bend` |  |
| `hist_record_nothing` | bend | seed K +-10% | `src/core/hist.bend` |  |
| `hist_track_unknown_file` | bend | seed K +-10% | `src/core/hist.bend` |  |
| `hist_snapshot_keeps_sheet` | bend | seed K +-10% | `src/core/hist.bend` |  |
| `hist_window_keeps_steps` | unported | seed R +-10% | `src/core/model.bend` |  |
| `hist_unresolved_track_empty` | client | seed C +-10% | `src/core/client.bend` |  |
| `hist_empty_source_not_jumpable` | client | seed C +-10% | `src/core/client.bend` |  |
| `hist_live_design_closes_picker` | client | seed C +-10% | `src/core/client.bend` |  |
| `hist_picker_scope` | unported | seed R +-10% | `src/server/thin-actor.bend` |  |
| `hist_tree_wire_file` | bend | seed K +-10% | `src/core/hier.bend` |  |
| `hist_playback_same_scope` | unported | seed R +-10% | `src/server/thin-actor.bend` |  |
| `look_plain_board` | bend | seed K +-10% | `src/core/look.bend` |  |
| `look_plain_sch` | bend | seed K +-10% | `src/core/look.bend` |  |
| `look_marks_distinct` | bend | seed K +-10% | `src/core/look.bend` |  |
| `look_names_marks` | bend | seed K +-10% | `src/core/look.bend` |  |
| `scratch_pinned_kept` | unported | seed R +-10% | `src/core/model.bend` |  |
| `scratch_running_kept` | unported | seed R +-10% | `src/core/model.bend` |  |
| `scratch_project_kept` | unported | seed R +-10% | `src/core/model.bend` |  |
| `scratch_expires` | unported | seed R +-10% | `src/core/model.bend` |  |
| `scratch_not_yet` | unported | seed R +-10% | `src/core/model.bend` |  |
| `scratch_days_default` | unported | seed R +-10% | `src/core/model.bend` |  |
| `scratch_days_most` | unported | seed R +-10% | `src/core/model.bend` |  |
| `scratch_made` | unported | seed R +-10% | `src/core/model.bend` |  |
| `scratch_removed_stays` | unported | seed R +-10% | `src/core/model.bend` |  |
| `scratch_always_live` | client | seed C +-10% | `src/core/client.bend` |  |
| `give_command` | client | seed C +-10% | `src/core/client.bend` |  |
| `give_not_a_word` | client | seed C +-10% | `src/core/client.bend` |  |
| `give_not_btw` | client | seed C +-10% | `src/core/client.bend` |  |
| `give_nothing_refused` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `keys_name_env` | bend | seed K +-10% | `src/core/apikeys.bend` |  |
| `keys_name_digit` | bend | seed K +-10% | `src/core/apikeys.bend` |  |
| `keys_quote_single` | bend | seed K +-10% | `src/core/apikeys.bend` |  |
| `keys_set_replaces` | bend | seed K +-10% | `src/core/apikeys.bend` |  |
| `keys_drop_keeps_others` | bend | seed K +-10% | `src/core/apikeys.bend` |  |
| `keys_names_of_lines` | bend | seed K +-10% | `src/core/apikeys.bend` |  |
| `keys_prompt_none` | bend | seed K +-10% | `src/core/apikeys.bend` |  |
| `keys_prompt_load_quotes_path` | bend | seed K +-10% | `src/core/apikeys.bend` |  |
| `keys_prompt_list_quotes_path` | bend | seed K +-10% | `src/core/apikeys.bend` |  |
| `md_head_first` | client | seed C +-10% | `src/web/dom.bend` |  |
| `md_head_only_first` | client | seed C +-10% | `src/web/dom.bend` |  |
| `md_fence_any` | client | seed C +-10% | `src/web/dom.bend` |  |
| `fm_kind_board` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_kind_schematic` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_kind_model` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_kind_pdf` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_kind_image` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_kind_other` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_safe_up` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_safe_abs` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_safe_down` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_parent_top` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_parent_down` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_join_root` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_skips_git` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_skips_history` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_skips_worktrees` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_keeps_checklist` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_skips_backups` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_skips_locks` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_skips_modules` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_keeps_board` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_keeps_lookalike` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_name_root` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_name_folder` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_top_never_up` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_top_one_name` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_url_encodes` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_get_url_encodes` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_disposition` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_disposition_one_line` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_skips_env` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_skips_env_variants` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_keeps_dotfiles` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_rules_known` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_within_history` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_within_notes` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_stores_compressed` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_deflates_boards` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_entry_backslash` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_top_windows` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_files_safe_backslash` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_files_safe_drive` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_files_safe_share` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_files_safe_down` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `emb_summary_round` | bend | seed K +-10% | `src/core/emb.bend` |  |
| `emb_size_round` | bend | seed K +-10% | `src/core/emb.bend` |  |
| `emb_pinned_live` | bend | seed K +-10% | `src/core/emb.bend` |  |
| `emb_message_ends` | bend | seed K +-10% | `src/core/emb.bend` |  |
| `emb_fresh_live` | bend | seed K +-10% | `src/core/emb.bend` |  |
| `emb_idle_ends` | bend | seed K +-10% | `src/core/emb.bend` |  |
| `emb_note_none` | bend | seed K +-10% | `src/core/emb.bend` |  |
| `emb_focus_round` | bend | seed K +-10% | `src/core/emb.bend` |  |
| `cmds_foreign_result_quiet` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `cmds_foreign_result_keeps_running` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `cmds_result_waits_lc` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `cmds_lc_completed_ends` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `cmds_lc_end_completes` | unported | seed R +-10% | `src/core/model.bend` |  |
| `cmds_pair_stays_running` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `cmds_tomb_error_quiet` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `cmds_tomb_cancel_quiet` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `cmds_stop_then_new_error_quiet` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `cmds_stop_then_new` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `cmds_wake_start` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `cmds_wake_runs` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `cmds_wake_result_ends` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `cmds_wake_with_queued_user` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `cmds_tomb_started_resumes` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `cmds_tomb_started_says` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `cmds_prep_counts` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `cmds_mode_version` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `cmds_mode_old` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `cmds_mode_old_legacy` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `cmds_mode_lc` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `cmds_legacy_retires_sent` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `cmds_legacy_learned_retires` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `cmds_legacy_prep_stays` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `cmds_lifecycle_keeps_sent` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `cmds_legacy_same` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `wake_act_once` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `bg_sync_ends_absent` | unported | seed R +-10% | `src/core/model.bend` |  |
| `bg_sync_never_adds` | unported | seed R +-10% | `src/core/model.bend` |  |
| `bg_updated_ends_not_drops` | unported | seed R +-10% | `src/core/model.bend` |  |
| `bg_notification_drops` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `bg_empty_snapshot_keeps_pending` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `bg_turn_end_settles_ended` | unported | seed R +-10% | `src/core/model.bend` |  |
| `workset_fold_same_as_setting` | unported | seed R +-10% | `src/core/model.bend` |  |
| `store_workset_encoded` | bend | seed K +-10% | `src/core/codec.bend` |  |
| `far_workset_shared` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_workset_mapped` | unported | seed R +-10% | `src/core/model.bend` |  |
| `far_forget_drops_work` | unported | seed R +-10% | `src/core/model.bend` |  |
| `bg_progress_coalesced` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `bg_progress_one_held_per_task` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `bg_progress_beat_waits` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `bg_progress_latest_lands` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `bg_progress_is_own_line` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `far_forget_equiv` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_forget_equiv_none` | bend | seed K +-10% | `src/core/far.bend` |  |
| `prompt_background_work` | bend | seed K +-10% | `src/core/prompt.bend` |  |
| `work_busy_counts_ended_rows` | unported | seed R +-10% | `src/core/model.bend` |  |
| `work_ready_idle` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `work_not_ready_busy` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `notice_decide_w_idle_same` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `notice_busy_quiet` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `notice_busy_fail_alerts` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `notice_busy_ask_kept` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `wait_busy_not_done` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `wait_idle_done` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `settle_waits_for_work` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `mcp_phase_monitoring` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `mcp_phase_complete` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `wake_says_killed` | unported | seed R +-10% | `src/core/json.bend` |  |
| `wake_completed_plain` | unported | seed R +-10% | `src/core/json.bend` |  |
| `settle_without_work` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `life_working_held` | unported | seed R +-10% | `src/core/model.bend` |  |
| `life_failed_held` | unported | seed R +-10% | `src/core/model.bend` |  |
| `life_pinned_held` | unported | seed R +-10% | `src/core/model.bend` |  |
| `life_brought_back_held` | unported | seed R +-10% | `src/core/model.bend` |  |
| `life_wake_needs_start` | unported | seed R +-10% | `src/core/model.bend` |  |
| `life_wake_needs_work` | unported | seed R +-10% | `src/core/model.bend` |  |
| `life_woke_clears_override` | unported | seed R +-10% | `src/core/model.bend` |  |
| `wt_again_recent_turn` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `wt_again_inflight` | unported | seed R +-10% | `src/core/model.bend` |  |
| `wt_again_expires` | unported | seed R +-10% | `src/core/model.bend` |  |
| `wt_again_answered` | unported | seed R +-10% | `src/core/model.bend` |  |
| `wt_first_waits` | unported | seed R +-10% | `src/core/model.bend` |  |
| `life_finished_settles` | unported | seed R +-10% | `src/core/model.bend` |  |
| `life_owned_work_held` | unported | seed R +-10% | `src/core/model.bend` |  |
| `life_owned_work_done` | unported | seed R +-10% | `src/core/model.bend` |  |
| `life_new_turn_wakes` | unported | seed R +-10% | `src/core/model.bend` |  |
| `life_duplicate_quiet` | unported | seed R +-10% | `src/core/model.bend` |  |
| `life_message_wakes` | unported | seed R +-10% | `src/core/model.bend` |  |
| `life_reply_held` | unported | seed R +-10% | `src/core/model.bend` |  |
| `life_manual_settle_held` | unported | seed R +-10% | `src/core/model.bend` |  |
| `life_off_by_default` | unported | seed R +-10% | `src/core/model.bend` |  |
| `life_off_never_done` | unported | seed R +-10% | `src/core/model.bend` |  |
| `life_off_finished_stays` | unported | seed R +-10% | `src/core/model.bend` |  |
| `life_off_owned_work_done_stays` | unported | seed R +-10% | `src/core/model.bend` |  |
| `life_off_tick_stays` | unported | seed R +-10% | `src/core/model.bend` |  |
| `life_off_window_settles` | unported | seed R +-10% | `src/core/model.bend` |  |
| `life_on_tick_settles` | unported | seed R +-10% | `src/core/model.bend` |  |
| `life_off_message_wakes` | unported | seed R +-10% | `src/core/model.bend` |  |
| `settle_idle_completed` | unported | seed R +-10% | `src/core/model.bend` |  |
| `settle_idle_failed` | unported | seed R +-10% | `src/core/model.bend` |  |
| `settle_idle_interrupted` | unported | seed R +-10% | `src/core/model.bend` |  |
| `settle_idle_not_before` | unported | seed R +-10% | `src/core/model.bend` |  |
| `settle_idle_running_stays` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `settle_idle_off` | unported | seed R +-10% | `src/core/model.bend` |  |
| `settle_idle_queued_stays` | unported | seed R +-10% | `src/core/model.bend` |  |
| `settle_idle_own_stays` | unported | seed R +-10% | `src/core/model.bend` |  |
| `settle_idle_agent_launch_stays` | unported | seed R +-10% | `src/core/model.bend` |  |
| `settle_idle_bot_delegated` | unported | seed R +-10% | `src/core/model.bend` |  |
| `settle_idle_ephemeral_queue_goes` | unported | seed R +-10% | `src/core/model.bend` |  |
| `settle_idle_message_reopens` | unported | seed R +-10% | `src/core/model.bend` |  |
| `reset_words_none` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `pending_due_passed` | bend | seed K +-10% | `src/core/fallback.bend` |  |
| `pending_due_ahead` | bend | seed K +-10% | `src/core/fallback.bend` |  |
| `pending_due_unknown` | bend | seed K +-10% | `src/core/fallback.bend` |  |
| `scratch_busy_kept` | unported | seed R +-10% | `src/core/model.bend` |  |
| `worktree_busy_kept` | unported | seed R +-10% | `src/core/model.bend` |  |
| `island_busy_shows` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `island_gone_hidden` | unported | seed R +-10% | `src/core/model.bend` |  |
| `island_unheard_empty` | unported | seed R +-10% | `src/core/model.bend` |  |
| `island_heard_online` | unported | none | `-` |  |
| `island_unheard_late` | unported | none | `-` |  |
| `island_unheard_recent` | unported | none | `-` |  |
| `island_lines_capped` | unported | seed R +-10% | `src/core/json.bend` |  |
| `upload_thread_local` | client | seed C +-10% | `src/core/client.bend` |  |
| `upload_thread_plain` | client | seed C +-10% | `src/core/client.bend` |  |
| `task_waits_for_child_work` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `task_ends_when_ready` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `task_ended_last_change` | unported | seed R +-10% | `src/core/model.bend` |  |
| `task_failed_ends_even_busy` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `bot_reply_captured_once` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `bot_reply_waits_for_work` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `phase_failed_before_monitoring` | unported | seed R +-10% | `src/core/model.bend` |  |
| `status_wait_f_keeps_failed` | unported | seed R +-10% | `src/core/model.bend` |  |
| `commit_text_joins` | bend | seed K +-10% | `src/core/store.bend` |  |
| `task_finish_delivered_last` | unported | seed R +-10% | `src/core/model.bend` |  |
| `task_repair_delivers` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `task_repair_delivers_once` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `task_repair_activates_idle` | unported | seed R +-10% | `src/core/model.bend` |  |
| `task_repair_running_marks_only` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `task_repair_quiet_when_done` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `store_torn_tail_resyncs` | bend | seed K +-10% | `src/core/store.bend` |  |
| `store_torn_tail_newline` | bend | seed K +-10% | `src/core/store.bend` |  |
| `store_fail_names_repair` | bend | seed K +-10% | `src/core/store.bend` |  |
| `task_repair_finishes_ended_child` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `task_repair_leaves_working_child` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `recover_lost_told` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `recover_lost_clears` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `recover_no_work_quiet` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `recover_lost_once` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `lost_off_logs_and_keeps` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `gone_idle_told` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `gone_running_told` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `replaced_told_once` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `replaced_keeps_new_rows` | unported | seed R +-10% | `src/core/model.bend` |  |
| `carry_keeps_child_task` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `carry_off_fails_child_task` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `carry_child_result_once` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `rec_burst_bounded` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `rec_release_bounded` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `rec_release_carry_marker_on` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `rec_release_carry_marker_off` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `rec_release_takes_held` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `lost_text_not_carry_text` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `update_wait_busy` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `update_wait_idle` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `update_wait_goes_when_done` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `update_wait_holds_while_busy` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `update_ready_holds_busy` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `update_ready_idle_restarts` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `update_ready_forced_restarts` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `update_held_goes_when_done` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `update_held_waits_while_busy` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `update_wait_late_forces` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `update_wait_early_not_forced` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `kill_never_low_pid` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `spawn_gen_accepts_current` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `spawn_gen_drops_stale` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `stale_spawn_fail_quiet` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `mcp_stale_gen_refused` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `mcp_stale_gen_reads` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `prep_stale_dropped` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `gen_mcp_path` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `quiet_once` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `quiet_never_when_busy` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `quiet_never_idle` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `exit_stale_gen_quiet` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `final_partial_line_flushed` | bend | seed K +-10% | `src/core/utf8.bend` |  |
| `watch_intent_before_launch` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `watch_end_once` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `watch_end_wakes_idle` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `watch_end_delivers_running` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `watch_timeout_unknown_not_success` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `watch_pid_identity_unknown` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `watch_cancel_keeps_until_confirmed` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `watch_cancel_confirmed` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `watch_end_retired_silent` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `watch_retire_unwatches` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `watch_target_roundtrip` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `watch_target_one_line` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `watch_target_old_row_as_written` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `watch_unwatch_says_may_run` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `recover_rearms_watches` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `mirror_never_awaits` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `prompt_background_work_b` | bend | seed K +-10% | `src/core/prompt.bend` |  |
| `credit_drop_over_margin` | unported | seed R +-10% | `src/core/outbox.bend` |  |
| `credit_behind_catches` | unported | seed R +-10% | `src/core/outbox.bend` |  |
| `credit_stale_gen_quiet` | unported | seed R +-10% | `src/core/outbox.bend` |  |
| `credit_drained_counts` | unported | seed R +-10% | `src/core/outbox.bend` |  |
| `credit_drained_never_back` | unported | seed R +-10% | `src/core/outbox.bend` |  |
| `boot_room_after_drain` | unported | seed R +-10% | `src/core/outbox.bend` |  |
| `outbox_fifo` | unported | seed R +-10% | `src/core/outbox.bend` |  |
| `join_window_same` | unported | seed R +-10% | `src/core/outbox.bend` |  |
| `join_chunks_cover` | unported | seed R +-10% | `src/core/outbox.bend` |  |
| `join_chunks_flat` | unported | seed R +-10% | `src/core/outbox.bend` |  |
| `join_oversized_alone` | unported | seed R +-10% | `src/core/outbox.bend` |  |
| `items_one_frame` | unported | seed R +-10% | `src/core/outbox.bend` |  |
| `items_split_by_bytes` | unported | seed R +-10% | `src/core/outbox.bend` |  |
| `items_split_keeps_boot` | unported | seed R +-10% | `src/core/outbox.bend` |  |
| `items_split_marks_more` | unported | seed R +-10% | `src/core/outbox.bend` |  |
| `items_whole_not_marked` | unported | seed R +-10% | `src/core/outbox.bend` |  |
| `store_hold_len` | bend | seed K +-10% | `src/core/store.bend` |  |
| `store_hold_err` | bend | seed K +-10% | `src/core/store.bend` |  |
| `store_trunc_args` | bend | seed K +-10% | `src/core/store.bend` |  |
| `ui_chunked_join_equals_whole` | client | seed C +-10% | `src/core/client.bend` |  |
| `boot_on_until_last` | client | seed C +-10% | `src/core/client.bend` |  |
| `boot_no_alerts` | unported | seed R +-10% | `src/core/model.bend` |  |
| `boot_done_alerts` | unported | seed R +-10% | `src/core/model.bend` |  |
| `boot_catchup_no_alerts` | client | seed C +-10% | `src/core/client.bend` |  |
| `pong_rate_bounded` | unported | seed R +-10% | `src/core/outbox.bend` |  |
| `wire_all_bin_eq` | bend | seed K +-10% | `src/core/http.bend` |  |
| `live_snapshot_replaces` | unported | seed R +-10% | `src/core/json.bend` |  |
| `live_snapshot_idempotent` | client | seed C +-10% | `src/core/client.bend` |  |
| `live_snapshot_clears_absent` | unported | seed R +-10% | `src/core/json.bend` |  |
| `live_end_clears` | bend | seed K +-10% | `src/core/live.bend` |  |
| `live_turn_end_clears` | bend | seed K +-10% | `src/core/live.bend` |  |
| `live_joined_exact` | bend | seed K +-10% | `src/core/live.bend` |  |
| `live_old_gen_quiet` | client | seed C +-10% | `src/core/client.bend` |  |
| `live_trunc_marked` | client | seed C +-10% | `src/core/client.bend` |  |
| `want_new_ignores_other_threads` | client | seed C +-10% | `src/core/client.bend` |  |
| `made_selects_own` | client | seed C +-10% | `src/core/client.bend` |  |
| `made_selects_own_later` | client | seed C +-10% | `src/core/client.bend` |  |
| `replay_cannot_select` | client | seed C +-10% | `src/core/client.bend` |  |
| `made_stale_quiet` | client | seed C +-10% | `src/core/client.bend` |  |
| `made_fork_selects_own` | client | seed C +-10% | `src/core/client.bend` |  |
| `made_waiting_at_once` | client | seed C +-10% | `src/core/client.bend` |  |
| `made_waiting_ends_made` | client | seed C +-10% | `src/core/client.bend` |  |
| `made_waiting_ends_failed` | unported | seed R +-10% | `src/core/json.bend` |  |
| `far_create_routes` | unported | seed R +-10% | `src/core/json.bend` |  |
| `far_fork_routes` | unported | seed R +-10% | `src/core/json.bend` |  |
| `far_create_ids_local` | unported | seed R +-10% | `src/core/json.bend` |  |
| `far_machine_routes` | unported | seed R +-10% | `src/core/json.bend` |  |
| `far_machine_none_here` | unported | seed R +-10% | `src/core/json.bend` |  |
| `far_create_in_shared_project` | unported | seed R +-10% | `src/core/json.bend` |  |
| `far_create_not_in_unshared_project` | unported | seed R +-10% | `src/core/json.bend` |  |
| `far_create_not_in_a_mirror` | unported | seed R +-10% | `src/core/json.bend` |  |
| `far_pin_only_shared_thread` | unported | seed R +-10% | `src/core/json.bend` |  |
| `far_settings_not_remote` | unported | seed R +-10% | `src/core/json.bend` |  |
| `far_reply_names_thread` | unported | seed R +-10% | `src/core/json.bend` |  |
| `far_pin_shared` | unported | seed R +-10% | `src/core/model.bend` |  |
| `made_far_waits_for_mirror` | client | seed C +-10% | `src/core/client.bend` |  |
| `made_far_selects_when_mirrored` | client | seed C +-10% | `src/core/client.bend` |  |
| `made_far_done_when_mirrored` | client | seed C +-10% | `src/core/client.bend` |  |
| `made_far_failed_not_waiting` | unported | seed R +-10% | `src/core/json.bend` |  |
| `proj_machine_cycle_ends_here` | client | seed C +-10% | `src/core/client.bend` |  |
| `proj_machine_cycle_next` | client | seed C +-10% | `src/core/client.bend` |  |
| `proj_on_request_names_machine` | unported | seed R +-10% | `src/core/json.bend` |  |
| `proj_here_request_unchanged` | unported | seed R +-10% | `src/core/json.bend` |  |
| `side_rows_ix_eq` | client | seed C +-10% | `src/core/client.bend` |  |
| `side_phase_ix_eq` | client | seed C +-10% | `src/core/client.bend` |  |
| `subs_count_ix_eq` | client | seed C +-10% | `src/core/client.bend` |  |
| `active_rows_ix_eq` | client | seed C +-10% | `src/core/client.bend` |  |
| `settle_ix_eq` | client | seed C +-10% | `src/core/client.bend` |  |
| `alerts_ids_same` | unported | seed R +-10% | `src/core/model.bend` |  |
| `alerts_ids_raise` | unported | seed R +-10% | `src/core/model.bend` |  |
| `far_fail_keeps_items` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_ack_removes_batch` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_ack_keeps_waiting` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_retry_order` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_retry_begins` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_hold_waits` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_release_resends` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_backoff` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_epoch_on_unknown_lines` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_epoch_view_bumped` | bend | seed K +-10% | `src/core/far.bend` |  |
| `hb_shows_nothing` | unported | seed R +-10% | `src/core/json.bend` |  |
| `hubs_redraw_hb` | unported | seed R +-10% | `src/core/json.bend` |  |
| `hb_ignored_by_fold` | client | seed C +-10% | `src/core/client.bend` |  |
| `alive_due` | bend | seed K +-10% | `src/core/desk.bend` |  |
| `claude_text_ids_distinct` | unported | seed R +-10% | `src/core/model.bend` |  |
| `bots_say_once` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `bots_post_once` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `bots_tell_once` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `exit_why_killed` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `live_split_fence_safe` | client | seed C +-10% | `src/core/client.bend` |  |
| `live_split_fence_closed` | client | seed C +-10% | `src/core/client.bend` |  |
| `live_split_none` | client | seed C +-10% | `src/core/client.bend` |  |
| `live_head_rest` | client | seed C +-10% | `src/core/client.bend` |  |
| `live_prefix_exact` | client | seed C +-10% | `src/web/dom.bend` |  |
| `live_prefix_exact_open_fence` | client | seed C +-10% | `src/web/dom.bend` |  |
| `qr_format_m` | bend | seed K +-10% | `src/core/qr.bend` |  |
| `qr_format_m1` | bend | seed K +-10% | `src/core/qr.bend` |  |
| `qr_version7_bits` | bend | seed K +-10% | `src/core/qr.bend` |  |
| `qr_version_fits` | bend | seed K +-10% | `src/core/qr.bend` |  |
| `qr_version_grows` | bend | seed K +-10% | `src/core/qr.bend` |  |
| `qr_version_none` | bend | seed K +-10% | `src/core/qr.bend` |  |
| `qr_gmul_wraps` | bend | seed K +-10% | `src/core/qr.bend` |  |
| `qr_link_escapes` | bend | seed K +-10% | `src/core/qr.bend` |  |
| `qr_link_none` | bend | seed K +-10% | `src/core/qr.bend` |  |
| `pair_qr_none` | client | seed C +-10% | `src/core/client.bend` |  |
| `onboard_steps_sound` | client | seed C +-10% | `src/core/onboard.bend` |  |
| `onboard_nav_every_step` | client | seed C +-10% | `src/core/onboard.bend` |  |
| `onboard_back_floor` | client | seed C +-10% | `src/core/onboard.bend` |  |
| `onboard_next_step` | client | seed C +-10% | `src/core/onboard.bend` |  |
| `onboard_back_step` | client | seed C +-10% | `src/core/onboard.bend` |  |
| `onboard_next_ends` | client | seed C +-10% | `src/core/onboard.bend` |  |
| `onboard_end_stays` | client | seed C +-10% | `src/core/onboard.bend` |  |
| `onboard_skip_ends` | client | seed C +-10% | `src/core/onboard.bend` |  |
| `onboard_waits` | client | seed C +-10% | `src/core/onboard.bend` |  |
| `onboard_fresh_first` | client | seed C +-10% | `src/core/onboard.bend` |  |
| `onboard_done_quiet` | client | seed C +-10% | `src/core/onboard.bend` |  |
| `onboard_auto_moved` | client | seed C +-10% | `src/core/onboard.bend` |  |
| `onboard_done_closes_auto` | client | seed C +-10% | `src/core/onboard.bend` |  |
| `onboard_hand_stays` | client | seed C +-10% | `src/core/onboard.bend` |  |
| `onboard_hand_offline` | client | seed C +-10% | `src/core/onboard.bend` |  |
| `onboard_wild_ends` | client | seed C +-10% | `src/core/onboard.bend` |  |
| `onboard_phone_no_target` | client | seed C +-10% | `src/core/onboard.bend` |  |
| `onboard_keys` | client | seed C +-10% | `src/core/onboard.bend` |  |
| `onboard_syms` | unported | seed R +-10% | `src/core/model.bend` |  |
| `onboard_unsynced_hidden` | client | seed C +-10% | `src/core/client.bend` |  |
| `onboard_synced_shows` | client | seed C +-10% | `src/core/client.bend` |  |
| `onboard_finish_sets_done` | client | seed C +-10% | `src/core/client.bend` |  |
| `onboard_elsewhere_closes` | client | seed C +-10% | `src/core/client.bend` |  |
| `onboard_reopen` | client | seed C +-10% | `src/core/client.bend` |  |
| `onboard_live_log_shows` | client | seed C +-10% | `src/core/client.bend` |  |
| `onboard_join_waits` | client | seed C +-10% | `src/core/client.bend` |  |
| `onboard_join_shows` | client | seed C +-10% | `src/core/client.bend` |  |
| `onboard_cache_unsynced` | client | seed C +-10% | `src/core/client.bend` |  |
| `onboard_phone_cache_unsynced` | client | seed C +-10% | `src/core/client.bend` |  |
| `onboard_focus_thread` | client | seed C +-10% | `src/core/client.bend` |  |
| `onboard_focus_none` | client | seed C +-10% | `src/core/client.bend` |  |
| `onboard_focus_remote` | client | seed C +-10% | `src/core/client.bend` |  |
| `onboard_focus_room` | client | seed C +-10% | `src/core/client.bend` |  |
| `onboard_focus_settings` | client | seed C +-10% | `src/core/client.bend` |  |
| `onboard_head_thread` | client | seed C +-10% | `src/core/client.bend` |  |
| `onboard_head_remote` | client | seed C +-10% | `src/core/client.bend` |  |
| `onboard_head_settings` | client | seed C +-10% | `src/core/client.bend` |  |
| `held_set_has` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `held_other_key` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `held_let_frees` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `held_empty_passes_press` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `held_empty_passes_release` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `held_none_at_first` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `held_swallowed_without_tour` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `tour_scroll_down` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `tour_scroll_down_stops` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `tour_scroll_up` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `tour_scroll_up_stops` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `tour_scroll_none` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `mate_lower_asks` | client | seed C +-10% | `src/core/client.bend` |  |
| `mate_higher_waits` | client | seed C +-10% | `src/core/client.bend` |  |
| `mate_linked_never_asks` | client | seed C +-10% | `src/core/client.bend` |  |
| `mate_off_never_asks` | client | seed C +-10% | `src/core/client.bend` |  |
| `mate_no_self_never_asks` | client | seed C +-10% | `src/core/client.bend` |  |
| `mate_unnamed_without_tailnet` | client | seed C +-10% | `src/core/client.bend` |  |
| `mate_answer_stranger` | client | seed C +-10% | `src/core/client.bend` |  |
| `mate_answer_invites` | client | seed C +-10% | `src/core/client.bend` |  |
| `mate_answer_off` | client | seed C +-10% | `src/core/client.bend` |  |
| `mate_answer_linked` | client | seed C +-10% | `src/core/client.bend` |  |
| `mate_off_hidden` | unported | seed R +-10% | `src/core/model.bend` |  |
| `mate_on_shown` | client | seed C +-10% | `src/core/client.bend` |  |
| `far_catalog_stands_in` | client | seed C +-10% | `src/core/client.bend` |  |
| `far_catalog_syncing` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_catalog_caught_up` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_catalog_here_never` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_catalog_titled` | client | seed C +-10% | `src/core/client.bend` |  |
| `far_scope_thread` | unported | seed R +-10% | `src/core/model.bend` |  |
| `far_scope_project` | unported | seed R +-10% | `src/core/model.bend` |  |
| `refs_machine_word` | bend | seed K +-10% | `src/core/refs.bend` |  |
| `refs_colon_after_stays_here` | bend | seed K +-10% | `src/core/refs.bend` |  |
| `mate_loopback_lower_asks` | client | seed C +-10% | `src/core/client.bend` |  |
| `mate_loopback_higher_waits` | client | seed C +-10% | `src/core/client.bend` |  |
| `fview_file_up` | bend | seed K +-10% | `src/core/fview.bend` |  |
| `fview_file_abs` | bend | seed K +-10% | `src/core/fview.bend` |  |
| `fview_file_hidden` | bend | seed K +-10% | `src/core/fview.bend` |  |
| `fview_file_hidden_deep` | bend | seed K +-10% | `src/core/fview.bend` |  |
| `fview_file_pick` | bend | seed K +-10% | `src/core/fview.bend` |  |
| `fview_file_ok` | bend | seed K +-10% | `src/core/fview.bend` |  |
| `fview_file_empty` | bend | seed K +-10% | `src/core/fview.bend` |  |
| `fview_dir_root` | bend | seed K +-10% | `src/core/fview.bend` |  |
| `fview_dir_up` | bend | seed K +-10% | `src/core/fview.bend` |  |
| `fview_reads_disjoint` | bend | seed K +-10% | `src/core/fview.bend` |  |
| `fview_no_open` | bend | seed K +-10% | `src/core/fview.bend` |  |
| `fview_no_pdf` | bend | seed K +-10% | `src/core/fview.bend` |  |
| `fview_no_fs` | bend | seed K +-10% | `src/core/fview.bend` |  |
| `fview_allowed_not_write` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fview_allowed_path` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fview_allowed_want` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fview_allowed_dir` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fview_cache_under` | bend | seed K +-10% | `src/core/fview.bend` |  |
| `fview_cache_dotdot` | bend | seed K +-10% | `src/core/fview.bend` |  |
| `fview_cache_link_slash` | bend | seed K +-10% | `src/core/fview.bend` |  |
| `fview_cache_link_dotdot` | bend | seed K +-10% | `src/core/fview.bend` |  |
| `fview_cache_relative` | bend | seed K +-10% | `src/core/fview.bend` |  |
| `fview_cache_of_file` | bend | seed K +-10% | `src/core/fview.bend` |  |
| `fview_cache_of_outside` | bend | seed K +-10% | `src/core/fview.bend` |  |
| `fview_cache_of_prefix` | bend | seed K +-10% | `src/core/fview.bend` |  |
| `fview_fix_thread` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fview_fix_root` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fview_fix_again` | bend | seed K +-10% | `src/core/fview.bend` |  |
| `fview_fix_leaves_error` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fmc_taken_tool` | bend | seed K +-10% | `src/core/fmcp.bend` |  |
| `fmc_only_listed_tools` | bend | seed K +-10% | `src/core/fmcp.bend` |  |
| `fmc_no_terminal_or_delegate` | bend | seed K +-10% | `src/core/fmcp.bend` |  |
| `fmc_permit_keeps_client_rules` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fmc_unshared_refused` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fmc_unshared_changes_nothing` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fmc_unshared_interrupt_refused` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fmc_plan_here_stays` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fmc_plan_wait_polls_here` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fmc_plan_unlinked_never_sent` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fmc_plan_unlinked_machine_never_sent` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fmc_plan_machine_list_answered` | bend | seed K +-10% | `src/core/fmcp.bend` |  |
| `fmc_plan_other_tools_here` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fmc_args_made_local` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fmc_args_other_link_kept` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fmc_reply_ids_named_by_link` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fmc_give_unlinked_nothing` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fmc_give_local_drops_machines` | bend | seed K +-10% | `src/core/fmcp.bend` |  |
| `flive_delta_appends` | unported | seed R +-10% | `src/core/farlive.bend` |  |
| `flive_gap_asks` | unported | seed R +-10% | `src/core/farlive.bend` |  |
| `flive_gap_changes_nothing` | unported | seed R +-10% | `src/core/farlive.bend` |  |
| `flive_end_blocks_old` | unported | seed R +-10% | `src/core/farlive.bend` |  |
| `flive_end_no_resurrect` | unported | seed R +-10% | `src/core/farlive.bend` |  |
| `flive_next_gen_taken` | unported | seed R +-10% | `src/core/farlive.bend` |  |
| `flive_end_before_any` | unported | seed R +-10% | `src/core/farlive.bend` |  |
| `flive_end_keeps_newer` | unported | seed R +-10% | `src/core/farlive.bend` |  |
| `flive_snap_replaces` | unported | seed R +-10% | `src/core/farlive.bend` |  |
| `flive_snap_older_stale` | unported | seed R +-10% | `src/core/farlive.bend` |  |
| `flive_ended_ids` | unported | seed R +-10% | `src/core/model.bend` |  |
| `flive_user_message_keeps` | unported | seed R +-10% | `src/core/farlive.bend` |  |
| `flive_body_no_items` | unported | seed R +-10% | `src/core/json.bend` |  |
| `flive_body_has_live` | unported | seed R +-10% | `src/core/json.bend` |  |
| `flive_queue_joins` | unported | seed R +-10% | `src/core/farlive.bend` |  |
| `flive_queue_gap_owes_whole` | unported | seed R +-10% | `src/core/farlive.bend` |  |
| `flive_queue_whole_absorbs` | unported | seed R +-10% | `src/core/farlive.bend` |  |
| `flive_queue_threads_apart` | unported | seed R +-10% | `src/core/farlive.bend` |  |
| `flive_one_request_out` | unported | seed R +-10% | `src/core/farlive.bend` |  |
| `flive_due_after_answer` | unported | seed R +-10% | `src/core/farlive.bend` |  |
| `flive_quiet_drops` | unported | seed R +-10% | `src/core/farlive.bend` |  |
| `flive_quiet_ends` | unported | seed R +-10% | `src/core/farlive.bend` |  |
| `flive_whole_from_buffer` | unported | seed R +-10% | `src/core/farlive.bend` |  |
| `flive_delta_ended_dropped` | unported | seed R +-10% | `src/core/farlive.bend` |  |
| `flive_whole_ended_dropped` | unported | seed R +-10% | `src/core/farlive.bend` |  |
| `flive_link_gone` | bend | seed K +-10% | `src/core/live.bend` |  |
| `flive_far_message_ends_live` | unported | seed R +-10% | `src/core/json.bend` |  |
| `flive_far_activity_keeps_live` | unported | seed R +-10% | `src/core/json.bend` |  |
| `flive_far_delta_shows` | unported | seed R +-10% | `src/core/json.bend` |  |
| `notice_far_key_same` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `notice_far_fail_key_same` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `notice_local_key_kept` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `island_far_once` | unported | seed R +-10% | `src/core/json.bend` |  |
| `island_far_owner_wins` | unported | seed R +-10% | `src/core/json.bend` |  |
| `island_far_alone_kept` | unported | seed R +-10% | `src/core/json.bend` |  |
| `rview_at_under` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_at_climbs` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_at_relative` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_back_at` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_back_outside` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_want_hidden` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_want_file` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_ask_get_up` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_ask_get_abs` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_ask_get_hidden` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_ask_get_ok` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_ask_ver_hidden` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_ask_no_folder` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_ask_other` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_git_option` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_git_dir` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_git_climbs` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_git_ok` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_git_other_repo` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_git_hidden` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_root_unknown` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_root_far_cache` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_root_far_other_link` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_root_far_climbs` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_root_own_ignores_claim` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_enc_unreserved` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_enc_breaks` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `rview_spec_local` | bend | seed K +-10% | `src/core/rview.bend` |  |
| `lyr_board_order_kicad` | bend | seed K +-10% | `src/core/layers.bend` |  |
| `lyr_plain_as_is` | bend | seed K +-10% | `src/core/layers.bend` |  |
| `lyr_dim_quarter` | bend | seed K +-10% | `src/core/layers.bend` |  |
| `lyr_only_hides` | bend | seed K +-10% | `src/core/layers.bend` |  |
| `lyr_active_shows` | bend | seed K +-10% | `src/core/layers.bend` |  |
| `lyr_copper_keeps_vias` | bend | seed K +-10% | `src/core/layers.bend` |  |
| `lyr_outline_stays` | bend | seed K +-10% | `src/core/layers.bend` |  |
| `lyr_pick_again_all` | bend | seed K +-10% | `src/core/layers.bend` |  |
| `lyr_cycle_round` | bend | seed K +-10% | `src/core/layers.bend` |  |
| `lyr_kind_round` | bend | seed K +-10% | `src/core/layers.bend` |  |
| `lyr_kind_base` | bend | seed K +-10% | `src/core/layers.bend` |  |
| `lyr_masks_only` | bend | seed K +-10% | `src/core/layers.bend` |  |
| `rv_diff_on_all` | bend | seed K +-10% | `src/core/hist.bend` |  |
| `rv_mark_on_all` | bend | seed K +-10% | `src/core/board.bend` |  |
| `rv_sheet_plain` | client | seed C +-10% | `src/core/client.bend` |  |
| `rv_sheet_compared` | bend | seed K +-10% | `src/core/hist.bend` |  |
| `rv_bits_plain` | client | seed C +-10% | `src/core/client.bend` |  |
| `bom_nat_c2_c10` | bend | seed K +-10% | `src/core/bom.bend` |  |
| `bom_nat_r10_r2` | bend | seed K +-10% | `src/core/bom.bend` |  |
| `bom_diff_self` | bend | seed K +-10% | `src/core/bom.bend` |  |
| `bom_group_qty` | bend | seed K +-10% | `src/core/bom.bend` |  |
| `bom_group_csv` | bend | seed K +-10% | `src/core/bom.bend` |  |
| `bom_csv_quote` | bend | seed K +-10% | `src/core/bom.bend` |  |
| `bom_csv_quote_inner` | bend | seed K +-10% | `src/core/bom.bend` |  |
| `bom_csv_plain` | bend | seed K +-10% | `src/core/bom.bend` |  |
| `bom_diff_line` | bend | seed K +-10% | `src/core/bom.bend` |  |
| `bom_diff_md_none` | bend | seed K +-10% | `src/core/bom.bend` |  |
| `bom_md_pipe` | bend | seed K +-10% | `src/core/bom.bend` |  |
| `cad_of_kicad_board` | bend | seed K +-10% | `src/core/cad.bend` |  |
| `cad_of_kicad_schematic` | bend | seed K +-10% | `src/core/cad.bend` |  |
| `cad_of_case_ignored` | bend | seed K +-10% | `src/core/cad.bend` |  |
| `cad_of_altium_not_drawn` | bend | seed K +-10% | `src/core/cad.bend` |  |
| `cad_of_unknown` | bend | seed K +-10% | `src/core/cad.bend` |  |
| `cad_fill_puts_vars_in` | bend | seed K +-10% | `src/core/cad.bend` |  |
| `cad_fill_keeps_other_braces` | bend | seed K +-10% | `src/core/cad.bend` |  |
| `cad_shell_quotes_a_quote` | bend | seed K +-10% | `src/core/cad.bend` |  |
| `cad_shell_words` | bend | seed K +-10% | `src/core/cad.bend` |  |
| `checklist_toggle_twice` | bend | seed K +-10% | `src/core/checks.bend` |  |
| `checklist_toggle_flips_one` | bend | seed K +-10% | `src/core/checks.bend` |  |
| `checklist_toggle_past_end` | bend | seed K +-10% | `src/core/checks.bend` |  |
| `checks_gate_no_findings` | bend | seed K +-10% | `src/core/checks.bend` |  |
| `checks_gate_no_findings_strict` | bend | seed K +-10% | `src/core/checks.bend` |  |
| `checks_md_empty` | bend | seed K +-10% | `src/core/checks.bend` |  |
| `checks_erc_error_counted` | bend | seed K +-10% | `src/core/checks.bend` |  |
| `release_next_dotted` | bend | seed K +-10% | `src/core/release.bend` |  |
| `release_next_letter` | bend | seed K +-10% | `src/core/release.bend` |  |
| `release_next_plain` | bend | seed K +-10% | `src/core/release.bend` |  |
| `release_next_first` | bend | seed K +-10% | `src/core/release.bend` |  |
| `release_tag_valid` | bend | seed K +-10% | `src/core/release.bend` |  |
| `release_tag_no_space` | bend | seed K +-10% | `src/core/release.bend` |  |
| `release_tag_no_dotdot` | bend | seed K +-10% | `src/core/release.bend` |  |
| `release_tag_no_dash_lead` | bend | seed K +-10% | `src/core/release.bend` |  |
| `release_tag_no_lock_end` | bend | seed K +-10% | `src/core/release.bend` |  |
| `release_tag_not_empty` | bend | seed K +-10% | `src/core/release.bend` |  |
| `release_dir_in_repo` | bend | seed K +-10% | `src/core/release.bend` |  |
| `release_gh_command` | bend | seed K +-10% | `src/core/release.bend` |  |
| `review_fold_empty` | bend | seed K +-10% | `src/core/review.bend` |  |
| `review_prompt_none` | bend | seed K +-10% | `src/core/review.bend` |  |
| `review_unknown_ignored` | bend | seed K +-10% | `src/core/review.bend` |  |
| `review_resolve` | bend | seed K +-10% | `src/core/review.bend` |  |
| `review_reopen` | bend | seed K +-10% | `src/core/review.bend` |  |
| `review_prompt_resolved` | bend | seed K +-10% | `src/core/review.bend` |  |
| `review_drop` | bend | seed K +-10% | `src/core/review.bend` |  |
| `review_reply_no_pin` | bend | seed K +-10% | `src/core/review.bend` |  |
| `review_reply_counted` | bend | seed K +-10% | `src/core/review.bend` |  |
| `review_orphan_ignored` | bend | seed K +-10% | `src/core/review.bend` |  |
| `review_roundtrip_resolve` | bend | seed K +-10% | `src/core/review.bend` |  |
| `review_roundtrip_edit` | bend | seed K +-10% | `src/core/review.bend` |  |
| `review_roundtrip_drop` | bend | seed K +-10% | `src/core/review.bend` |  |
| `review_other_tone` | bend | seed K +-10% | `src/core/review.bend` |  |
| `review_moved` | bend | seed K +-10% | `src/core/review.bend` |  |
| `review_not_moved` | bend | seed K +-10% | `src/core/review.bend` |  |
| `slash_typing_word` | bend | seed K +-10% | `src/core/slash.bend` |  |
| `slash_typing_alone` | bend | seed K +-10% | `src/core/slash.bend` |  |
| `slash_not_after_space` | bend | seed K +-10% | `src/core/slash.bend` |  |
| `slash_not_midline` | bend | seed K +-10% | `src/core/slash.bend` |  |
| `slash_claude_prefix` | bend | seed K +-10% | `src/core/slash.bend` |  |
| `slash_others_own_only` | bend | seed K +-10% | `src/core/slash.bend` |  |
| `slash_none_unknown` | bend | seed K +-10% | `src/core/slash.bend` |  |
| `uber_promote_far_refused` | unported | seed R +-10% | `src/core/model.bend` |  |
| `uber_demote_clears` | unported | seed R +-10% | `src/core/model.bend` |  |
| `uber_promote_sets` | unported | seed R +-10% | `src/core/model.bend` |  |
| `uber_promote_again_nothing` | unported | seed R +-10% | `src/core/model.bend` |  |
| `plane_each_once` | client | seed C +-10% | `src/core/client.bend` |  |
| `plane_sections_once` | bend | seed K +-10% | `src/core/plane.bend` |  |
| `plane_need_ranked` | bend | seed K +-10% | `src/core/plane.bend` |  |
| `plane_hit_opens` | unported | seed R +-10% | `src/core/model.bend` |  |
| `onboard_uber_none_creates` | client | seed C +-10% | `src/core/client.bend` |  |
| `onboard_uber_pick_most_used` | client | seed C +-10% | `src/core/client.bend` |  |
| `onboard_uber_have_quiet` | unported | seed R +-10% | `src/core/model.bend` |  |
| `onboard_uber_existing_shows` | client | seed C +-10% | `src/core/client.bend` |  |
| `onboard_uber_existing_cached` | client | seed C +-10% | `src/core/client.bend` |  |
| `onboard_uber_answered_quiet` | unported | seed R +-10% | `src/core/model.bend` |  |
| `onboard_uber_have_no_ask` | unported | seed R +-10% | `src/core/model.bend` |  |
| `onboard_uber_skip_answers` | client | seed C +-10% | `src/core/client.bend` |  |
| `onboard_uber_single_no_back` | client | seed C +-10% | `src/core/client.bend` |  |
| `hubs_plane_once` | unported | seed R +-10% | `src/core/json.bend` |  |
| `uber_merge_comm` | bend | seed K +-10% | `src/core/ubersync.bend` |  |
| `uber_merge_idem` | bend | seed K +-10% | `src/core/ubersync.bend` |  |
| `uber_forget_wins_older` | bend | seed K +-10% | `src/core/ubersync.bend` |  |
| `uber_forget_loses_newer` | bend | seed K +-10% | `src/core/ubersync.bend` |  |
| `uber_one_holder_per_term` | bend | seed K +-10% | `src/core/ubersync.bend` |  |
| `uber_higher_term_wins` | bend | seed K +-10% | `src/core/ubersync.bend` |  |
| `uber_fire_holder_only` | bend | seed K +-10% | `src/core/ubersync.bend` |  |
| `uber_pinned_stays` | bend | seed K +-10% | `src/core/ubersync.bend` |  |
| `uber_slot_once` | bend | seed K +-10% | `src/core/ubersync.bend` |  |
| `uber_fire_state` | bend | seed K +-10% | `src/core/ubersync.bend` |  |
| `uber_fire_state_other` | bend | seed K +-10% | `src/core/ubersync.bend` |  |
| `uber_slot_once_state` | bend | seed K +-10% | `src/core/ubersync.bend` |  |
| `uber_pinned_state` | bend | seed K +-10% | `src/core/ubersync.bend` |  |
| `uber_derive_n_no_ops` | bend | seed K +-10% | `src/core/ubersync.bend` |  |
| `uber_derived_no_ops` | bend | seed K +-10% | `src/core/ubersync.bend` |  |
| `uber_superseded_nop` | bend | seed K +-10% | `src/core/ubop.bend` |  |
| `uber_superseded_keeps` | bend | seed K +-10% | `src/core/ubersync.bend` |  |
| `uber_seed_once` | bend | seed K +-10% | `src/core/ubersync.bend` |  |
| `side_work_ix_eq` | client | seed C +-10% | `src/core/client.bend` |  |
| `plane_tally_counts` | bend | seed K +-10% | `src/core/plane.bend` |  |
| `plane_tally_counts_rich` | bend | seed K +-10% | `src/core/plane.bend` |  |
| `plane_text_need_first` | bend | seed K +-10% | `src/core/plane.bend` |  |
| `plane_text_need_first_rich` | bend | seed K +-10% | `src/core/plane.bend` |  |
| `fmc_plan_plane_read_here` | bend | seed K +-10% | `src/core/fmcp.bend` |  |
| `fmc_plan_plane_read_unlinked_never_sent` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fmc_plane_read_allowed` | bend | seed K +-10% | `src/core/fmcp.bend` |  |
| `uber_home_takes_term` | bend | seed K +-10% | `src/core/ubersync.bend` |  |
| `uber_home_holder_nothing` | bend | seed K +-10% | `src/core/ubersync.bend` |  |
| `plane_home_button` | client | seed C +-10% | `src/core/client.bend` |  |
| `plane_home_button_held` | client | seed C +-10% | `src/core/client.bend` |  |
| `plane_list_same` | bend | seed K +-10% | `src/core/plane.bend` |  |
| `plane_list_same_rich` | bend | seed K +-10% | `src/core/plane.bend` |  |
| `plane_launch_once` | bend | seed K +-10% | `src/core/plane.bend` |  |
| `plane_counts_split` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `launch_reports_turn_end` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `launch_ask_at_once` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `launch_fail_told` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `launch_once_per_end` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `launch_muted_quiet` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `launch_waits_for_work` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `launch_said_quiet` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `launch_own_stop_quiet` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `launch_stop_told` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `launch_delivered_in_order` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `launch_loop_bounded` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `launch_far_once` | bend | seed K +-10% | `src/core/fmcp.bend` |  |
| `launch_far_order` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `launch_list_once` | unported | seed R +-10% | `src/core/model.bend` |  |
| `launched_up_launcher` | client | seed C +-10% | `src/core/client.bend` |  |
| `subs_launched_live_kept` | client | seed C +-10% | `src/core/subs.bend` |  |
| `notice_report_quiet` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `launch_send_launcher` | unported | seed R +-10% | `src/core/model.bend` |  |
| `interrupt_antigravity_stops` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `antigravity_run_turn` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `agy_mode_supervised_asks` | bend | seed K +-10% | `src/core/agy.bend` |  |
| `agy_mode_full_allows` | bend | seed K +-10% | `src/core/agy.bend` |  |
| `agy_default_api_key` | bend | seed K +-10% | `src/core/agy.bend` |  |
| `agy_google_warns` | bend | seed K +-10% | `src/core/agy.bend` |  |
| `agy_callback_needs_signin` | bend | seed K +-10% | `src/core/agy.bend` |  |
| `mnav_bot_sel` | client | seed C +-10% | `src/core/botui.bend` |  |
| `mnav_room_local` | unported | none | `-` |  |
| `mnav_remote_local` | unported | none | `-` |  |
| `mnav_bot_thread` | client | seed C +-10% | `src/core/botui.bend` |  |
| `policy_no_alert_unless` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `policy_settle_once` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `policy_settle_after_launch` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `policy_fail_person_only` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `policy_ask_alerts` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `policy_ask_alerted_out` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `policy_ask_off_silent` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `policy_merge_alerts` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `policy_end_alerted` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `policy_setting_off` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `policy_all_legacy` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `policy_link_not_person` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `policy_person_direct` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `notify_refuses_chatter` | unported | seed R +-10% | `src/core/bothub.bend` |  |
| `notify_reminder_routine_only` | unported | seed R +-10% | `src/core/bothub.bend` |  |
| `notify_needs_you_once` | unported | seed R +-10% | `src/core/bothub.bend` |  |
| `notify_done_settles` | unported | seed R +-10% | `src/core/bothub.bend` |  |
| `notify_setting_off` | unported | seed R +-10% | `src/core/bothub.bend` |  |
| `desk_redraw_not_input` | window | seed D +-10% | `src/app/win.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `desk_key_input` | window | seed D +-10% | `src/app/win.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `policy_phone_raises` | client | seed C +-10% | `src/core/client.bend` |  |
| `policy_phone_same_none` | client | seed C +-10% | `src/core/client.bend` |  |
| `policy_phone_empty_none` | client | seed C +-10% | `src/core/client.bend` |  |
| `policy_phone_unseen_none` | client | seed C +-10% | `src/core/client.bend` |  |
| `policy_phone_watched_held` | client | seed C +-10% | `src/core/client.bend` |  |
| `policy_phone_note_not_held` | client | seed C +-10% | `src/core/client.bend` |  |
| `policy_phone_ids_record` | unported | seed R +-10% | `src/core/json.bend` |  |
| `policy_phone_frame_raises` | unported | seed R +-10% | `src/core/model.bend` |  |
| `policy_phone_turn_end_quiet` | unported | seed R +-10% | `src/core/model.bend` |  |
| `policy_phone_boot_quiet` | client | seed C +-10% | `src/core/client.bend` |  |
| `policy_reminder_alerts` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `notify_rows_defaults` | client | seed C +-10% | `src/core/client.bend` |  |
| `island_compact_one_line` | unported | seed R +-10% | `src/core/json.bend` |  |
| `island_no_children` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `island_line_last_sent` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `clipath_keeps_current` | bend | seed K +-10% | `src/core/clipath.bend` |  |
| `clipath_no_dupes` | bend | seed K +-10% | `src/core/clipath.bend` |  |
| `clipath_idempotent` | bend | seed K +-10% | `src/core/clipath.bend` |  |
| `clipath_nvm_newest_first` | bend | seed K +-10% | `src/core/clipath.bend` |  |
| `clipath_windows_missing_probe` | bend | seed K +-10% | `src/core/clipath.bend` |  |
| `clipath_windows_claude` | bend | seed K +-10% | `src/core/clipath.bend` |  |
| `wake_once_per_event` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `wake_once_drops` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `wake_two_events_one_message` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `wake_not_self` | bend | seed K +-10% | `src/core/wake.bend` |  |
| `wake_bot_thread_delivered` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `wake_pr_merged` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `wake_pr_repeat_quiet` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `codec_wake_roundtrip` | unported | seed R +-10% | `src/core/model.bend` |  |
| `codec_wake_dropped_roundtrip` | unported | seed R +-10% | `src/core/model.bend` |  |
| `wake_message_one_paragraph` | bend | seed K +-10% | `src/core/wake.bend` |  |
| `wake_event_independent_of_step` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `told_setting_empty_removes` | unported | seed R +-10% | `src/core/model.bend` |  |
| `told_mark_key_norm` | bend | seed K +-10% | `src/core/told.bend` |  |
| `told_check_finds_marked` | bend | seed K +-10% | `src/core/told.bend` |  |
| `told_check_text_overlap` | bend | seed K +-10% | `src/core/told.bend` |  |
| `told_prune_old` | bend | seed K +-10% | `src/core/told.bend` |  |
| `told_quiet_end_marks_nothing` | bend | seed K +-10% | `src/core/told.bend` |  |
| `rules_block_empty_when_none` | bend | seed K +-10% | `src/core/rules.bend` |  |
| `rules_block_scoped` | bend | seed K +-10% | `src/core/rules.bend` |  |
| `rules_refuse_enforced` | bend | seed K +-10% | `src/core/rules.bend` |  |
| `rules_word_unlocks` | bend | seed K +-10% | `src/core/rules.bend` |  |
| `rules_mcp_prefix` | bend | seed K +-10% | `src/core/rules.bend` |  |
| `rules_off_ignored` | bend | seed K +-10% | `src/core/rules.bend` |  |
| `tb_order_open_first` | bend | seed K +-10% | `src/core/tboard.bend` |  |
| `tb_import_checkbox` | bend | seed K +-10% | `src/core/tboard.bend` |  |
| `tb_import_idempotent` | bend | seed K +-10% | `src/core/tboard.bend` |  |
| `tb_update_merges` | bend | seed K +-10% | `src/core/tboard.bend` |  |
| `tb_due_shape` | bend | seed K +-10% | `src/core/tboard.bend` |  |
| `told_report_quiet` | bend | seed K +-10% | `src/core/told.bend` |  |
| `rules_report_never_unlocks` | bend | seed K +-10% | `src/core/rules.bend` |  |
| `rules_link_never_unlocks` | bend | seed K +-10% | `src/core/rules.bend` |  |
| `told_note_marked` | bend | seed K +-10% | `src/core/told.bend` |  |
| `told_quiet_record_nothing` | unported | seed R +-10% | `src/core/model.bend` |  |
| `deploy_waits_for_tool` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `deploy_waits_for_lock` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `deploy_goes_when_clear` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `deploy_go_clears_wait` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `deploy_go_writes_done` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `deploy_idle_nothing` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `deploy_after_note_once` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `deploy_after_clears` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `deploy_after_second_start_quiet` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `deploy_requester_bot_thread` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `deploy_cut_requester_no_second` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `deploy_cancel_clears` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `exe_path_plain` | bend | seed K +-10% | `src/core/exe.bend` |  |
| `exe_path_deleted` | bend | seed K +-10% | `src/core/exe.bend` |  |
| `exe_child_plain` | bend | seed K +-10% | `src/core/exe.bend` |  |
| `exe_child_deleted` | bend | seed K +-10% | `src/core/exe.bend` |  |
| `exe_gone_plain` | bend | seed K +-10% | `src/core/exe.bend` |  |
| `exe_gone_deleted` | bend | seed K +-10% | `src/core/exe.bend` |  |
| `lock_exclusive` | bend | seed K +-10% | `src/core/locks.bend` |  |
| `lock_fifo` | bend | seed K +-10% | `src/core/locks.bend` |  |
| `lock_holder_only` | bend | seed K +-10% | `src/core/locks.bend` |  |
| `lock_release_next` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `lock_deleted_holder_frees` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `lock_work_releases` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `lock_grant_once` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `lock_ready_idempotent` | bend | seed K +-10% | `src/core/locks.bend` |  |
| `codec_lock_roundtrip` | bend | seed K +-10% | `src/core/locks.bend` |  |
| `lock_status_requeues_lost_client` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `lock_renew_requeues_lost_client` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `lock_requeue_keeps_fifo_for_others` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `lock_restart_extends_client_leases` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `cal_patch_only_given` | unported | seed R +-10% | `src/core/json.bend` |  |
| `cal_update_asks` | bend | seed K +-10% | `src/core/google.bend` |  |
| `cal_delete_asks` | bend | seed K +-10% | `src/core/google.bend` |  |
| `cal_id_no_slash` | bend | seed K +-10% | `src/core/google.bend` |  |
| `tz_fixed_wall` | bend | seed K +-10% | `src/core/tz.bend` |  |
| `sched_done_never` | bend | seed K +-10% | `src/core/bot.bend` |  |
| `sched_once_due` | bend | seed K +-10% | `src/core/bot.bend` |  |
| `sched_legacy_same` | bend | seed K +-10% | `src/core/bot.bend` |  |
| `sched_zoned_once_per_slot` | bend | seed K +-10% | `src/core/tz.bend` |  |
| `when_in_minutes` | bend | seed K +-10% | `src/core/tz.bend` |  |
| `when_in_hours` | bend | seed K +-10% | `src/core/tz.bend` |  |
| `task_secret_never_stored` | bend | seed K +-10% | `src/core/task.bend` |  |
| `task_risky_needs_confirm` | bend | seed K +-10% | `src/core/task.bend` |  |
| `run_confirm_runs` | bend | seed K +-10% | `src/core/task.bend` |  |
| `run_stuck_alerts_once` | bend | seed K +-10% | `src/core/task.bend` |  |
| `run_pause_alerts` | bend | seed K +-10% | `src/core/task.bend` |  |
| `run_stop_ends` | bend | seed K +-10% | `src/core/task.bend` |  |
| `drive_person_locks_bot` | bend | seed K +-10% | `src/core/task.bend` |  |
| `task_slug_nonempty` | bend | seed K +-10% | `src/core/task.bend` |  |
| `task_slug_empty` | bend | seed K +-10% | `src/core/task.bend` |  |
| `gmail_reply_subject_adds_re` | bend | seed K +-10% | `src/core/google.bend` |  |
| `gmail_reply_subject_keeps_re` | bend | seed K +-10% | `src/core/google.bend` |  |
| `gmail_references_appends` | bend | seed K +-10% | `src/core/google.bend` |  |
| `gmail_reply_to_prefers_reply_to` | bend | seed K +-10% | `src/core/google.bend` |  |
| `gmail_mime_type_pdf` | bend | seed K +-10% | `src/core/google.bend` |  |
| `gmail_mime_type_default` | bend | seed K +-10% | `src/core/google.bend` |  |
| `gmail_signed_plain` | bend | seed K +-10% | `src/core/google.bend` |  |
| `gmail_signed_html_escapes` | bend | seed K +-10% | `src/core/google.bend` |  |
| `gmail_signed_empty_unchanged` | bend | seed K +-10% | `src/core/google.bend` |  |
| `gmail_draft_no_ask` | bend | seed K +-10% | `src/core/google.bend` |  |
| `gmail_size_kb` | bend | seed K +-10% | `src/core/google.bend` |  |
| `gmail_review_omits_empty_cc` | bend | seed K +-10% | `src/core/google.bend` |  |
| `ask_head_gmail` | client | seed C +-10% | `src/core/client.bend` |  |
| `ask_head_draft` | client | seed C +-10% | `src/core/client.bend` |  |
| `ask_head_other_tool` | client | seed C +-10% | `src/core/client.bend` |  |
| `ask_head_cal_update` | client | seed C +-10% | `src/core/client.bend` |  |
| `tool_open_shut_pair` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `tool_turn_end_clears` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `deploy_goes_between_calls` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `deploy_waits_open_call` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `lostwork_exit_clears` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `lostwork_exit_clears_off` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `lostwork_exit_clears_running` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `lostwork_exit_keeps_child` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `lostwork_exit_keeps_watch` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `lostwork_exit_island_hides` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `lostwork_recover_none_open` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `lostwork_recover_none_open_off` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `lostwork_recover_none_held` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `lostwork_recover_none_held_carried` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `lostwork_recover_island_hides` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `lostwork_recover_still_told` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `lostwork_recover_nothing_left_aside` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `lostwork_recover_old_rows_go` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `lostwork_recover_old_rows_go_off` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `teach_mode_idle` | client | seed C +-10% | `src/core/client.bend` |  |
| `teach_mode_recording` | client | seed C +-10% | `src/core/client.bend` |  |
| `teach_mode_review` | client | seed C +-10% | `src/core/client.bend` |  |
| `teach_mode_takeover` | client | seed C +-10% | `src/core/client.bend` |  |
| `teach_input_only_when_driving` | client | seed C +-10% | `src/core/client.bend` |  |
| `teach_bot_drives_no_input` | client | seed C +-10% | `src/core/client.bend` |  |
| `teach_risky_confirms` | client | seed C +-10% | `src/core/client.bend` |  |
| `teach_plain_continues` | client | seed C +-10% | `src/core/client.bend` |  |
| `teach_risky_default_options` | client | seed C +-10% | `src/core/client.bend` |  |
| `teach_risky_button` | client | seed C +-10% | `src/core/client.bend` |  |
| `teach_secret_no_chip` | client | seed C +-10% | `src/core/client.bend` |  |
| `teach_secret_row_no_chip` | client | seed C +-10% | `src/core/client.bend` |  |
| `teach_tasks_tab` | client | seed C +-10% | `src/core/botui.bend` |  |
| `teach_clock` | client | seed C +-10% | `src/core/client.bend` |  |
| `teach_preview_refused_teaching` | bend | seed K +-10% | `src/core/teach.bend` |  |
| `teach_preview_refused_takeover` | bend | seed K +-10% | `src/core/teach.bend` |  |
| `teach_preview_other_bot_free` | bend | seed K +-10% | `src/core/teach.bend` |  |
| `teach_preview_allowed_idle` | bend | seed K +-10% | `src/core/teach.bend` |  |
| `teach_handback_returns_browser` | bend | seed K +-10% | `src/core/teach.bend` |  |
| `teach_info_idle` | bend | seed K +-10% | `src/core/teach.bend` |  |
| `teach_info_recording` | bend | seed K +-10% | `src/core/teach.bend` |  |
| `teach_info_review` | bend | seed K +-10% | `src/core/teach.bend` |  |
| `teach_pause_options_risky` | bend | seed K +-10% | `src/core/teach.bend` |  |
| `teach_pause_options_plain` | bend | seed K +-10% | `src/core/teach.bend` |  |
| `teach_scale_corner` | bend | seed K +-10% | `src/core/task.bend` |  |
| `teach_scale_middle` | bend | seed K +-10% | `src/core/task.bend` |  |
| `teach_run_no_slash` | bend | seed K +-10% | `src/core/teach.bend` |  |
| `teach_run_no_dots` | bend | seed K +-10% | `src/core/teach.bend` |  |
| `teach_file_no_traversal` | bend | seed K +-10% | `src/core/teach.bend` |  |
| `teach_run_slug_roundtrip` | bend | seed K +-10% | `src/core/teach.bend` |  |
| `teach_recording_expires` | bend | seed K +-10% | `src/core/teach.bend` |  |
| `teach_recording_not_early` | bend | seed K +-10% | `src/core/teach.bend` |  |
| `teach_draft_secret_no_value` | unported | seed R +-10% | `src/core/json.bend` |  |
| `mobile_browser_ask_card` | client | seed C +-10% | `src/core/client.bend` |  |
| `mobile_other_ask_standard` | client | seed C +-10% | `src/mobile/screen.bend` |  |
| `mobile_browser_card_only_browser` | unported | seed R +-10% | `src/core/json.bend` |  |
| `local_view_wins` | client | seed C +-10% | `src/mobile/screen.bend` |  |
| `local_view_wins_projects` | client | seed C +-10% | `src/mobile/screen.bend` |  |
| `local_view_unset_hub` | client | seed C +-10% | `src/mobile/screen.bend` |  |
| `local_view_unset_hub_projects` | client | seed C +-10% | `src/mobile/screen.bend` |  |
| `local_view_junk_ignored` | client | seed C +-10% | `src/mobile/screen.bend` |  |
| `hub_settings_moves_focus` | unported | none | `-` |  |
| `busy_stream_inert` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `busy_stream_keeps_count` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `busy_demo_count` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `busy_turn_moves` | unported | seed R +-10% | `src/core/model.bend` |  |
| `busy_rows_move` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `commit_line_is_wire` | unported | seed R +-10% | `src/core/model.bend` |  |
| `commit_line_device_hides_token` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `input_wire_blank` | bend | seed K +-10% | `src/core/codec.bend` |  |
| `input_wire_other_kept` | bend | seed K +-10% | `src/core/codec.bend` |  |
| `input_commit_line_is_wire` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `input_replayed_line_is_wire` | unported | seed R +-10% | `src/core/json.bend` |  |
| `input_store_wire` | unported | seed R +-10% | `src/core/model.bend` |  |
| `input_blank_removes` | unported | seed R +-10% | `src/core/model.bend` |  |
| `input_launch_falls_back` | unported | seed R +-10% | `src/core/model.bend` |  |
| `input_prune_rest` | unported | seed R +-10% | `src/core/model.bend` |  |
| `input_blank_scoped` | unported | seed R +-10% | `src/core/model.bend` |  |
| `input_blank_scoped_value` | unported | seed R +-10% | `src/core/model.bend` |  |
| `caret_keep` | window | seed D +-10% | `src/gfx/raster.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `caret_loc` | window | seed D +-10% | `src/gfx/tiles.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `caret_fresh` | window | seed D +-10% | `src/gfx/raster.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `tiles_blink_exact` | window | seed D +-10% | `src/gfx/tiles.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `alerts_none_for_hb` | unported | seed R +-10% | `src/core/json.bend` |  |
| `alerts_none_for_delta_end` | unported | seed R +-10% | `src/core/json.bend` |  |
| `hubs_redraw_delta_end` | unported | seed R +-10% | `src/core/json.bend` |  |
| `policy_phone_untouched_quiet` | client | seed C +-10% | `src/core/client.bend` |  |
| `policy_phone_touched_raises` | client | seed C +-10% | `src/core/client.bend` |  |
| `hubs_lean_no_live` | client | seed C +-10% | `src/core/client.bend` |  |
| `hubs_lean_keeps_state` | client | seed C +-10% | `src/core/client.bend` |  |
| `subs_finished_hidden` | client | seed C +-10% | `src/core/subs.bend` |  |
| `subs_ageout_hidden` | client | seed C +-10% | `src/core/subs.bend` |  |
| `subs_old_row_not_listed` | client | seed C +-10% | `src/core/subs.bend` |  |
| `subs_panel_new_reopens` | client | seed C +-10% | `src/core/subs.bend` |  |
| `subs_panel_shrinks_shut` | client | seed C +-10% | `src/core/subs.bend` |  |
| `subs_panel_attention_reopens` | client | seed C +-10% | `src/core/subs.bend` |  |
| `subs_model_alias` | client | seed C +-10% | `src/core/subs.bend` |  |
| `subs_panel_resolved_stays_shut` | client | seed C +-10% | `src/core/subs.bend` |  |
| `paste_name_png` | bend | seed K +-10% | `src/core/attach.bend` |  |
| `paste_name_jpeg` | bend | seed K +-10% | `src/core/attach.bend` |  |
| `paste_name_webp` | bend | seed K +-10% | `src/core/attach.bend` |  |
| `paste_name_gif` | bend | seed K +-10% | `src/core/attach.bend` |  |
| `paste_name_bmp` | bend | seed K +-10% | `src/core/attach.bend` |  |
| `paste_name_nonempty` | bend | seed K +-10% | `src/core/attach.bend` |  |
| `paste_split_rejoins` | unported | none | `-` |  |
| `paste_parts_rejoin` | bend | seed K +-10% | `src/core/attach.bend` |  |
| `paste_full_piece` | unported | none | `-` |  |
| `paste_chunk_quartets` | bend | seed K +-10% | `src/core/attach.bend` |  |
| `paste_chunk_triples` | bend | seed K +-10% | `src/core/attach.bend` |  |
| `paste_tour_ignored` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `paste_parts_k_rejoin` | bend | seed K +-10% | `src/core/attach.bend` |  |
| `paste_upload_rejoins` | bend | seed K +-10% | `src/core/attach.bend` |  |
| `paste_full_piece_size` | unported | none | `-` |  |
| `paste_key_length` | bend | seed K +-10% | `src/core/attach.bend` |  |
| `adopt_demo_kept` | bend | seed K +-10% | `src/core/adopt.bend` |  |
| `adopt_demo_stopped` | bend | seed K +-10% | `src/core/adopt.bend` |  |
| `adopt_off_stops_all_live` | bend | seed K +-10% | `src/core/adopt.bend` |  |
| `adopt_one_per_thread` | bend | seed K +-10% | `src/core/adopt.bend` |  |
| `adopt_mismatch_left` | bend | seed K +-10% | `src/core/adopt.bend` |  |
| `adopt_low_pid_left` | bend | seed K +-10% | `src/core/adopt.bend` |  |
| `cmds_json_round` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `adopt_kept_turn_runs` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `adopt_kept_never_carried` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `adopt_not_kept_carried` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `stuck_ends_ok` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `stuck_ends_failed_or_stopped` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `stuck_waits` | bend | seed K +-10% | `src/core/cmds.bend` |  |
| `stuck_step_ends_turn` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `keep_cat` | window | seed D +-10% | `src/gfx/raster.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `ops_after_cat` | window | seed D +-10% | `src/gfx/raster.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `tiles_recat_exact` | window | seed D +-10% | `src/gfx/tiles.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `keep_eq` | window | seed D +-10% | `src/gfx/raster.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `plan_cut_same` | window | seed D +-10% | `src/gfx/tiles.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `draft_clear_tells_host` | client | seed C +-10% | `src/core/client.bend` |  |
| `draft_clear_empties` | client | seed C +-10% | `src/core/client.bend` |  |
| `draft_send_forgets_first` | client | seed C +-10% | `src/core/client.bend` |  |
| `draft_bot_say_forgets_first` | client | seed C +-10% | `src/core/client.bend` |  |
| `draft_btw_forgets_first` | client | seed C +-10% | `src/core/client.bend` |  |
| `draft_give_forgets_first` | client | seed C +-10% | `src/core/client.bend` |  |
| `draft_stale_same` | client | seed C +-10% | `src/core/client.bend` |  |
| `draft_stale_with_attachments` | client | seed C +-10% | `src/core/client.bend` |  |
| `draft_stale_longer_is_not` | client | seed C +-10% | `src/core/client.bend` |  |
| `draft_stale_empty_never` | client | seed C +-10% | `src/core/client.bend` |  |
| `draft_stale_last` | unported | seed R +-10% | `src/core/model.bend` |  |
| `mail_route_person_empty` | unported | none | `-` |  |
| `mail_route_thread_pair` | unported | none | `-` |  |
| `mail_route_launch` | unported | none | `-` |  |
| `mail_route_room` | unported | none | `-` |  |
| `mail_route_system` | unported | none | `-` |  |
| `mail_want_completed_fyi` | unported | none | `-` |  |
| `mail_want_failed_act` | unported | none | `-` |  |
| `mail_want_given_wins` | unported | none | `-` |  |
| `mail_want_given_act` | unported | none | `-` |  |
| `mail_want_room_fyi` | unported | none | `-` |  |
| `mail_want_mention_act` | unported | none | `-` |  |
| `mail_want_own_request` | unported | none | `-` |  |
| `mail_want_wake_act` | unported | none | `-` |  |
| `mail_want_lost_act` | unported | none | `-` |  |
| `mail_want_deploy_own` | unported | none | `-` |  |
| `mail_want_lock_own` | unported | none | `-` |  |
| `mail_want_status_fyi` | unported | none | `-` |  |
| `mail_want_routine_act` | unported | none | `-` |  |
| `mail_want_hook_act` | unported | none | `-` |  |
| `mail_want_direct_act` | unported | none | `-` |  |
| `mail_want_person_act` | unported | none | `-` |  |
| `mail_post_idempotent` | unported | none | `-` |  |
| `mail_unread_after_post` | unported | none | `-` |  |
| `mail_origin_keeps_outputs` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `ref_uri_parse_thread` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_uri_parse_bot` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_uri_parse_room` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_uri_parse_person` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_uri_parse_project` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_uri_parse_routine` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_uri_parse_task` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_uri_parse_pr` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_uri_parse_work` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_uri_parse_file` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_uri_parse_machine` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_uri_parse_conv` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_uri_parse_mail` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_uri_parse_mailroom` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_uri_parse_decision` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_uri_parse_charter` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_web_parse` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_web_parse_slash` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_addr_roundtrip_thread` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_addr_roundtrip_bot` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_addr_roundtrip_room` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_addr_roundtrip_person` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_addr_roundtrip_mail` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_addr_roundtrip_conv` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_thread_addr` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_pct_safe` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_pct_bytes` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_pct_three` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_pct_four` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_pct_encoding` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_pct_malformed` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_unknown_kind` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_no_script` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_legacy_open` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_bare_thread` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_bare_bot` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_bare_task` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_bare_project` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_bare_room` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_bare_routine` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_bare_remote` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_bare_not_short` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_bare_not_word` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_bare_not_incomplete` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `ref_bare_not_unknown` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `link_ref_uri` | bend | seed K +-10% | `src/core/links.bend` |  |
| `link_ref_bare` | bend | seed K +-10% | `src/core/links.bend` |  |
| `link_ref_md` | bend | seed K +-10% | `src/core/links.bend` |  |
| `link_ref_plain_unchanged` | bend | seed K +-10% | `src/core/links.bend` |  |
| `i18n_plural_ja_other` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `i18n_plural_en_one` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `i18n_plural_en_zero` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `i18n_plural_en_many` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `i18n_resolve_override_en` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `i18n_resolve_override_ja` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `i18n_resolve_auto` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `i18n_of_tag_posix` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `i18n_of_tag_c` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `i18n_of_tag_empty` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `i18n_of_tag_bcp` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `i18n_ago_en_same` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `i18n_took_en_same` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `i18n_fallback_fixture_save` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `i18n_fallback_fixture_plural` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `usage_tail_na` | bend | seed K +-10% | `src/core/usage.bend` |  |
| `usage_tail_claude_waits` | bend | seed K +-10% | `src/core/usage.bend` |  |
| `usage_tail_reading` | bend | seed K +-10% | `src/core/usage.bend` |  |
| `usage_note_held` | bend | seed K +-10% | `src/core/usage.bend` |  |
| `usage_note_no_window` | bend | seed K +-10% | `src/core/usage.bend` |  |
| `usage_pct_no_window` | bend | seed K +-10% | `src/core/usage.bend` |  |
| `usage_boat_due_never_read` | bend | seed K +-10% | `src/core/usage.bend` |  |
| `usage_boat_usd_exact` | bend | seed K +-10% | `src/core/usage.bend` |  |
| `cjk_segs_lossless` | bend | seed K +-10% | `src/core/cjk.bend` |  |
| `cjk_segs_lossless_mixed` | bend | seed K +-10% | `src/core/cjk.bend` |  |
| `cjk_segs_lossless_spaces` | bend | seed K +-10% | `src/core/cjk.bend` |  |
| `cjk_segs_lossless_empty` | bend | seed K +-10% | `src/core/cjk.bend` |  |
| `cjk_no_start_stop` | bend | seed K +-10% | `src/core/cjk.bend` |  |
| `cjk_no_end_quote` | bend | seed K +-10% | `src/core/cjk.bend` |  |
| `cjk_kana_break` | bend | seed K +-10% | `src/core/cjk.bend` |  |
| `cjk_ascii_no_break` | bend | seed K +-10% | `src/core/cjk.bend` |  |
| `cjk_terminal_cells` | window | seed D +-10% | `src/app/layout.bend` | GPUI layout, native/gpui/src/main.rs (equivalence unchecked) |
| `cjk_terminal_combining_cells` | window | seed D +-10% | `src/app/layout.bend` | GPUI layout, native/gpui/src/main.rs (equivalence unchecked) |
| `tok_claude_seen_nothing` | unported | seed R +-10% | `src/core/json.bend` |  |
| `tok_rise_rose` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `tok_rise_restarted` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `tok_codex_restart_whole` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `tok_add_one_turn` | unported | seed R +-10% | `src/core/model.bend` |  |
| `tok_add_no_cost` | unported | seed R +-10% | `src/core/model.bend` |  |
| `tokstats_on_by_default` | unported | seed R +-10% | `src/core/model.bend` |  |
| `speed_unseen_untold` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `think_off_nothing` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `speed_told_rise` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `usage_boat_no_cap_nothing` | unported | seed R +-10% | `src/core/json.bend` |  |
| `errtail_scrub_token` | bend | seed K +-10% | `src/core/errtail.bend` |  |
| `errtail_scrub_words` | bend | seed K +-10% | `src/core/errtail.bend` |  |
| `errtail_say_exit` | bend | seed K +-10% | `src/core/errtail.bend` |  |
| `errtail_say_signal` | bend | seed K +-10% | `src/core/errtail.bend` |  |
| `keyring_locked_never_starts` | bend | seed K +-10% | `src/core/keyring.bend` |  |
| `keyring_unknown_or_open_starts` | bend | seed K +-10% | `src/core/keyring.bend` |  |
| `keyring_answers` | bend | seed K +-10% | `src/core/keyring.bend` |  |
| `keyring_hint_only_locked` | bend | seed K +-10% | `src/core/keyring.bend` |  |
| `ride_running_no_restart` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `ride_posted_at_once` | unported | seed R +-10% | `src/core/model.bend` |  |
| `ride_kept` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `ride_carries_block` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `ride_taken_empties` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `ride_nothing_waiting_same` | bend | seed K +-10% | `src/core/mcp.bend` |  |
| `ride_turn_end_starts` | unported | seed R +-10% | `src/core/model.bend` |  |
| `ride_interrupted_starts` | unported | seed R +-10% | `src/core/model.bend` |  |
| `ride_turn_end_empties` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `ride_after_running_same` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `deliver_held_restart` | unported | seed R +-10% | `src/core/model.bend` |  |
| `resume_turn_process_stopped` | bend | seed K +-10% | `src/core/adopt.bend` |  |
| `resume_turn_process_not_kept` | bend | seed K +-10% | `src/core/adopt.bend` |  |
| `resume_carry_tells_cut` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `deploy_mcp_call_waits` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `deploy_mcp_answered_goes` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `deploy_mcp_own_call_not_counted` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `deploy_mcp_outlives_turn_end` | bend | seed K +-10% | `src/core/deploy.bend` |  |
| `ime_preedit_not_in_draft` | bend | seed K +-10% | `src/core/ime.bend` |  |
| `ime_shown_at_caret` | bend | seed K +-10% | `src/core/ime.bend` |  |
| `ime_range_replace_chars` | bend | seed K +-10% | `src/core/ime.bend` |  |
| `ime_delete_range` | bend | seed K +-10% | `src/core/ime.bend` |  |
| `ime_wide_counts_once` | bend | seed K +-10% | `src/core/ime.bend` |  |
| `ime_clear_is_off` | bend | seed K +-10% | `src/core/ime.bend` |  |
| `ime_focus_change_drops` | bend | seed K +-10% | `src/core/ime.bend` |  |
| `ime_owner_keeps` | bend | seed K +-10% | `src/core/ime.bend` |  |
| `ime_other_owner_never_merges` | bend | seed K +-10% | `src/core/ime.bend` |  |
| `ime_unknown_event_not_quit` | window | seed D +-10% | `src/app/win.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `ime_unknown_event_is_junk` | window | seed D +-10% | `src/app/win.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `ime_quit_is_quit` | window | seed D +-10% | `src/app/win.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `ime_commit_event_id` | window | seed D +-10% | `src/app/win.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `body_line_terminated` | unported | seed R +-10% | `src/core/json.bend` |  |
| `body_header_omits_text` | unported | seed R +-10% | `src/core/model.bend` |  |
| `recover_seen_empty` | unported | seed R +-10% | `src/core/model.bend` |  |
| `recover_missing_header` | unported | seed R +-10% | `src/core/model.bend` |  |
| `body_path_encoded_segment` | unported | seed R +-10% | `src/core/model.bend` |  |
| `cards_get_fresh` | unported | seed R +-10% | `src/core/model.bend` |  |
| `cards_get_fresh_settle` | unported | seed R +-10% | `src/core/model.bend` |  |
| `cards_get_fresh_start` | unported | seed R +-10% | `src/core/model.bend` |  |
| `cards_get_fresh_end` | unported | seed R +-10% | `src/core/model.bend` |  |
| `cards_get_fresh_delete` | unported | seed R +-10% | `src/core/model.bend` |  |
| `cards_get_fresh_project` | unported | seed R +-10% | `src/core/model.bend` |  |
| `cards_peek_after_get` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `cards_drop_other_kept` | unported | none | `-` |  |
| `trail_go_back` | unported | none | `-` |  |
| `trail_back_fwd` | unported | none | `-` |  |
| `trail_go_same` | unported | none | `-` |  |
| `trail_go_clears_fwd` | unported | none | `-` |  |
| `trail_cap` | unported | none | `-` |  |
| `card_file_kind_shared` | unported | seed R +-10% | `src/core/model.bend` |  |
| `card_thread_no_timeline` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `card_github_pr_no_state` | bend | seed K +-10% | `src/core/ref.bend` |  |
| `card_person_no_state` | unported | seed R +-10% | `src/core/model.bend` |  |
| `card_task_status_agrees` | bend | seed K +-10% | `src/core/tboard.bend` |  |
| `order_buy_needs_parts` | unported | none | `-` |  |
| `order_buy_priced` | unported | none | `-` |  |
| `order_buy_unreachable_hidden` | unported | none | `-` |  |
| `order_buy_estimates_off` | unported | none | `-` |  |
| `order_buy_board_only` | unported | none | `-` |  |
| `order_buy_board_unreachable` | unported | none | `-` |  |
| `intent_edit_round_trip` | unported | none | `-` |  |
| `intent_edit_is_intent` | unported | none | `-` |  |
| `order_tools_buy_only` | unported | seed R +-10% | `src/core/bothub.bend` |  |
| `order_ready_names` | unported | none | `-` |  |
| `order_ready_board` | unported | none | `-` |  |
| `order_ready_nothing` | unported | none | `-` |  |
| `fab_ready_running_hidden` | client | seed C +-10% | `src/core/client.bend` |  |
| `fab_ready_needs_buy` | client | seed C +-10% | `src/core/client.bend` |  |
| `fab_ready_fresh_hidden` | client | seed C +-10% | `src/core/client.bend` |  |
| `fab_ready_fresh_note_hidden` | client | seed C +-10% | `src/core/client.bend` |  |
| `fab_ready_after_first_turn` | unported | seed R +-10% | `src/core/model.bend` |  |
| `order_review_none` | unported | seed R +-10% | `src/core/json.bend` |  |
| `order_review_current` | unported | none | `-` |  |
| `order_review_stale` | unported | none | `-` |  |
| `order_review_verdict_closed` | unported | none | `-` |  |
| `bd_name_folder` | client | seed C +-10% | `src/core/client.bend` |  |
| `bd_name_root` | client | seed C +-10% | `src/core/client.bend` |  |
| `bd_schematic_beside` | client | seed C +-10% | `src/core/client.bend` |  |
| `bd_board_itself` | client | seed C +-10% | `src/core/client.bend` |  |
| `bd_on_sheet` | client | seed C +-10% | `src/core/client.bend` |  |
| `bd_off_other` | client | seed C +-10% | `src/core/client.bend` |  |
| `bd_cur_exact_board` | client | seed C +-10% | `src/core/client.bend` |  |
| `bd_cur_exact_sheet` | client | seed C +-10% | `src/core/client.bend` |  |
| `bd_cur_sheet_anchor` | client | seed C +-10% | `src/core/client.bend` |  |
| `bd_keep_schematic` | client | seed C +-10% | `src/core/client.bend` |  |
| `bd_keep_none` | client | seed C +-10% | `src/core/client.bend` |  |
| `bd_shown_default` | client | seed C +-10% | `src/core/client.bend` |  |
| `bd_shown_picked` | client | seed C +-10% | `src/core/client.bend` |  |
| `bd_look_here` | client | seed C +-10% | `src/core/client.bend` |  |
| `bd_look_far` | client | seed C +-10% | `src/core/client.bend` |  |
| `bd_look_none` | client | seed C +-10% | `src/core/client.bend` |  |
| `boards_list_offered` | unported | none | `-` |  |
| `buy_title_first` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `buy_title_second` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `order_ordered_paid` | unported | none | `-` |  |
| `order_ordered_quote_only` | unported | none | `-` |  |
| `fab_buy_again` | client | seed C +-10% | `src/core/client.bend` |  |
| `fab_buy_again_off` | client | seed C +-10% | `src/core/client.bend` |  |
| `fab_min_key` | client | seed C +-10% | `src/core/client.bend` |  |
| `fab_min_hides_bar` | client | seed C +-10% | `src/core/client.bend` |  |
| `fab_min_keeps_bar` | client | seed C +-10% | `src/core/client.bend` |  |
| `fab_min_header` | client | seed C +-10% | `src/core/client.bend` |  |
| `fab_min_header_shown_bar` | client | seed C +-10% | `src/core/client.bend` |  |
| `fab_min_header_nothing` | client | seed C +-10% | `src/core/client.bend` |  |
| `fab_min_header_ordered` | client | seed C +-10% | `src/core/client.bend` |  |
| `buy_server_default` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `buy_server_set` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `order_note_shipped` | unported | none | `-` |  |
| `order_note_expired` | unported | none | `-` |  |
| `order_state_cancelled` | unported | none | `-` |  |
| `order_state_paid` | unported | none | `-` |  |
| `fab_buy_off_hidden` | client | seed C +-10% | `src/core/client.bend` |  |
| `order_off_no_tools` | unported | seed R +-10% | `src/core/bothub.bend` |  |
| `order_on_by_default` | unported | seed R +-10% | `src/core/model.bend` |  |
| `order_settings_on_by_default` | client | seed C +-10% | `src/core/client.bend` |  |
| `order_off_stays_off` | unported | seed R +-10% | `src/core/model.bend` |  |
| `order_on_when_on` | unported | seed R +-10% | `src/core/model.bend` |  |
| `order_review_issues_kept` | unported | seed R +-10% | `src/core/json.bend` |  |
| `order_review_issues_at_most` | unported | seed R +-10% | `src/core/json.bend` |  |
| `order_ask_survives_retire` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `order_approval_retired` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `order_gate_passed` | unported | none | `-` |  |
| `order_gate_stale` | unported | none | `-` |  |
| `order_gate_none` | unported | none | `-` |  |
| `order_gate_issues` | unported | none | `-` |  |
| `order_pilot_paid` | unported | none | `-` |  |
| `order_pilot_off` | unported | none | `-` |  |
| `order_pilot_done` | unported | none | `-` |  |
| `order_pilot_said` | unported | none | `-` |  |
| `order_pilot_quiet` | unported | none | `-` |  |
| `order_gate_lists_issues` | unported | seed R +-10% | `src/core/json.bend` |  |
| `order_gate_cuts_summary` | unported | none | `-` |  |
| `order_ask_alert_plain` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `order_ask_typed_alert_plain` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `order_ask_typed_survives_retire` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `order_ask_last_value` | unported | none | `-` |  |
| `order_card_files_first` | unported | none | `-` |  |
| `order_card_waits` | unported | none | `-` |  |
| `order_card_waiting_says` | unported | none | `-` |  |
| `order_card_question_first` | unported | none | `-` |  |
| `order_card_no_buy_paid` | unported | none | `-` |  |
| `order_card_buys_quoted` | unported | none | `-` |  |
| `order_wake_hidden` | unported | seed R +-10% | `src/core/model.bend` |  |
| `order_wake_failed_hidden` | unported | seed R +-10% | `src/core/model.bend` |  |
| `order_wake_untaken_hidden` | unported | seed R +-10% | `src/core/model.bend` |  |
| `order_wake_declined_hidden` | unported | seed R +-10% | `src/core/model.bend` |  |
| `order_wake_person_shown` | unported | seed R +-10% | `src/core/model.bend` |  |
| `order_customs_paid` | unported | none | `-` |  |
| `order_customs_courier` | unported | none | `-` |  |
| `order_status_sent_says` | unported | none | `-` |  |
| `order_status_unsent_says` | unported | none | `-` |  |
| `tool_summary_question` | unported | seed R +-10% | `src/core/json.bend` |  |
| `order_off_says_where` | unported | none | `-` |  |
| `order_on_says_buy_it` | unported | none | `-` |  |
| `order_fab_board_kinds` | unported | none | `-` |  |
| `order_buy_assembles_rigid_only` | unported | none | `-` |  |
| `order_on_when_asked` | unported | none | `-` |  |
| `lang_default_auto` | bend | seed K +-10% | `src/core/client-language.bend` |  |
| `lang_default_unknown` | bend | seed K +-10% | `src/core/client-language.bend` |  |
| `lang_gate` | bend | seed K +-10% | `src/core/client-language.bend` |  |
| `lang_gate_preview` | bend | seed K +-10% | `src/core/client-language.bend` |  |
| `lang_auto_preview` | bend | seed K +-10% | `src/core/client-language.bend` |  |
| `lang_override_en` | bend | seed K +-10% | `src/core/client-language.bend` |  |
| `prompt_lang_off` | bend | seed K +-10% | `src/core/prompt.bend` |  |
| `prompt_lang_unknown` | bend | seed K +-10% | `src/core/prompt.bend` |  |
| `mv_visible_bounds` | unported | none | `-` |  |
| `mv_merge_no_dup` | unported | none | `-` |  |
| `mv_rows_stable` | unported | none | `-` |  |
| `mv_filter_key_separates` | unported | none | `-` |  |
| `tok_cost_unknown_without_start` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `tok_cost_known_at_start` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `tok_codex_start_zero` | unported | seed R +-10% | `src/core/model.bend` |  |
| `tok_epoch_once` | unported | seed R +-10% | `src/core/model.bend` |  |
| `tok_pre_epoch` | unported | seed R +-10% | `src/core/model.bend` |  |
| `mail_index_wrong_position` | unported | none | `-` |  |
| `mail_index_forward_chain_rejected` | unported | none | `-` |  |
| `mail_index_overflow_rejected` | unported | none | `-` |  |
| `mail_index_hash_empty` | unported | none | `-` |  |
| `hold_at_end_zero` | client | seed C +-10% | `src/core/client.bend` |  |
| `hold_label_nothing` | client | seed C +-10% | `src/core/client.bend` |  |
| `hold_shown_stopped` | client | seed C +-10% | `src/core/client.bend` |  |
| `hold_shown_receipt_hidden` | client | seed C +-10% | `src/core/client.bend` |  |
| `hold_shown_empty_hidden` | unported | seed R +-10% | `src/core/model.bend` |  |
| `hold_fresh_seen` | unported | seed R +-10% | `src/core/model.bend` |  |
| `hold_off_still` | client | seed C +-10% | `src/core/client.bend` |  |
| `hold_end_follows` | window | seed D +-10% | `src/app/layout.bend` | GPUI layout, native/gpui/src/main.rs (equivalence unchecked) |
| `hold_boundary_zero` | window | seed D +-10% | `src/app/layout.bend` | GPUI layout, native/gpui/src/main.rs (equivalence unchecked) |
| `hold_entry_moves` | window | seed D +-10% | `src/app/layout.bend` | GPUI layout, native/gpui/src/main.rs (equivalence unchecked) |
| `hold_page_free` | client | seed C +-10% | `src/core/client.bend` |  |
| `tok_claude_start_zero` | unported | seed R +-10% | `src/core/model.bend` |  |
| `tok_claude_same_session_keeps` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `tok_recount_once` | unported | seed R +-10% | `src/core/model.bend` |  |
| `chip_none` | bend | seed K +-10% | `src/core/usage.bend` |  |
| `chip_running` | bend | seed K +-10% | `src/core/usage.bend` |  |
| `chip_warm_gone` | bend | seed K +-10% | `src/core/usage.bend` |  |
| `chip_warm_not_stale` | bend | seed K +-10% | `src/core/usage.bend` |  |
| `ship_covered_never_builds` | unported | none | `-` |  |
| `ship_busy_never_builds` | unported | none | `-` |  |
| `ship_building_is_busy` | unported | none | `-` |  |
| `ship_coalesce_at_most_one` | unported | none | `-` |  |
| `ship_coalesce_three` | unported | none | `-` |  |
| `ship_coalesce_askers` | unported | none | `-` |  |
| `ship_told_each_once` | unported | none | `-` |  |
| `ship_told_cleared` | unported | none | `-` |  |
| `ship_told_next_cleared` | unported | none | `-` |  |
| `ship_covered_joins_build` | unported | none | `-` |  |
| `ship_follow_every_minute` | unported | none | `-` |  |
| `ship_follow_off_default` | unported | none | `-` |  |
| `ship_gone_not_ok` | unported | none | `-` |  |
| `kicad_status_missing` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `kicad_status_broken` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `kicad_status_stock` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `kicad_notice_ok_none` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `kicad_notice_checking_none` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `kicad_notice_has_retry_missing` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `kicad_notice_install_iff_missing` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `kicad_notice_has_retry_stock` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `kicad_notice_install_iff_stock` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `kicad_notice_has_retry_old` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `kicad_notice_install_iff_old` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `kicad_notice_has_retry_broken` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `kicad_notice_install_iff_broken` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `prompt_kicad_ipc_only_ok_missing` | bend | seed K +-10% | `src/core/prompt.bend` |  |
| `prompt_kicad_ipc_only_ok_stock` | bend | seed K +-10% | `src/core/prompt.bend` |  |
| `prompt_kicad_ipc_only_ok_old` | bend | seed K +-10% | `src/core/prompt.bend` |  |
| `prompt_kicad_ipc_only_ok_broken` | bend | seed K +-10% | `src/core/prompt.bend` |  |
| `prompt_kicad_ipc_only_ok_checking` | bend | seed K +-10% | `src/core/prompt.bend` |  |
| `mm_late_misses` | unported | none | `-` |  |
| `mail_off_unchanged` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `mail_file_never_waits` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `mail_tee_keeps_delivery` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `mail_file_one_conv` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `mail_room_once` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `title_shown_ja_new` | client | seed C +-10% | `src/core/client.bend` |  |
| `title_shown_en_new` | client | seed C +-10% | `src/core/client.bend` |  |
| `title_shown_other` | client | seed C +-10% | `src/core/client.bend` |  |
| `tl_entries_locale` | window | seed D +-10% | `src/app/layout.bend` | GPUI layout, native/gpui/src/main.rs (equivalence unchecked) |
| `kserve_argv_parent_pid_with_idle` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_argv_parent_pid_without_idle` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_argv_socket_in_dir` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_argv_idle_with_cap` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_argv_no_idle_without_cap` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_ready_line` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_ready_other_line` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_version_ok_exact` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_version_ok_describe` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_version_other_build` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_version_longer_build` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_version_not_fork` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_retry_first` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_retry_second` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_retry_third` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_retry_gives_up` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_retry_old_failures_forgotten` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_clean_exit_not_a_failure` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_crash_is_a_failure` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_clean_exit_stops` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_three_crashes_restart` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_fourth_crash_fails` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_evict_least_recent` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_no_evict_with_room` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_third_project_evicts` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_third_project_busy_without_shutdown` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_running_server_is_used` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_refused_selection` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_refused_view` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_refused_run_action` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_get_items_allowed` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_eighteen_refused` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_gate_off` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_gate_ok` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_unavailable_names_fallback` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kicad_min_version` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `kicad_old_runtime_is_old` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `kicad_new_runtime_is_ok` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `prompt_kicad_ok_names_kicad_ipc` | bend | seed K +-10% | `src/core/prompt.bend` |  |
| `prompt_kicad_ok_no_pcbnew_import` | bend | seed K +-10% | `src/core/prompt.bend` |  |
| `prompt_kicad_ok_starts_no_server` | bend | seed K +-10% | `src/core/prompt.bend` |  |
| `prompt_kicad_off_names_fallback` | bend | seed K +-10% | `src/core/prompt.bend` |  |
| `kserve_desc_needs_cap` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_desc_with_cap` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve_idle_needs_api2` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kexport_no_unmarked_clear` | bend | seed K +-10% | `src/core/kexport.bend` |  |
| `kexport_no_unconsented_force` | bend | seed K +-10% | `src/core/kexport.bend` |  |
| `kexport_block_force` | bend | seed K +-10% | `src/core/kexport.bend` |  |
| `kexport_lossy_force` | bend | seed K +-10% | `src/core/kexport.bend` |  |
| `kexport_no_outside_clear` | bend | seed K +-10% | `src/core/kexport.bend` |  |
| `kexport_no_foreign_base` | bend | seed K +-10% | `src/core/kexport.bend` |  |
| `foh_clean` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `fyi_never_spawns` | unported | none | `-` |  |
| `act_one_run` | unported | none | `-` |  |
| `run_fresh` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `run_one_live_per_conv` | unported | none | `-` |  |
| `escalate_only_path` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `charter_gate_denies` | unported | seed R +-10% | `src/core/json.bend` |  |
| `charter_gate_allows_listed` | unported | seed R +-10% | `src/core/json.bend` |  |
| `charter_risky_fixed` | unported | seed R +-10% | `src/core/json.bend` |  |
| `window_notice_english` | window | seed D +-10% | `src/app/layout.bend` | GPUI layout, native/gpui/src/main.rs (equivalence unchecked) |
| `window_notice_words_kept` | window | seed D +-10% | `src/app/layout.bend` | GPUI layout, native/gpui/src/main.rs (equivalence unchecked) |
| `window_tools_english` | bend | seed K +-10% | `src/core/tools.bend` |  |
| `window_picker_default` | window | seed D +-10% | `src/app/layout.bend` | GPUI layout, native/gpui/src/main.rs (equivalence unchecked) |
| `window_provider_missing` | window | seed D +-10% | `src/app/layout.bend` | GPUI layout, native/gpui/src/main.rs (equivalence unchecked) |
| `cef_engine_needs_setting` | unported | none | `-` |  |
| `cef_engine_needs_embed` | unported | none | `-` |  |
| `cef_engine_needs_runtime` | unported | none | `-` |  |
| `cef_engine_on` | unported | none | `-` |  |
| `cef_embed_window_here` | unported | none | `-` |  |
| `cef_embed_loopback_link` | unported | none | `-` |  |
| `cef_embed_remote_refused` | unported | none | `-` |  |
| `cef_loopback_v4` | unported | none | `-` |  |
| `cef_loopback_v6` | unported | none | `-` |  |
| `cef_tailnet_not_loopback` | unported | none | `-` |  |
| `cef_leave_clears_embed` | unported | none | `-` |  |
| `cef_leave_other_keeps_embed` | unported | none | `-` |  |
| `cef_embed_bad_xid_unknown` | unported | none | `-` |  |
| `cef_embed_once_per_join` | unported | none | `-` |  |
| `cef_embed_new_join` | unported | none | `-` |  |
| `cef_embed_not_remote` | unported | none | `-` |  |
| `cef_embed_needs_xid` | unported | none | `-` |  |
| `cef_shield_agent_drives` | unported | none | `-` |  |
| `cef_shield_takeover_off` | unported | none | `-` |  |
| `cef_shield_teach_off` | unported | none | `-` |  |
| `cef_shield_focus_opposite` | unported | none | `-` |  |
| `cef_shield_take_from_agent` | unported | none | `-` |  |
| `cef_shield_take_not_from_person` | unported | none | `-` |  |
| `cef_shield_due_same` | unported | none | `-` |  |
| `cef_shield_due_first` | unported | none | `-` |  |
| `cef_shield_due_person` | unported | none | `-` |  |
| `cef_covered_none` | unported | none | `-` |  |
| `cef_covered_palette` | unported | none | `-` |  |
| `cef_covered_picker` | unported | none | `-` |  |
| `cef_covered_menu` | unported | none | `-` |  |
| `cef_covered_look` | unported | none | `-` |  |
| `cef_covered_models` | unported | none | `-` |  |
| `cef_covered_tour` | unported | none | `-` |  |
| `cef_covered_perf` | unported | none | `-` |  |
| `cef_covered_lightbox` | unported | none | `-` |  |
| `cef_shows_needs_live` | unported | none | `-` |  |
| `cef_shows_not_covered` | unported | none | `-` |  |
| `cef_place_same_nothing` | unported | none | `-` |  |
| `cef_place_hidden_nothing` | unported | none | `-` |  |
| `cef_place_new_shows` | unported | none | `-` |  |
| `cef_place_moved_shows` | unported | none | `-` |  |
| `cef_place_gone_hides` | unported | none | `-` |  |
| `cef_post_chrome_idle` | unported | none | `-` |  |
| `cef_post_chrome_forgets` | unported | none | `-` |  |
| `cef_post_native_places` | unported | none | `-` |  |
| `cef_pane_extent` | unported | none | `-` |  |
| `cef_extent_never_wraps` | unported | none | `-` |  |
| `cef_native_local_only` | unported | none | `-` |  |
| `cef_native_local_cef` | unported | none | `-` |  |
| `cef_native_chrome` | unported | none | `-` |  |
| `cef_native_skips_cast` | unported | none | `-` |  |
| `cef_web_keeps_cast` | unported | none | `-` |  |
| `cef_remote_keeps_cast` | unported | none | `-` |  |
| `cef_page_here_native` | unported | none | `-` |  |
| `cef_page_elsewhere_jpeg` | unported | none | `-` |  |
| `cef_fallback_keeps_handback` | unported | none | `-` |  |
| `cef_chrome_idle_strip_off` | unported | none | `-` |  |
| `cef_note_quiet` | unported | none | `-` |  |
| `cef_old_engine_reply_ignored` | unported | none | `-` |  |
| `cef_current_engine_reply_used` | unported | none | `-` |  |
| `cef_want_key_chrome` | unported | none | `-` |  |
| `cef_after_rpc` | unported | none | `-` |  |
| `cef_after_helper` | unported | none | `-` |  |
| `cef_not_after_agent_line` | unported | none | `-` |  |
| `cef_revive_first` | unported | none | `-` |  |
| `cef_revive_bounded` | unported | none | `-` |  |
| `cef_revive_not_chrome` | unported | none | `-` |  |
| `cef_unknown_refused` | unported | none | `-` |  |
| `cef_thread_owner` | unported | none | `-` |  |
| `cef_empty_owner_refused` | unported | none | `-` |  |
| `cef_handback_names_page` | client | seed C +-10% | `src/core/client.bend` |  |
| `cef_handback_default_bot` | client | seed C +-10% | `src/core/client.bend` |  |
| `cef_takeover_names_page` | client | seed C +-10% | `src/core/client.bend` |  |
| `cef_other_actions_keep_bot` | client | seed C +-10% | `src/core/client.bend` |  |
| `cef_web_origin_refused` | unported | none | `-` |  |
| `cef_native_link_allowed` | unported | none | `-` |  |
| `mail_credit_carried` | unported | seed R +-10% | `src/server/wire.bend` |  |
| `mail_credit_native_kept` | unported | seed R +-10% | `src/server/wire.bend` |  |
| `webphone_option_english` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `webphone_model_name_kept` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `webphone_count_ungrouped` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `webphone_count_japanese` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `webphone_idle_japanese` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `cli_unknown_never_runs` | unported | none | `-` |  |
| `cli_unknown_after_flags_never_runs` | unported | none | `-` |  |
| `cli_word_never_runs` | unported | none | `-` |  |
| `cli_version_never_runs` | unported | none | `-` |  |
| `cli_version_after_flags` | unported | none | `-` |  |
| `cli_help_never_runs` | unported | none | `-` |  |
| `cli_h_never_runs` | unported | none | `-` |  |
| `cli_missing_value` | unported | none | `-` |  |
| `cli_value_not_a_flag` | unported | none | `-` |  |
| `cli_test_hub_runs` | unported | none | `-` |  |
| `onehub_ours` | unported | none | `-` |  |
| `onehub_held` | unported | none | `-` |  |
| `onehub_says` | unported | none | `-` |  |
| `standin_path_first` | unported | none | `-` |  |
| `standin_merge_behind` | bend | seed K +-10% | `src/core/clipath.bend` |  |
| `kicad4_bp1_old` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `kicad4_bp2_old` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `kicad4_bp3_old` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `kicad4_bp4_ok` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `kicad4_bp10_ok` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `kicad4_describe_part_reads_bp` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `kicad4_no_suffix_is_bp0` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `kicad4_bp3_needs_api2` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `kicad4_bp2_before_bp3` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `kicad4_bp10_after_bp3` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `kicad4_min_api` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `kicad4_pinned_version_is_min` | bend | seed K +-10% | `src/core/kihealth.bend` |  |
| `kserve4_launcher_never_real` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve4_launcher_kept` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve4_argv_launcher` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve4_bare_footprint_refused` | unported | seed R +-10% | `src/core/json.bend` |  |
| `kserve4_refusal_names_full_instance` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve4_full_footprint_passes` | unported | seed R +-10% | `src/core/json.bend` |  |
| `kserve4_footprint_cap_lifts` | unported | seed R +-10% | `src/core/json.bend` |  |
| `kserve4_other_item_passes` | unported | seed R +-10% | `src/core/json.bend` |  |
| `kserve4_other_command_passes` | unported | seed R +-10% | `src/core/json.bend` |  |
| `kserve4_bare_symbol_refused` | unported | seed R +-10% | `src/core/json.bend` |  |
| `kserve4_symbol_advice` | bend | seed K +-10% | `src/core/kserve.bend` |  |
| `kserve4_symbol_with_definition_passes` | unported | seed R +-10% | `src/core/json.bend` |  |
| `kserve4_symbol_cap_lifts` | unported | seed R +-10% | `src/core/json.bend` |  |
| `kserve4_no_items_passes` | unported | seed R +-10% | `src/core/json.bend` |  |
| `prompt4_kicad_ok_names_the_rules` | bend | seed K +-10% | `src/core/prompt.bend` |  |
| `kserve4_nested_symbol_library_id_refused` | unported | seed R +-10% | `src/core/json.bend` |  |
| `kserve4_nested_symbol_cap_lifts` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fork_cache_legacy` | unported | none | `-` |  |
| `fork_cache_alias` | unported | none | `-` |  |
| `fork_footprint_legacy_at` | bend | seed K +-10% | `src/core/board.bend` |  |
| `mail_plain_count` | unported | none | `-` |  |
| `mail_global_primary` | unported | none | `-` |  |
| `mail_log_count_set` | unported | none | `-` |  |
| `mail_log_count_committed` | unported | none | `-` |  |
| `native_language_english` | bend | seed K +-10% | `src/core/client-language.bend` |  |
| `native_language_preview` | bend | seed K +-10% | `src/core/client-language.bend` |  |
| `native_language_system` | bend | seed K +-10% | `src/core/client-language.bend` |  |
| `native_language_fallback` | bend | seed K +-10% | `src/core/client-language.bend` |  |
| `exe_server_name` | bend | seed K +-10% | `src/core/exe.bend` |  |
| `exe_server_path` | bend | seed K +-10% | `src/core/exe.bend` |  |
| `exe_app_name` | bend | seed K +-10% | `src/core/exe.bend` |  |
| `exe_server_exact` | bend | seed K +-10% | `src/core/exe.bend` |  |
| `fallback_empty_exhausted` | bend | seed K +-10% | `src/core/fallback.bend` |  |
| `fallback_never_wraps` | bend | seed K +-10% | `src/core/fallback.bend` |  |
| `fallback_auth_is_not_quota` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fallback_unknown_reset_held` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fallback_reset_boundary` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fallback_no_recovery_while_locked` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `sidebar_row_key_sound` | window | seed D +-10% | `src/app/side_row_types.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `sidebar_row_paint_exact` | window | seed D +-10% | `src/app/side_rows.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `visible_cats_empty_preserves_cache` | window | seed D +-10% | `src/app/cats.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `visible_sleep_cat_posed_once` | window | seed D +-10% | `src/app/bots.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `mailroom_tab_after_chat` | client | seed C +-10% | `src/core/botui.bend` |  |
| `mailroom_uber_default` | client | seed C +-10% | `src/core/botui.bend` |  |
| `mailroom_run_collapsed_default` | client | seed C +-10% | `src/core/client.bend` |  |
| `mailroom_chip_filters` | unported | none | `-` |  |
| `mailroom_chip_keeps_selection` | client | seed C +-10% | `src/core/client.bend` |  |
| `mailroom_covers_plane` | bend | seed K +-10% | `src/core/plane.bend` |  |
| `mail_body_kept_output` | window | seed D +-10% | `src/app/layout.bend` | GPUI layout, native/gpui/src/main.rs (equivalence unchecked) |
| `mail_age_relative_deadline` | unported | none | `-` |  |
| `mail_plane_indexed_build` | client | seed C +-10% | `src/core/client.bend` |  |
| `mail_plane_indexed_hidden` | window | seed D +-10% | `src/app/bots.bend` | GPUI replaces this window code (native/gpui); equivalence unchecked, no law-level mapping yet |
| `auth_foreign_origin_denied` | bend | seed K +-10% | `src/core/http.bend` |  |
| `auth_empty_remote_token_denied` | bend | seed K +-10% | `src/core/auth.bend` |  |
| `auth_cookie_prefix_denied` | bend | seed K +-10% | `src/core/auth.bend` |  |
| `auth_loopback_host_needs_token` | bend | seed K +-10% | `src/core/auth.bend` |  |
| `auth_localhost_needs_token` | bend | seed K +-10% | `src/core/auth.bend` |  |
| `auth_no_host_needs_token` | bend | seed K +-10% | `src/core/auth.bend` |  |
| `auth_loopback_with_token` | bend | seed K +-10% | `src/core/auth.bend` |  |
| `auth_oauth_post_denied` | bend | seed K +-10% | `src/core/auth.bend` |  |
| `auth_oauth_other_route_denied` | bend | seed K +-10% | `src/core/auth.bend` |  |
| `auth_oauth_foreign_origin_denied` | bend | seed K +-10% | `src/core/http.bend` |  |
| `auth_oauth_local_document_reaches_state_check` | bend | seed K +-10% | `src/core/http.bend` |  |
| `auth_oauth_document_does_not_authorize_websocket` | bend | seed K +-10% | `src/core/http.bend` |  |
| `pcb2_hidden_text_draws_nothing` | bend | seed K +-10% | `src/core/board.bend` |  |
| `viewer_reopen_board_keeps_scene` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `viewer_reopen_schematic_keeps_scene` | window | seed D +-10% | `src/app/nui.bend` | GPUI input handling, native/gpui/src/input.rs (equivalence unchecked) |
| `cef_default_request` | unported | none | `-` |  |
| `cef_unset_request` | unported | none | `-` |  |
| `cef_explicit_chrome_request` | unported | none | `-` |  |
| `cef_explicit_cef_request` | unported | none | `-` |  |
| `cef_unavailable_default_is_explained` | unported | none | `-` |  |
| `cef_explicit_chrome_has_no_warning` | unported | none | `-` |  |
| `cef_fallback_keeps_error` | unported | none | `-` |  |
| `run_model_codex_unset_inherits` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `run_model_codex_stale_claude_uses_default` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `run_model_codex_compatible_override_kept` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `run_model_muse_keeps_exact_full_selector` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `run_model_claude_uber_default_kept` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `run_model_claude_worker_default_kept` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `run_model_codex_stale_parent_uses_cli_default` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `wt_seed_digit_step` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `wt_seed_skips_separator` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `wt_seed_legacy_small` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `wt_seed_large_id` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `wt_seed_same_second_next` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `wt_seed_wrap` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `tm_optout_no_id` | unported | seed ? +-10% | `test/telemetry_cases.bend` |  |
| `tm_unasked_no_id` | bend | seed K +-10% | `src/core/telemetry.bend` |  |
| `tm_sandbox_never` | bend | seed K +-10% | `src/core/telemetry.bend` |  |
| `tm_gate_no_key` | bend | seed K +-10% | `src/core/telemetry.bend` |  |
| `tm_clean_drops_path` | unported | seed ? +-10% | `test/telemetry_cases.bend` |  |
| `tm_clean_drops_email` | unported | seed ? +-10% | `test/telemetry_cases.bend` |  |
| `tm_clean_drops_name` | unported | seed ? +-10% | `test/telemetry_cases.bend` |  |
| `tm_active_once_a_day` | bend | seed K +-10% | `src/core/telemetry.bend` |  |
| `tm_queue_cap` | bend | seed K +-10% | `src/core/telemetry.bend` |  |
| `tm_sent_202_drops` | bend | seed K +-10% | `src/core/telemetry.bend` |  |
| `tm_sent_500_keeps` | unported | seed ? +-10% | `test/telemetry_cases.bend` |  |
| `tm_state_roundtrip` | unported | seed ? +-10% | `test/telemetry_cases.bend` |  |
| `share_on_sets_asked` | client | seed C +-10% | `src/core/client.bend` |  |
| `share_step_before_uber` | client | seed C +-10% | `src/core/onboard.bend` |  |
| `bug_url_only_version` | bend | seed K +-10% | `src/core/telemetry.bend` |  |
| `tm_reset_invalidates_worker` | unported | seed R +-10% | `src/core/model.bend` |  |
| `tm_optout_invalidates_worker` | unported | seed R +-10% | `src/core/model.bend` |  |
| `tm_loopback_rejects_userinfo` | bend | seed K +-10% | `src/core/telemetry.bend` |  |
| `tm_package_nightly_channel` | bend | seed K +-10% | `src/core/telemetry.bend` |  |
| `tm_new_usage_marker` | client | seed C +-10% | `src/core/client.bend` |  |
| `tm_cached_uber_step_kept` | client | seed C +-10% | `src/core/onboard.bend` |  |
| `tm_marked_usage_step_kept` | client | seed C +-10% | `src/core/onboard.bend` |  |
| `browser_url_hostname` | unported | none | `-` |  |
| `browser_url_localhost` | unported | none | `-` |  |
| `browser_url_explicit` | unported | none | `-` |  |
| `browser_url_no_script` | unported | none | `-` |  |
| `browser_toolbar_no_eval` | unported | none | `-` |  |
| `browser_toolbar_tabs_allowed` | unported | none | `-` |  |
| `browser_new_tab_google` | unported | seed R +-10% | `src/core/json.bend` |  |
| `refresh_revision_changed` | bend | seed K +-10% | `src/core/refresh.bend` |  |
| `refresh_size_changed` | bend | seed K +-10% | `src/core/refresh.bend` |  |
| `refresh_unchanged_quiet` | bend | seed K +-10% | `src/core/refresh.bend` |  |
| `cef_release_uses_paired_archive` | unported | none | `-` |  |
| `cef_release_arm_archive_unchanged` | unported | none | `-` |  |
| `cef_release_mac_archive_unchanged` | unported | none | `-` |  |
| `cef_release_old_updater_cannot_match` | unported | none | `-` |  |
| `ix_rename_global_invalidation_retained` | unported | none | `-` |  |
| `ix_rename_relationship_invalidation_retained` | unported | none | `-` |  |
| `ix_old_touch_global_fallback` | unported | none | `-` |  |
| `subs_phase_index_supplied_exact` | client | seed C +-10% | `src/core/client.bend` |  |
| `subs_children_indexed_exact` | client | seed C +-10% | `src/core/subs.bend` |  |
| `subs_rows_indexed_exact` | client | seed C +-10% | `src/core/subs.bend` |  |
| `mem_score_w_strict_equivalence` | bend | seed K +-10% | `src/core/bot.bend` |  |
| `tl_live_single_exact` | window | seed D +-10% | `src/app/layout.bend` | GPUI layout, native/gpui/src/main.rs (equivalence unchecked) |
| `textscale_default` | bend | seed K +-10% | `src/core/textscale.bend` |  |
| `textscale_invalid` | bend | seed K +-10% | `src/core/textscale.bend` |  |
| `textscale_real_font_size` | bend | seed K +-10% | `src/core/textscale.bend` |  |
| `mail_continuity_read_open_unresolved` | bend | seed K +-10% | `src/core/mailcontinuity.bend` |  |
| `mail_continuity_unread_unresolved` | bend | seed K +-10% | `src/core/mailcontinuity.bend` |  |
| `mail_continuity_current_escalation_once` | bend | seed K +-10% | `src/core/mailcontinuity.bend` |  |
| `mail_continuity_committed_reply_resolves` | bend | seed K +-10% | `src/core/mailcontinuity.bend` |  |
| `mail_disposition_inactive_refused` | bend | seed K +-10% | `src/core/maildisposition.bend` |  |
| `mail_disposition_ordinary_codex_unchanged` | bend | seed K +-10% | `src/core/maildisposition.bend` |  |
| `mail_disposition_codex_single_tool` | bend | seed K +-10% | `src/core/maildisposition.bend` |  |
| `mail_disposition_done_notice_only` | bend | seed K +-10% | `src/core/maildisposition.bend` |  |
| `mail_disposition_defer_notice_only` | bend | seed K +-10% | `src/core/maildisposition.bend` |  |
| `mail_disposition_no_product_action` | bend | seed K +-10% | `src/core/maildisposition.bend` |  |
| `mail_disposition_bot_router_scoped_tools` | unported | seed R +-10% | `src/core/json.bend` |  |
| `mail_disposition_plain_router_scoped_tools` | unported | seed R +-10% | `src/core/bothub.bend` |  |
| `mail_ui_dispatch_available` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `mail_ui_body_read_available` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `mail_ui_invalid_target_no_effect` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `mail_ui_owned_target_uses_mailbox` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `mail_room_invalid_request_is_error` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `mail_room_invalid_room_is_error` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `mail_room_send_no_eager_events` | unported | seed R +-10% | `src/core/model.bend` |  |
| `mail_room_set_updates_client_index` | client | seed C +-10% | `src/core/client.bend` |  |
| `mail_room_post_updates_client_index` | client | seed C +-10% | `src/core/client.bend` |  |
| `mail_room_post_affects_room_only` | client | seed C +-10% | `src/core/client.bend` |  |
| `mail_room_target_nonroom_unchanged` | client | seed C +-10% | `src/core/client.bend` |  |
| `mail_room_target_single_empty_option` | client | seed C +-10% | `src/core/client.bend` |  |
| `mail_room_target_duplicate_dropped` | client | seed C +-10% | `src/core/client.bend` |  |
| `desktop_public_row_no_document` | unported | seed R +-10% | `src/core/json.bend` |  |
| `desktop_public_row_no_lease` | unported | seed R +-10% | `src/core/json.bend` |  |
| `desktop_public_row_no_capture` | unported | seed R +-10% | `src/core/json.bend` |  |
| `desktop_lost_guard_no_cdp` | unported | seed R +-10% | `src/core/json.bend` |  |
| `desktop_lost_guard_no_reinitialize` | unported | seed R +-10% | `src/core/json.bend` |  |
| `desktop_no_generic_file_assignment` | bend | seed K +-10% | `src/core/desktop-browser.bend` |  |
| `desktop_no_public_interception_toggle` | bend | seed K +-10% | `src/core/desktop-browser.bend` |  |
| `desktop_unknown_owner_has_no_drive` | bend | seed K +-10% | `src/core/desktop-browser.bend` |  |
| `desktop_connection_frame_bytes_unchanged` | unported | seed R +-10% | `src/server/desktop-info.bend` |  |
| `desktop_connection_close_bytes_unchanged` | unported | seed R +-10% | `src/server/desktop-info.bend` |  |
| `presentation_state_kept` | client | seed C +-10% | `src/core/client.bend` |  |
| `presentation_seq_kept` | client | seed C +-10% | `src/core/client.bend` |  |
| `presentation_bodies_kept` | client | seed C +-10% | `src/core/client.bend` |  |
| `presentation_history_generation_kept` | unported | seed R +-10% | `src/core/outbox.bend` |  |
| `presentation_history_room_kept` | unported | seed R +-10% | `src/core/outbox.bend` |  |
| `presentation_history_over_kept` | unported | seed R +-10% | `src/core/outbox.bend` |  |
| `cache_identity_post_refused` | bend | seed K +-10% | `src/core/auth.bend` |  |
| `cache_identity_empty_cookie` | bend | seed K +-10% | `src/core/auth.bend` |  |
| `oprep_task_not_in_chat` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `oprep_task_delivered_with_filing` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `oprep_other_thread_unchanged` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `cbor_trees_exact` | unported | none | `-` |  |
| `thin_pace_own_urgent` | unported | none | `-` |  |
| `thin_pace_other_soft` | unported | none | `-` |  |
| `thin_pace_all_urgent` | unported | none | `-` |  |
| `thin_pace_none_quiet` | unported | none | `-` |  |
| `thin_pace_no_thread_soft` | unported | none | `-` |  |
| `thin_pace_own_sends` | unported | none | `-` |  |
| `thin_pace_soft_owed` | unported | none | `-` |  |
| `thin_pace_flush_sends` | unported | none | `-` |  |
| `thin_pace_owed_raised` | unported | none | `-` |  |
| `thin_pace_quiet` | unported | none | `-` |  |
| `store_size_go_succ` | bend | seed K +-10% | `src/core/store.bend` |  |
| `store_size_is_length` | bend | seed K +-10% | `src/core/store.bend` |  |
| `thin_words_lower_each` | unported | none | `-` |  |
| `sdb_replay_append` | unported | none | `-` |  |
| `sdb_eq_fold_small` | unported | seed R +-10% | `src/core/model.bend` |  |
| `sdb_entries_mine_small` | unported | seed R +-10% | `src/core/model.bend` |  |
| `thin_changes_append` | unported | seed R +-10% | `src/core/model.bend` |  |
| `thin_lazy_of` | unported | seed R +-10% | `src/core/model.bend` |  |
| `thin_lazy_changes` | unported | seed R +-10% | `src/core/model.bend` |  |
| `thin_lazy_warm` | unported | none | `-` |  |
| `thin_mail_replay_append` | unported | none | `-` |  |
| `thin_mail_lazy_of` | unported | seed R +-10% | `src/core/model.bend` |  |
| `thin_mail_lazy_committed` | unported | seed R +-10% | `src/core/model.bend` |  |
| `thin_mail_lazy_warm` | unported | seed R +-10% | `src/core/model.bend` |  |
| `http_count_go_succ` | bend | seed K +-10% | `src/core/http.bend` |  |
| `http_count_is_length` | bend | seed K +-10% | `src/core/http.bend` |  |
| `ws_unmask_go` | bend | seed K +-10% | `src/core/http.bend` |  |
| `ws_unmask_take_go` | bend | seed K +-10% | `src/core/http.bend` |  |
| `watch_copy_gate_none` | unported | seed R +-10% | `src/server/watch-plan.bend` |  |
| `watch_live_gate_kept` | unported | seed R +-10% | `src/server/watch-plan.bend` |  |
| `watch_copy_launch_none` | unported | seed R +-10% | `src/server/watch-plan.bend` |  |
| `watch_copy_check_none` | unported | seed R +-10% | `src/server/watch-plan.bend` |  |
| `watch_copy_stop_none` | unported | seed R +-10% | `src/server/watch-plan.bend` |  |
| `watch_copy_intent_none` | unported | seed R +-10% | `src/server/watch-plan.bend` |  |
| `copy_off_runs_all` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `copy_drops_outside` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `copy_keeps_inside` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `copy_git_write_told` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `copy_refuses_net` | unported | none | `-` |  |
| `copy_off_refuses_none` | unported | none | `-` |  |
| `adopt_copy_none` | bend | seed K +-10% | `src/core/adopt.bend` |  |
| `copy_file_open_told` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `pair_msg_kept` | unported | seed R +-10% | `src/server/pairing-info.bend` |  |
| `pair_msg_other_link` | unported | seed R +-10% | `src/server/pairing-info.bend` |  |
| `pair_msg_no_link_qr` | unported | seed R +-10% | `src/server/pairing-info.bend` |  |
| `thin_pair_told` | unported | seed R +-10% | `src/core/json.bend` |  |
| `far_feed_sliced` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_feed_sliced_one` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_feed_sliced_big` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_fact_shared` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_fact_private` | bend | seed K +-10% | `src/core/far.bend` |  |
| `locks_idle_never_set` | unported | seed R +-10% | `src/core/model.bend` |  |
| `locks_idle_released` | unported | seed R +-10% | `src/core/model.bend` |  |
| `locks_idle_held_not` | unported | seed R +-10% | `src/core/model.bend` |  |
| `locks_after_idle_none` | unported | seed R +-10% | `src/core/model.bend` |  |
| `locks_idle_text_exact` | bend | seed K +-10% | `src/core/locks.bend` |  |
| `auth_beat_kept` | unported | seed R +-10% | `src/core/model.bend` |  |
| `auth_beat_asked` | unported | seed R +-10% | `src/core/model.bend` |  |
| `triage_duplicate_delivery_quiet` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `triage_unknown_run_quiet` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `oc_lost_session` | bend | seed K +-10% | `src/core/opencode.bend` |  |
| `oc_other_folder_session` | bend | seed K +-10% | `src/core/opencode.bend` |  |
| `oc_tool_names_target` | bend | seed K +-10% | `src/core/opencode.bend` |  |
| `oc_tool_command_once` | bend | seed K +-10% | `src/core/opencode.bend` |  |
| `oc_notice_kind` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `oc_notice_info` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `oc_models_usage_none` | bend | seed K +-10% | `src/core/opencode.bend` |  |
| `oc_models_failed_says` | bend | seed K +-10% | `src/core/opencode.bend` |  |
| `oc_models_empty_says` | bend | seed K +-10% | `src/core/opencode.bend` |  |
| `oc_models_missing_quiet` | bend | seed K +-10% | `src/core/opencode.bend` |  |
| `pi_run_turn` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `omp_run_turn` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `interrupt_pi_stops` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `interrupt_omp_stops` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `pi_acp_like` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `omp_acp_like` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `pi_mode_supervised_asks` | bend | seed K +-10% | `src/core/pi.bend` |  |
| `pi_mode_accept_edits` | bend | seed K +-10% | `src/core/pi.bend` |  |
| `pi_mode_full_allows` | bend | seed K +-10% | `src/core/pi.bend` |  |
| `omp_mode_supervised_asks` | bend | seed K +-10% | `src/core/omp.bend` |  |
| `omp_mode_accept_edits` | bend | seed K +-10% | `src/core/omp.bend` |  |
| `omp_mode_full_allows` | bend | seed K +-10% | `src/core/omp.bend` |  |
| `pi_session_kept` | bend | seed K +-10% | `src/core/pi.bend` |  |
| `dialog_decline_denies` | bend | seed K +-10% | `src/core/dialog.bend` |  |
| `dialog_dismissed_denies` | bend | seed K +-10% | `src/core/dialog.bend` |  |
| `dialog_session_allows` | bend | seed K +-10% | `src/core/dialog.bend` |  |
| `pi_effort_flag` | bend | seed K +-10% | `src/core/agent.bend` |  |
| `pi_effort_unknown_default` | bend | seed K +-10% | `src/core/agent.bend` |  |
| `sidebar_wire_unissued` | unported | seed R +-10% | `src/server/thin-sidebar-wire.bend` |  |
| `sidebar_wire_wrong_ack` | unported | seed R +-10% | `src/server/thin-sidebar-wire.bend` |  |
| `active_rows_given_eq` | client | seed C +-10% | `src/core/client.bend` |  |
| `side_mine_work_independent` | client | seed C +-10% | `src/core/client.bend` |  |
| `side_signals_given_eq` | client | seed C +-10% | `src/core/client.bend` |  |
| `signal_cache_epoch_miss` | unported | seed R +-10% | `src/server/thin-signals.bend` |  |
| `signal_given_mine` | client | seed C +-10% | `src/core/client.bend` |  |
| `signal_finished_named` | bend | seed K +-10% | `src/core/signal.bend` |  |
| `signal_finished_keeps_rest` | bend | seed K +-10% | `src/core/signal.bend` |  |
| `signal_finished_needs_completed` | bend | seed K +-10% | `src/core/signal.bend` |  |
| `signal_finished_quiet` | bend | seed K +-10% | `src/core/signal.bend` |  |
| `signal_waiting_named` | bend | seed K +-10% | `src/core/signal.bend` |  |
| `signal_waiting_active` | bend | seed K +-10% | `src/core/signal.bend` |  |
| `signal_waiting_not_finished` | bend | seed K +-10% | `src/core/signal.bend` |  |
| `signal_waiting_keeps_rest` | bend | seed K +-10% | `src/core/signal.bend` |  |
| `signal_waiting_turn_over` | bend | seed K +-10% | `src/core/signal.bend` |  |
| `signal_activity_input` | unported | seed R +-10% | `src/core/model.bend` |  |
| `signal_assistant_input` | unported | seed R +-10% | `src/core/model.bend` |  |
| `signal_system_input` | unported | seed R +-10% | `src/core/model.bend` |  |
| `signal_user_revision` | unported | seed R +-10% | `src/core/model.bend` |  |
| `signal_pair_offline_work_hidden` | bend | seed K +-10% | `src/core/signal.bend` |  |
| `signal_pair_offline_block_kept` | bend | seed K +-10% | `src/core/signal.bend` |  |
| `side_ix_metadata_input` | client | seed C +-10% | `src/core/client.bend` |  |
| `side_input_activity` | client | seed C +-10% | `src/core/client.bend` |  |
| `side_input_assistant` | unported | seed R +-10% | `src/core/model.bend` |  |
| `side_input_system` | unported | seed R +-10% | `src/core/model.bend` |  |
| `sidebar_read_activity` | unported | seed R +-10% | `src/core/model.bend` |  |
| `sidebar_read_assistant` | unported | seed R +-10% | `src/core/model.bend` |  |
| `sidebar_read_system` | unported | seed R +-10% | `src/core/model.bend` |  |
| `signal_text_overlay_empty` | bend | seed K +-10% | `src/core/signal.bend` |  |
| `signal_text_overlay_known` | bend | seed K +-10% | `src/core/signal.bend` |  |
| `signal_text_overlay_online` | bend | seed K +-10% | `src/core/signal.bend` |  |
| `signal_text_overlay_fresh` | bend | seed K +-10% | `src/core/signal.bend` |  |
| `model_labels_given` | client | seed C +-10% | `src/core/client.bend` |  |
| `chat_derived_read` | unported | seed R +-10% | `src/core/json.bend` |  |
| `chat_picture_full_sequence` | unported | seed R +-10% | `src/server/thin-sidebar.bend` |  |
| `chat_sidebar_image_key` | client | seed C +-10% | `src/core/client.bend` |  |
| `chat_sidebar_image_settings_separate` | client | seed C +-10% | `src/core/client.bend` |  |
| `chat_sidebar_image_no_wire_authority` | unported | seed R +-10% | `src/server/thin-actor.bend` |  |
| `sidebar_shared_key_equal` | client | seed C +-10% | `src/core/client.bend` |  |
| `sidebar_shared_key_different` | client | seed C +-10% | `src/core/client.bend` |  |
| `sidebar_shared_key_settings` | client | seed C +-10% | `src/core/client.bend` |  |
| `sidebar_content_exact_sound` | unported | seed R +-10% | `src/core/json.bend` |  |
| `sidebar_content_wrong_scope` | unported | seed R +-10% | `src/core/json.bend` |  |
| `sidebar_sent_no_picture_authority` | unported | seed R +-10% | `src/server/thin-actor.bend` |  |
| `sidebar_full_branch_identity` | unported | seed R +-10% | `src/core/json.bend` |  |
| `sidebar_full_branch_exact` | unported | seed R +-10% | `src/core/json.bend` |  |
| `still_picture_disabled` | unported | seed R +-10% | `src/core/json.bend` |  |
| `still_picture_miss` | unported | seed R +-10% | `src/core/json.bend` |  |
| `still_picture_exact` | unported | seed R +-10% | `src/core/json.bend` |  |
| `sidebar_draft_body_not_input` | bend | seed K +-10% | `src/core/sidebar-drafts.bend` |  |
| `sidebar_draft_scratch_input` | bend | seed K +-10% | `src/core/sidebar-drafts.bend` |  |
| `sidebar_draft_settings_separate` | client | seed C +-10% | `src/core/client.bend` |  |
| `far_snap_live` | unported | none | `-` |  |
| `far_snap_fix` | unported | seed R +-10% | `src/core/model.bend` |  |
| `far_compact_live` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_compact_head` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_compact_tail` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_view_put_threads` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_view_put_projects` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_view_compact` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_v3_snap_when` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_v3_snap_shared` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_v3_snap_numbers` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_v3_snap_page` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_v3_page_full` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_v3_page_empty` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_v3_rows` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_v3_range` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_v3_skips` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_v3_lasts` | bend | seed K +-10% | `src/core/far.bend` |  |
| `far_items_projects` | unported | seed R +-10% | `src/core/json.bend` |  |
| `hot_demo_threads` | unported | none | `-` |  |
| `sec_box_handlers` | bend | seed K +-10% | `src/core/secret-box.bend` |  |
| `sec_mask_total` | bend | seed K +-10% | `src/core/secret.bend` |  |
| `sec_vault_valid` | bend | seed K +-10% | `src/core/secret.bend` |  |
| `sec_vault_total` | bend | seed K +-10% | `src/core/secret.bend` |  |
| `sec_mask_idem` | bend | seed K +-10% | `src/core/secret.bend` |  |
| `sec_mask_clean_id` | bend | seed K +-10% | `src/core/secret.bend` |  |
| `sec_empty_id` | bend | seed K +-10% | `src/core/secret.bend` |  |
| `sec_json_total` | bend | seed K +-10% | `src/core/secret.bend` |  |
| `sec_handle_shape` | unported | seed R +-10% | `src/core/json.bend` |  |
| `sec_changes_empty` | unported | seed R +-10% | `src/core/model.bend` |  |
| `tailnet_announce_off` | bend | seed K +-10% | `src/core/tailnet.bend` |  |
| `auth_owner_pair_untrusted` | bend | seed K +-10% | `src/core/auth.bend` |  |
| `auth_owner_pair_routes` | bend | seed K +-10% | `src/core/auth.bend` |  |
| `auth_owner_pair_rebound` | bend | seed K +-10% | `src/core/auth.bend` |  |
| `auth_pair_proof_needs_token` | bend | seed K +-10% | `src/core/auth.bend` |  |
| `auth_pair_proof_not_empty` | bend | seed K +-10% | `src/core/auth.bend` |  |
| `auth_pair_proof_stale` | bend | seed K +-10% | `src/core/auth.bend` |  |
| `fq_tools` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `fq_fields` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `fq_at` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `fq_order` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `fq_done_drops` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `fq_done_once` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `fq_old` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `fq_queue` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `fq_room` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `fq_window` | bend | seed K +-10% | `src/core/fmcp.bend` |  |
| `fq_replay_once` | bend | seed K +-10% | `src/core/fmcp.bend` |  |
| `fq_bytes_utf8` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `fq_room_keyed` | bend | seed K +-10% | `src/core/fmcp.bend` |  |
| `queue_copy_refuses_network` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `mirror_no_authority` | bend | seed K +-10% | `src/core/far.bend` |  |
| `mirror_no_source_state` | unported | seed R +-10% | `src/core/model.bend` |  |
| `mirror_local_epoch` | unported | seed R +-10% | `src/server/thin-mirror.bend` |  |
| `mirror_changed_basis_anchor` | unported | seed R +-10% | `src/server/thin-mirror.bend` |  |
| `retired_scratch_launch_refused` | bend | seed K +-10% | `src/core/mcp.bend` |  |
| `retired_scratch_id` | bend | seed K +-10% | `src/core/mcp.bend` |  |
| `legacy_active_visible` | unported | seed R +-10% | `src/core/model.bend` |  |
| `ref_origin_no_link` | bend | seed K +-10% | `src/core/ref-origin.bend` |  |
| `ref_origin_return_local` | bend | seed K +-10% | `src/core/ref-origin.bend` |  |
| `ref_origin_owner_remote` | bend | seed K +-10% | `src/core/ref-origin.bend` |  |
| `ref_origin_foreign_untouched` | bend | seed K +-10% | `src/core/ref-origin.bend` |  |
| `startup_warm_accepted` | unported | seed R +-10% | `src/server/hotboot.bend` |  |
| `startup_warm_fallback` | unported | seed R +-10% | `src/core/model.bend` |  |
| `mail_empty_success_quiet` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `mail_failed_empty_preserved` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `mail_no_triage_grants_outside_run` | bend | seed K +-10% | `src/core/maildisposition.bend` |  |
| `mail_silent_bot_envelope` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `codex_error_notice_kind` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `codex_error_notice_no_effects` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `life_fresh_owned_wakes` | unported | seed R +-10% | `src/core/model.bend` |  |
| `life_owned_progress_held` | unported | seed R +-10% | `src/core/model.bend` |  |
| `life_fresh_child_wakes` | unported | seed R +-10% | `src/core/model.bend` |  |
| `life_later_manual_settle_held` | unported | seed R +-10% | `src/core/model.bend` |  |
| `hook_off_unset` | bend | seed K +-10% | `src/core/wnote.bend` |  |
| `hook_on_set` | bend | seed K +-10% | `src/core/wnote.bend` |  |
| `hook_url_https` | bend | seed K +-10% | `src/core/wnote.bend` |  |
| `hook_url_file` | bend | seed K +-10% | `src/core/wnote.bend` |  |
| `hook_url_space` | bend | seed K +-10% | `src/core/wnote.bend` |  |
| `hook_live_blank` | bend | seed K +-10% | `src/core/wnote.bend` |  |
| `hook_mode_unsigned` | bend | seed K +-10% | `src/core/wnote.bend` |  |
| `hook_mode_default_file` | bend | seed K +-10% | `src/core/wnote.bend` |  |
| `hook_mode_missing` | bend | seed K +-10% | `src/core/wnote.bend` |  |
| `hook_mode_configured` | bend | seed K +-10% | `src/core/wnote.bend` |  |
| `hook_stopped_word` | unported | seed R +-10% | `src/core/model.bend` |  |
| `hook_failed_word` | unported | seed R +-10% | `src/core/model.bend` |  |
| `hook_completed_word` | unported | seed R +-10% | `src/core/model.bend` |  |
| `hook_body_state` | unported | seed R +-10% | `src/core/json.bend` |  |
| `hook_body_event` | unported | seed R +-10% | `src/core/json.bend` |  |
| `hook_body_kind` | unported | seed R +-10% | `src/core/json.bend` |  |
| `hook_body_ask_event` | unported | seed R +-10% | `src/core/json.bend` |  |
| `hook_unsigned_header` | bend | seed K +-10% | `src/core/wnote.bend` |  |
| `hook_signed_header` | bend | seed K +-10% | `src/core/wnote.bend` |  |
| `hook_secret_default` | bend | seed K +-10% | `src/core/wnote.bend` |  |
| `hook_secret_path` | bend | seed K +-10% | `src/core/wnote.bend` |  |
| `hook_report_keeps_end` | unported | seed R +-10% | `src/core/model.bend` |  |
| `hook_report_keeps_fail` | unported | seed R +-10% | `src/core/model.bend` |  |
| `hook_report_fallback` | unported | seed R +-10% | `src/core/model.bend` |  |
| `hook_follows_settle` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `hook_follows_stop` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `hook_follows_ask` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `hook_follows_ask_off` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `hook_follows_settled_off` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `hook_follows_fail` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `hook_follows_reminder` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `hook_follows_all` | bend | seed K +-10% | `src/core/notice.bend` |  |
| `hook_route_done` | bend | seed K +-10% | `src/core/wnote.bend` |  |
| `hook_route_fail` | bend | seed K +-10% | `src/core/wnote.bend` |  |
| `hook_route_ask` | bend | seed K +-10% | `src/core/wnote.bend` |  |
| `hook_route_merged` | bend | seed K +-10% | `src/core/wnote.bend` |  |
| `hook_route_reminder` | bend | seed K +-10% | `src/core/wnote.bend` |  |
| `hook_route_note` | bend | seed K +-10% | `src/core/wnote.bend` |  |
| `hook_route_quiet` | bend | seed K +-10% | `src/core/wnote.bend` |  |
| `accounts_legacy_allowed` | bend | seed K +-10% | `src/core/accounts.bend` |  |
| `accounts_legacy_catalog` | unported | seed R +-10% | `src/core/json.bend` |  |
| `accounts_profile_sessions_differ` | bend | seed K +-10% | `src/core/accounts.bend` |  |
| `accounts_identity_sessions_differ` | bend | seed K +-10% | `src/core/accounts.bend` |  |
| `native_history_page_visible` | bend | seed K +-10% | `src/core/history-window.bend` |  |
| `native_history_page_size` | bend | seed K +-10% | `src/core/history-window.bend` |  |
| `native_history_page_order` | bend | seed K +-10% | `src/core/history-window.bend` |  |
| `timeline_virtual_prepend_offset` | bend | seed K +-10% | `src/core/timeline-virtual.bend` |  |
| `timeline_virtual_anchor_rendered` | bend | seed K +-10% | `src/core/timeline-virtual.bend` |  |
| `timeline_virtual_follow_only_at_bottom` | bend | seed K +-10% | `src/core/timeline-virtual.bend` |  |
| `private_no_implicit_publication` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `private_duplicate_publication_none` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `liveness_stalled_only_after_gap` | bend | seed K +-10% | `src/core/liveness.bend` |  |
| `liveness_nudge_once_per_gap` | bend | seed K +-10% | `src/core/liveness.bend` |  |
| `liveness_never_kills` | bend | seed K +-10% | `src/core/liveness.bend` |  |
| `liveness_launcher_notified_once` | bend | seed K +-10% | `src/core/liveness.bend` |  |
| `liveness_progress_clears_stall` | bend | seed K +-10% | `src/core/liveness.bend` |  |
| `liveness_nudge_before_stall` | bend | seed K +-10% | `src/core/liveness.bend` |  |
| `liveness_timeout_explicit_optout` | bend | seed K +-10% | `src/core/liveness.bend` |  |
| `lock_lease_busy_retains_exclusion` | bend | seed K +-10% | `src/core/locks.bend` |  |
| `lock_lease_warns_before_release` | bend | seed K +-10% | `src/core/locks.bend` |  |
| `lock_lease_grace_before_release` | bend | seed K +-10% | `src/core/locks.bend` |  |
| `lock_lease_exact_threshold` | bend | seed K +-10% | `src/core/locks.bend` |  |
| `work_timeout_default_twenty_minutes` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `work_timeout_zero_optout` | unported | seed R +-10% | `src/core/json.bend` |  |
| `provider_resume_waits_for_ready` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `background_timeout_waits_for_deadline` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `liveness_display_active_without_stall` | unported | seed R +-10% | `src/core/model.bend` |  |
| `liveness_plane_stalled_needs_attention` | bend | seed K +-10% | `src/core/plane.bend` |  |
| `liveness_label_preserves_busy_work` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `restart_retry_initial_delay` | bend | seed K +-10% | `src/core/restartretry.bend` |  |
| `restart_retry_delay_bounded` | bend | seed K +-10% | `src/core/restartretry.bend` |  |
| `restart_retry_not_ready_no_resume` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `restart_retry_not_candidate_no_schedule` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `automatic_ride_no_process_effects` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `todo_person_field_survives` | bend | seed K +-10% | `src/core/todo.bend` |  |
| `todo_person_item_survives_snapshot` | bend | seed K +-10% | `src/core/todo.bend` |  |
| `todo_person_delete_survives_snapshot` | bend | seed K +-10% | `src/core/todo.bend` |  |
| `todo_agent_explicit_change` | bend | seed K +-10% | `src/core/todo.bend` |  |
| `todo_complete_auto_clears` | unported | seed R +-10% | `src/core/model.bend` |  |
| `todo_next_turn_hides_old` | unported | seed R +-10% | `src/core/model.bend` |  |
| `deploy_idle_waits_between_calls` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `deploy_idle_goes_after_turn_end` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `todo_hidden_turn_retires` | unported | seed R +-10% | `src/core/model.bend` |  |
| `todo_write_after_retire_fresh` | unported | seed R +-10% | `src/core/model.bend` |  |
| `todos_retire_shuts` | unported | seed R +-10% | `src/core/model.bend` |  |
| `todos_retire_shuts_unselected` | unported | seed R +-10% | `src/core/model.bend` |  |
| `plot_source_starts_unread` | unported | seed R +-10% | `src/server/plot.bend` |  |
| `intent_borrow_keeps_draft` | client | seed C +-10% | `src/core/client.bend` |  |
| `intent_borrow_twice_keeps_draft` | client | seed C +-10% | `src/core/client.bend` |  |
| `intent_chat_send_forgets_backup` | client | seed C +-10% | `src/core/client.bend` |  |
| `room_draft_key_is_scoped` | client | seed C +-10% | `src/core/client.bend` |  |
| `room_draft_read_is_scoped` | client | seed C +-10% | `src/core/client.bend` |  |
| `routine_invalid_keeps_form` | client | seed C +-10% | `src/core/client.bend` |  |
| `checkpoint_stopped_not_restored` | unported | seed R +-10% | `src/core/json.bend` |  |
| `checkpoint_anchor_ignores_activity` | unported | seed R +-10% | `src/core/model.bend` |  |
| `checkpoint_empty_buffers_omitted` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fm_restore_requests_root` | client | seed C +-10% | `src/core/client.bend` |  |
| `fm_refresh_keeps_received_request` | client | seed C +-10% | `src/core/client.bend` |  |
| `fm_closed_never_requests` | client | seed C +-10% | `src/core/client.bend` |  |
| `todos_open_token` | unported | seed R +-10% | `src/core/model.bend` |  |
| `todos_adopt_whole_retire_shuts` | client | seed C +-10% | `src/core/client.bend` |  |
| `todos_thin_click_holds` | unported | none | `-` |  |
| `wt_shared_kept` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `wt_shared_last_goes` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `wt_heal_worktree` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `wt_heal_project_folder_never` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `fold_blank_joins` | client | seed C +-10% | `src/core/fold.bend` |  |
| `fold_ctx_carries_head` | client | seed C +-10% | `src/core/fold.bend` |  |
| `fold_ctx_after_live` | unported | seed R +-10% | `src/core/model.bend` |  |
| `fold_ctx_thirteen` | client | seed C +-10% | `src/core/fold.bend` |  |
| `fold_ctx_text_splits` | client | seed C +-10% | `src/core/fold.bend` |  |
| `keys_navigation_disarms` | client | seed C +-10% | `src/core/client.bend` |  |
| `voice_category_requests_status` | client | seed C +-10% | `src/core/client.bend` |  |
| `account_blank_label_no_request` | client | seed C +-10% | `src/core/client.bend` |  |
| `bot_invalid_name_keeps_persona` | client | seed C +-10% | `src/core/client.bend` |  |
| `key_long_name_refused` | bend | seed K +-10% | `src/core/apikeys.bend` |  |
| `key_normalization_refused` | bend | seed K +-10% | `src/core/apikeys.bend` |  |
| `recurring_invalid_interval_help` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `routine_delete_first_press_preserves` | client | seed C +-10% | `src/core/client.bend` |  |
| `bot_navigation_restores_tab` | unported | seed R +-10% | `src/core/json.bend` |  |
| `checkpoint_untouched_not_rewritten` | bend | seed K +-10% | `src/core/live-checkpoint.bend` |  |
| `checkpoint_touched_once` | unported | seed R +-10% | `src/core/model.bend` |  |
| `revert_ask_sends_nothing` | client | seed C +-10% | `src/core/client.bend` |  |
| `revert_turn_sent_at_once` | client | seed C +-10% | `src/core/client.bend` |  |
| `intent_kept_backup_restores` | client | seed C +-10% | `src/core/client.bend` |  |
| `plot_board_skips_tree_wait` | client | seed C +-10% | `src/core/client.bend` |  |
| `plot_first_schematic_waits_for_tree` | client | seed C +-10% | `src/core/client.bend` |  |
| `plot_live_tree_path` | unported | seed R +-10% | `src/server/plot.bend` |  |
| `plot_version_tree_path` | unported | seed R +-10% | `src/server/plot.bend` |  |
| `plot_comparison_tree_path` | unported | seed R +-10% | `src/server/plot.bend` |  |
| `estimate_partial_withholds_total` | unported | none | `-` |  |
| `estimate_complete_keeps_total` | unported | none | `-` |  |
| `estimate_failure_keeps_preparation` | unported | none | `-` |  |
| `estimate_empty_fab_names_file` | unported | seed R +-10% | `src/core/json.bend` |  |
| `estimate_unreadable_fab_names_file` | unported | none | `-` |  |
| `buy_thread_never_nests` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `buy_rapid_repeat_same_intent` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `buy_later_intent_is_new` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `order_one_file_singular` | unported | none | `-` |  |
| `order_many_files_plural` | unported | none | `-` |  |
| `signal_failed_dot_truthful` | unported | seed R +-10% | `src/core/model.bend` |  |
| `signal_asked_dot_stays_blocked` | unported | seed R +-10% | `src/core/model.bend` |  |
| `settings_settle_names_idle_timer` | client | seed C +-10% | `src/core/client.bend` |  |
| `settings_settle_idle_off_truthful` | client | seed C +-10% | `src/core/client.bend` |  |
| `estimate_snapshot_is_explicit` | unported | none | `-` |  |
| `project_pasted_path_trimmed` | client | seed C +-10% | `src/core/client.bend` |  |
| `search_hides_design_history` | unported | seed R +-10% | `src/core/model.bend` |  |
| `project_blank_filter_keeps` | client | seed C +-10% | `src/core/client.bend` |  |
| `tour_preview_other_steps_keep_selection` | client | seed C +-10% | `src/core/client.bend` |  |
| `tour_tools_examples_visible` | client | seed C +-10% | `src/core/client.bend` |  |
| `search_unicode_spellings_agree` | bend | seed K +-10% | `src/core/search-fold.bend` |  |
| `stop_command_only_while_running` | client | seed C +-10% | `src/core/client.bend` |  |
| `terminal_scroll_bounds` | bend | seed K +-10% | `src/core/term-scroll.bend` |  |
| `terminal_empty_input_keeps_view` | client | seed C +-10% | `src/core/client.bend` |  |
| `browser_frame_empty_ignored` | bend | seed K +-10% | `src/core/browser-frame.bend` |  |
| `browser_frame_input_owner` | bend | seed K +-10% | `src/core/browser-frame.bend` |  |
| `browser_frame_bot_owner` | bend | seed K +-10% | `src/core/browser-frame.bend` |  |
| `terminal_reopen_keeps_screen` | unported | seed R +-10% | `src/server/term.bend` |  |
| `terminal_new_process_starts_blank` | unported | seed R +-10% | `src/server/term.bend` |  |
| `terminal_unicode_cell_widths` | client | seed C +-10% | `src/web/dom.bend` |  |
| `model_picker_replaces_accounts` | client | seed C +-10% | `src/core/client.bend` |  |
| `checks_detail_cap` | bend | seed K +-10% | `src/core/checks.bend` |  |
| `checks_omitted_visible` | bend | seed K +-10% | `src/core/checks.bend` |  |
| `viewer_report_keeps_board` | client | seed C +-10% | `src/core/client.bend` |  |
| `viewer_part_keeps_board` | client | seed C +-10% | `src/core/client.bend` |  |
| `fork_drawing_follows_worktree` | client | seed C +-10% | `src/core/client.bend` |  |
| `fork_report_is_not_a_file` | client | seed C +-10% | `src/core/client.bend` |  |
| `review_reading_place_names_page` | client | seed C +-10% | `src/core/client.bend` |  |
| `viewer_wire_empty_target_unchanged` | unported | seed R +-10% | `src/core/json.bend` |  |
| `viewer_sheet_keeps_board_anchor` | client | seed C +-10% | `src/core/client.bend` |  |
| `viewer_selected_root_before_card` | client | seed C +-10% | `src/core/client.bend` |  |
| `viewer_selected_board_id_before_card` | client | seed C +-10% | `src/core/client.bend` |  |
| `viewer_same_board_version_keeps_root` | client | seed C +-10% | `src/core/client.bend` |  |
| `viewer_tree_root_stays_inside_thread` | client | seed C +-10% | `src/core/client.bend` |  |
| `revert_entry_is_newest` | unported | seed R +-10% | `src/core/model.bend` |  |
| `revert_unarmed_is_off` | client | seed C +-10% | `src/core/client.bend` |  |
| `mobile_text_is_filepage` | client | seed C +-10% | `src/mobile/view.bend` |  |
| `plot_error_missing_keeps_filename` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `component_fragment_keeps_fitting_row` | unported | seed R +-10% | `src/core/json.bend` |  |
| `component_fragment_keeps_field` | unported | seed R +-10% | `src/core/json.bend` |  |
| `component_fragment_only_last_field_completes` | unported | seed R +-10% | `src/core/json.bend` |  |
| `copy_keeps_viewer_reads` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `sdb_up_parents_exact` | unported | seed R +-10% | `src/core/model.bend` |  |
| `sdb_up_parents_demo` | unported | seed R +-10% | `src/core/model.bend` |  |
| `sdb_up_keep_kinds` | unported | seed R +-10% | `src/core/model.bend` |  |
| `sdb_up_lines_decoded` | unported | seed R +-10% | `src/core/model.bend` |  |
| `sdb_up_from` | unported | none | `-` |  |
| `phone_subs_open_routed` | unported | none | `-` |  |
| `phone_subs_shut_routed` | unported | none | `-` |  |
| `phone_subs_fold_routed` | unported | none | `-` |  |
| `phone_subs_cancel_routed` | unported | none | `-` |  |
| `delegation_captures_baseline_before_agent` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `signal_failed_display_name` | unported | seed R +-10% | `src/core/model.bend` |  |
| `signal_pending_display_blocked` | unported | seed R +-10% | `src/core/model.bend` |  |
| `composer_slash_local` | unported | seed R +-10% | `src/core/json.bend` |  |
| `composer_slash_not_claude` | unported | seed R +-10% | `src/core/json.bend` |  |
| `composer_hub_rows_same_draft` | unported | seed R +-10% | `src/core/json.bend` |  |
| `composer_hub_rows_no_slash` | unported | seed R +-10% | `src/core/json.bend` |  |
| `composer_slash_overtakes_hub` | unported | seed R +-10% | `src/core/json.bend` |  |
| `composer_stop_running_blank` | unported | seed R +-10% | `src/core/json.bend` |  |
| `composer_send_running_typed` | unported | seed R +-10% | `src/core/json.bend` |  |
| `composer_screen_agrees` | unported | seed R +-10% | `src/core/json.bend` |  |
| `composer_no_slash_no_rows` | unported | seed R +-10% | `src/core/json.bend` |  |
| `composer_fields` | client | seed C +-10% | `src/mobile/session.bend` |  |
| `view_delta_same` | unported | seed R +-10% | `src/core/json.bend` |  |
| `view_delta_empty_same` | unported | seed R +-10% | `src/core/json.bend` |  |
| `view_delta_keyed_round` | unported | seed R +-10% | `src/core/json.bend` |  |
| `view_delta_entry_round` | unported | seed R +-10% | `src/core/json.bend` |  |
| `view_delta_unkeyed_round` | unported | seed R +-10% | `src/core/json.bend` |  |
| `view_delta_keys_round` | unported | seed R +-10% | `src/core/json.bend` |  |
| `view_delta_keyed_ops` | unported | seed R +-10% | `src/core/json.bend` |  |
| `view_delta_memo_key` | unported | seed R +-10% | `src/core/json.bend` |  |
| `draft_bound_slash_word` | bend | seed K +-10% | `src/core/slash.bend` |  |
| `draft_bound_slash_spaced` | bend | seed K +-10% | `src/core/slash.bend` |  |
| `draft_bound_give` | client | seed C +-10% | `src/core/client.bend` |  |
| `draft_bound_btw` | client | seed C +-10% | `src/core/client.bend` |  |
| `draft_bound_token` | bend | seed K +-10% | `src/core/refs.bend` |  |
| `draft_bound_token_nospace` | bend | seed K +-10% | `src/core/refs.bend` |  |
| `draft_bound_say_within` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `draft_bound_say_over` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `draft_bound_none` | unported | none | `-` |  |
| `view_page_older_next_whole` | unported | none | `-` |  |
| `view_page_older_next_partial` | unported | none | `-` |  |
| `refs_blank_empty` | bend | seed K +-10% | `src/core/refs.bend` |  |
| `refs_blank_spaces` | bend | seed K +-10% | `src/core/refs.bend` |  |
| `refs_blank_word` | bend | seed K +-10% | `src/core/refs.bend` |  |
| `refs_blank_trailing` | bend | seed K +-10% | `src/core/refs.bend` |  |
| `slash_typing_needs_slash` | bend | seed K +-10% | `src/core/slash.bend` |  |
| `refs_word_padded` | bend | seed K +-10% | `src/core/refs.bend` |  |
| `refs_word_longer` | bend | seed K +-10% | `src/core/refs.bend` |  |
| `refs_word_more` | bend | seed K +-10% | `src/core/refs.bend` |  |
| `refs_word_empty` | bend | seed K +-10% | `src/core/refs.bend` |  |
| `thin_review_index_whole` | unported | seed R +-10% | `src/core/model.bend` |  |
| `thin_review_index_isolated` | unported | seed R +-10% | `src/core/model.bend` |  |
| `review_history_keeps_newest` | unported | seed R +-10% | `src/core/model.bend` |  |
| `review_history_overlap_once` | unported | seed R +-10% | `src/core/model.bend` |  |
| `sdb_terms_due` | unported | none | `-` |  |
| `sdb_terms_sql_same` | unported | none | `-` |  |
| `viewclock_not_after` | unported | none | `-` |  |
| `viewclock_within_step` | unported | none | `-` |  |
| `viewclock_step_same` | unported | none | `-` |  |
| `viewclock_next_step` | unported | none | `-` |  |
| `view_memo_anchor_left_out` | unported | seed R +-10% | `src/core/json.bend` |  |
| `pace_live_own` | unported | none | `-` |  |
| `pace_live_other` | unported | none | `-` |  |
| `pace_live_none` | unported | none | `-` |  |
| `pace_live_committed_still` | unported | none | `-` |  |
| `dq_keeps_order` | bend | seed K +-10% | `src/core/diskq.bend` |  |
| `dq_counts` | bend | seed K +-10% | `src/core/diskq.bend` |  |
| `dq_room` | bend | seed K +-10% | `src/core/diskq.bend` |  |
| `dq_full_soft` | bend | seed K +-10% | `src/core/diskq.bend` |  |
| `dq_backoff` | bend | seed K +-10% | `src/core/diskq.bend` |  |
| `dq_due` | bend | seed K +-10% | `src/core/diskq.bend` |  |
| `dq_no_external_before_written` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `dq_held_in_order` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `dq_changes_in_order` | unported | seed R +-10% | `src/core/model.bend` |  |
| `land_outside_page` | bend | seed K +-10% | `src/core/landing.bend` |  |
| `land_root_page` | bend | seed K +-10% | `src/core/landing.bend` |  |
| `land_window_only` | bend | seed K +-10% | `src/core/landing.bend` |  |
| `land_timeline_only` | bend | seed K +-10% | `src/core/landing.bend` |  |
| `view_memo_key_page_last` | unported | seed R +-10% | `src/server/thin-actor.bend` |  |
| `view_page_memo_same_step` | unported | seed R +-10% | `src/core/json.bend` |  |
| `view_page_memo_next_step` | unported | seed R +-10% | `src/core/json.bend` |  |
| `view_page_memo_other_inputs` | unported | seed R +-10% | `src/core/json.bend` |  |
| `view_page_memo_not_fitted` | unported | seed R +-10% | `src/core/json.bend` |  |
| `view_page_still_same_step` | unported | seed R +-10% | `src/core/json.bend` |  |
| `view_page_still_next_step` | unported | seed R +-10% | `src/core/json.bend` |  |
| `view_page_still_not_fitted` | unported | seed R +-10% | `src/core/json.bend` |  |
| `view_page_splice_off_none` | unported | seed R +-10% | `src/server/thin-actor.bend` |  |
| `view_page_splice_default_off` | unported | seed R +-10% | `src/core/model.bend` |  |
| `view_page_splice_setting_on` | unported | seed R +-10% | `src/core/model.bend` |  |
| `timeline_valid_end_empty` | bend | seed K +-10% | `src/core/timeline-virtual.bend` |  |
| `timeline_valid_edge_untaken` | bend | seed K +-10% | `src/core/timeline-virtual.bend` |  |
| `plane_tally_words_plain` | bend | seed K +-10% | `src/core/plane.bend` |  |
| `plane_tally_words_rich` | bend | seed K +-10% | `src/core/plane.bend` |  |
| `plane_tally_words_stalled` | bend | seed K +-10% | `src/core/plane.bend` |  |
| `plane_tally_counts_stalled` | bend | seed K +-10% | `src/core/plane.bend` |  |
| `plane_list_same_stalled` | bend | seed K +-10% | `src/core/plane.bend` |  |
| `uber_rows_same_count` | bend | seed K +-10% | `src/core/plane.bend` |  |
| `side_rows_ix_tree_eq` | client | seed C +-10% | `src/core/client.bend` |  |
| `ride_wire_blank` | bend | seed K +-10% | `src/core/codec.bend` |  |
| `ride_commit_line_is_wire` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `ride_store_wire` | unported | seed R +-10% | `src/core/model.bend` |  |
| `ride_state_kept` | unported | seed R +-10% | `src/core/model.bend` |  |
| `ride_note_claude_now` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `ride_note_claude_given` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `ride_note_codex_rides` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `ride_note_idle_rides` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `ride_note_news` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `wait_news_done` | bend | seed K +-10% | `src/core/mcp.bend` |  |
| `timeline_layout_list_same` | unported | seed R +-10% | `src/core/json.bend` |  |
| `view_window_newer_empty` | unported | none | `-` |  |
| `att_max_default` | bend | seed K +-10% | `src/core/attach.bend` |  |
| `att_max_reads` | bend | seed K +-10% | `src/core/attach.bend` |  |
| `att_max_ceiling` | bend | seed K +-10% | `src/core/attach.bend` |  |
| `att_max_floor` | bend | seed K +-10% | `src/core/attach.bend` |  |
| `att_max_junk` | bend | seed K +-10% | `src/core/attach.bend` |  |
| `att_message_holds_path` | bend | seed K +-10% | `src/core/attach.bend` |  |
| `range_whole` | bend | seed K +-10% | `src/core/range.bend` |  |
| `range_span` | bend | seed K +-10% | `src/core/range.bend` |  |
| `range_open` | bend | seed K +-10% | `src/core/range.bend` |  |
| `range_tail` | bend | seed K +-10% | `src/core/range.bend` |  |
| `range_clip` | bend | seed K +-10% | `src/core/range.bend` |  |
| `range_past` | bend | seed K +-10% | `src/core/range.bend` |  |
| `range_multi` | bend | seed K +-10% | `src/core/range.bend` |  |
| `range_head_partial` | bend | seed K +-10% | `src/core/range.bend` |  |
| `improvements_retry_no_changes` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `improvements_retry_one_reply` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `vd_fix_empty` | bend | seed K +-10% | `src/core/vdict.bend` |  |
| `vd_learn_trusted` | bend | seed K +-10% | `src/core/vdict.bend` |  |
| `vd_undo_revokes` | bend | seed K +-10% | `src/core/vdict.bend` |  |
| `vs_done_tail_empty` | bend | seed K +-10% | `src/core/vdict.bend` |  |
| `vs_done_resets` | bend | seed K +-10% | `src/core/vdict.bend` |  |
| `alpha_usage_fixed_fields` | bend | seed K +-10% | `src/core/usage-counts.bend` |  |
| `alpha_usage_closed_snapshot` | unported | seed R +-10% | `src/core/json.bend` |  |
| `alpha_usage_closed_tokens` | unported | seed R +-10% | `src/core/model.bend` |  |
| `alpha_usage_mirror_excluded` | unported | seed R +-10% | `src/core/model.bend` |  |
| `provider_setup_missing_program` | unported | seed R +-10% | `src/core/json.bend` |  |
| `provider_setup_failure_retry` | bend | seed K +-10% | `src/core/provider-setup.bend` |  |
| `alpha_usage_empty_choice_declines` | client | seed C +-10% | `src/core/client.bend` |  |
| `tool_short_mcp` | client | seed C +-10% | `src/core/fold.bend` |  |
| `tool_short_server_underscores` | client | seed C +-10% | `src/core/fold.bend` |  |
| `tool_short_plain` | client | seed C +-10% | `src/core/fold.bend` |  |
| `fold_label_leaves_todo_out` | unported | seed R +-10% | `src/core/model.bend` |  |
| `fold_live_names_call` | unported | seed R +-10% | `src/core/model.bend` |  |
| `tool_summary_todo` | unported | seed R +-10% | `src/core/json.bend` |  |
| `stamp_mid_never_user` | unported | seed R +-10% | `src/core/model.bend` |  |
| `stamp_mid_inside_turn` | unported | seed R +-10% | `src/core/model.bend` |  |
| `stamp_mid_turn_end_shows` | unported | seed R +-10% | `src/core/model.bend` |  |
| `stamp_mid_running_newest_shows` | unported | seed R +-10% | `src/core/model.bend` |  |
| `subs_step_label_mcp` | client | seed C +-10% | `src/core/subs.bend` |  |
| `subs_step_label_say` | client | seed C +-10% | `src/core/subs.bend` |  |
| `star_nudge_rejected_accept` | unported | seed R +-10% | `src/core/model.bend` |  |
| `star_nudge_rejected_decision` | unported | seed R +-10% | `src/core/json.bend` |  |
| `star_nudge_official_repository` | bend | seed K +-10% | `src/core/star-nudge.bend` |  |
| `star_nudge_saved_send_waits` | bend | seed K +-10% | `src/core/diskq.bend` |  |
| `settings_proposal_decline_no_write` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `settings_proposal_deadline_exclusive` | bend | seed K +-10% | `src/core/settingsproposal.bend` |  |
| `settings_proposal_write_invalidates` | unported | seed R +-10% | `src/core/model.bend` |  |
| `fallback_settings_agents_row` | client | seed C +-10% | `src/core/client.bend` |  |
| `chat_pending_working` | bend | seed K +-10% | `src/core/chat-pending.bend` |  |
| `chat_pending_blocked` | bend | seed K +-10% | `src/core/chat-pending.bend` |  |
| `chat_pending_finished` | bend | seed K +-10% | `src/core/chat-pending.bend` |  |
| `chat_pending_no_partial_reply` | bend | seed K +-10% | `src/core/chat-pending.bend` |  |
| `timeline_fit_disabled` | bend | seed K +-10% | `src/core/timeline-virtual.bend` |  |
| `timeline_unbounded_position` | bend | seed K +-10% | `src/core/timeline-virtual.bend` |  |
| `sending_preview_excludes_secret` | client | seed C +-10% | `src/core/client.bend` |  |
| `lazy_tools_open_row_unchanged` | bend | seed K +-10% | `src/core/lazy-tools.bend` |  |
| `lazy_tools_shut_row_ordinal` | bend | seed K +-10% | `src/core/lazy-tools.bend` |  |
| `lazy_tools_shut_row_blocks` | bend | seed K +-10% | `src/core/lazy-tools.bend` |  |
| `lazy_tools_shut_row_text` | bend | seed K +-10% | `src/core/lazy-tools.bend` |  |
| `telemetry_default_selected` | bend | seed K +-10% | `src/core/telemetry-choice.bend` |  |
| `telemetry_default_pending` | bend | seed K +-10% | `src/core/telemetry-choice.bend` |  |
| `telemetry_all_off` | bend | seed K +-10% | `src/core/telemetry-choice.bend` |  |
| `telemetry_existing_optout_answered` | bend | seed K +-10% | `src/core/telemetry-choice.bend` |  |
| `telemetry_runtime_closed` | bend | seed K +-10% | `src/core/telemetry.bend` |  |
| `tm_off_view_current` | unported | seed R +-10% | `src/core/model.bend` |  |
| `tm_on_view_current` | unported | seed R +-10% | `src/core/model.bend` |  |
| `tm_permit_waits_for_disk` | bend | seed K +-10% | `src/core/telemetry.bend` |  |
| `tm_start_waits_for_disk` | bend | seed K +-10% | `src/core/telemetry.bend` |  |
| `tm_start_cleans_up_waiting_off` | bend | seed K +-10% | `src/core/telemetry.bend` |  |
| `telemetry_choice_no_decision_no_replay` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `tour_back_skips_usage` | client | seed C +-10% | `src/core/client.bend` |  |
| `tour_next_skips_usage` | client | seed C +-10% | `src/core/onboard.bend` |  |
| `tour_usage_kept_when_due` | client | seed C +-10% | `src/core/onboard.bend` |  |
| `settings_row_information` | client | seed C +-10% | `src/core/client.bend` |  |
| `voice_model_needs_runtime` | unported | seed R +-10% | `src/core/json.bend` |  |
| `pcb_usage_fixed_fields` | bend | seed K +-10% | `src/core/usage-counts.bend` |  |
| `project_drive_root_absolute` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `project_unc_root` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `plot_stamp_opt_out` | unported | seed R +-10% | `src/server/plot.bend` |  |
| `plot_stamp_other_client` | unported | seed R +-10% | `src/server/plot.bend` |  |
| `allowance_remaining_reported` | bend | seed K +-10% | `src/core/allowance.bend` |  |
| `allowance_refresh_held` | bend | seed K +-10% | `src/core/allowance.bend` |  |
| `allowance_refresh_due` | bend | seed K +-10% | `src/core/allowance.bend` |  |
| `allowance_future_unknown` | bend | seed K +-10% | `src/core/allowance.bend` |  |
| `issue_report_new_default` | bend | seed K +-10% | `src/core/issue-report.bend` |  |
| `issue_report_legacy_internal` | bend | seed K +-10% | `src/core/issue-report.bend` |  |
| `issue_report_explicit_issue` | bend | seed K +-10% | `src/core/issue-report.bend` |  |
| `issue_report_explicit_internal` | bend | seed K +-10% | `src/core/issue-report.bend` |  |
| `issue_report_verbatim_text` | bend | seed K +-10% | `src/core/issue-report.bend` |  |
| `issue_report_manual_details_verbatim` | bend | seed K +-10% | `src/core/issue-report.bend` |  |
| `issue_report_diagnostics_absent` | bend | seed K +-10% | `src/core/issue-report.bend` |  |
| `issue_report_os_description_closed` | bend | seed K +-10% | `src/core/issue-report.bend` |  |
| `issue_report_version_omits_build` | bend | seed K +-10% | `src/core/issue-report.bend` |  |
| `issue_report_version_rejects_path` | bend | seed K +-10% | `src/core/issue-report.bend` |  |
| `issue_report_preview_retains_body` | bend | seed K +-10% | `src/core/issue-report.bend` |  |
| `issue_report_url_overflow_closed` | bend | seed K +-10% | `src/core/issue-report.bend` |  |
| `issue_report_official_repository` | bend | seed K +-10% | `src/core/issue-report.bend` |  |
| `issue_report_template_revision` | bend | seed K +-10% | `src/core/issue-report.bend` |  |
| `issue_report_prompt_original_verbatim` | bend | seed K +-10% | `src/core/issue-report.bend` |  |
| `active_progress_other_soft` | unported | seed R +-10% | `src/core/model.bend` |  |
| `active_progress_owner_urgent` | unported | seed R +-10% | `src/core/model.bend` |  |
| `usage_rolling_week` | bend | seed K +-10% | `src/core/usage_page.bend` |  |
| `usage_rolling_two_weeks` | bend | seed K +-10% | `src/core/usage_page.bend` |  |
| `usage_rolling_four_weeks` | bend | seed K +-10% | `src/core/usage_page.bend` |  |
| `usage_rolling_month` | bend | seed K +-10% | `src/core/usage_page.bend` |  |
| `usage_rolling_default` | bend | seed K +-10% | `src/core/usage_page.bend` |  |
| `usage_rolling_empty_unknown` | bend | seed K +-10% | `src/core/usage_page.bend` |  |
| `subs_display_shell_background` | client | seed C +-10% | `src/core/subs.bend` |  |
| `subs_display_child_conversation` | client | seed C +-10% | `src/core/subs.bend` |  |
| `subs_display_finished_toggle_kept` | client | seed C +-10% | `src/core/subs.bend` |  |
| `subthreads_claude_blocks_native` | unported | seed R +-10% | `src/core/model.bend` |  |
| `pending_projection_unfitted` | client | seed C +-10% | `src/core/client.bend` |  |
| `pending_preview_whole` | client | seed C +-10% | `src/core/client.bend` |  |
| `pending_preview_ellipsis` | client | seed C +-10% | `src/core/client.bend` |  |
| `json_surrogate_pair` | unported | seed R +-10% | `src/core/json.bend` |  |
| `fm_empty_root_ready` | client | seed C +-10% | `src/core/client.bend` |  |
| `fm_unanswered_root_waits` | client | seed C +-10% | `src/core/client.bend` |  |
| `fm_previous_request_waits` | client | seed C +-10% | `src/core/client.bend` |  |
| `fm_other_folder_waits` | client | seed C +-10% | `src/core/client.bend` |  |
| `fm_previous_reply_rejected` | client | seed C +-10% | `src/core/client.bend` |  |
| `fm_root_reconnect_requested` | client | seed C +-10% | `src/core/client.bend` |  |
| `path_records_newline_is_data` | bend | seed K +-10% | `src/core/pathrecords.bend` |  |
| `path_records_keep_whitespace` | bend | seed K +-10% | `src/core/pathrecords.bend` |  |
| `path_records_unterminated_last` | bend | seed K +-10% | `src/core/pathrecords.bend` |  |
| `path_records_legacy_kept` | bend | seed K +-10% | `src/core/pathrecords.bend` |  |
| `mech_render_success_quiet` | bend | seed K +-10% | `src/core/mech.bend` |  |
| `mech_render_failure_visible` | bend | seed K +-10% | `src/core/mech.bend` |  |
| `mech_render_failure_diagnostic` | bend | seed K +-10% | `src/core/mech.bend` |  |
| `mech_configured_primary` | bend | seed K +-10% | `src/core/mech.bend` |  |
| `mech_primary_missing` | bend | seed K +-10% | `src/core/mech.bend` |  |
| `mech_primary_selection` | bend | seed K +-10% | `src/core/mech.bend` |  |
| `model_cache_missing_rejected` | unported | seed R +-10% | `src/server/modelcache.bend` |  |
| `model_cache_empty_rejected` | unported | seed R +-10% | `src/server/modelcache.bend` |  |
| `model_cache_directory_rejected` | unported | seed R +-10% | `src/server/modelcache.bend` |  |
| `model_cache_file_accepted` | unported | seed R +-10% | `src/server/modelcache.bend` |  |
| `checks_report_missing_erc` | bend | seed K +-10% | `src/core/checks.bend` |  |
| `checks_report_missing_drc` | bend | seed K +-10% | `src/core/checks.bend` |  |
| `checks_execution_keeps_identity` | bend | seed K +-10% | `src/core/checks.bend` |  |
| `owner_mail_local_holder` | bend | seed K +-10% | `src/core/mailowner.bend` |  |
| `owner_mail_remote_holder` | bend | seed K +-10% | `src/core/mailowner.bend` |  |
| `owner_mail_unknown_method_unacknowledged` | unported | seed R +-10% | `src/core/json.bend` |  |
| `owner_mail_copy_quiet` | unported | seed R +-10% | `src/core/hub.bend` |  |
| `message_image_reference_required` | bend | seed K +-10% | `src/core/images.bend` |  |
| `message_image_traversal_denied` | bend | seed K +-10% | `src/core/images.bend` |  |
| `message_image_text_denied` | bend | seed K +-10% | `src/core/images.bend` |  |
| `message_image_entry_bound_url` | bend | seed K +-10% | `src/core/images.bend` |  |
| `composer_bar_one_button` | client | seed C +-10% | `src/web/composer-bar.bend` |  |
| `composer_bar_never_wraps` | client | seed C +-10% | `src/web/composer-bar.bend` |  |
| `allowance_no_measurement_hidden` | bend | seed K +-10% | `src/core/allowance.bend` |  |
| `allowance_last_measurement_visible` | bend | seed K +-10% | `src/core/allowance.bend` |  |
| `provider_fallback_only_when_accounts_exhausted` | bend | seed K +-10% | `src/core/account-fallback.bend` |  |
| `ui_navigation_total` | client | seed C +-10% | `src/core/client.bend` |  |
| `ui_view_error_preserves_all_drafts` | client | seed C +-10% | `src/core/client.bend` |  |
| `ui_state_adoption_preserves_all_drafts` | client | seed C +-10% | `src/core/client.bend` |  |
| `ui_bottom_has_no_unread_pill` | client | seed C +-10% | `src/core/client.bend` |  |
| `ui_seen_boundary_has_no_unread_pill` | unported | seed R +-10% | `src/core/model.bend` |  |
| `json_low_surrogate_replaced` | unported | seed R +-10% | `src/core/json.bend` |  |
| `json_high_surrogate_waits` | unported | seed R +-10% | `src/core/json.bend` |  |
| `json_pair_unit_combines` | unported | seed R +-10% | `src/core/json.bend` |  |
| `json_unpaired_high_preserves_next_character` | unported | seed R +-10% | `src/core/json.bend` |  |
| `json_unpaired_high_preserves_next_escape` | unported | seed R +-10% | `src/core/json.bend` |  |
| `ui_stored_echo_removes_pending_preview` | client | seed C +-10% | `src/core/client.bend` |  |
| `sync_credit_report_duplicate` | unported | seed R +-10% | `src/core/outbox.bend` |  |
| `sync_credit_reports_commute` | unported | seed R +-10% | `src/core/outbox.bend` |  |
| `conversation_preview_encoded_bytes` | unported | seed R +-10% | `src/core/json.bend` |  |
| `conversation_control_chunk_encoded_bytes` | unported | seed R +-10% | `src/server/conversation-frame.bend` |  |
| `conversation_control_split_lossless` | unported | seed R +-10% | `src/server/conversation-frame.bend` |  |
| `conversation_source_window_nonempty` | bend | seed K +-10% | `src/core/thin.bend` |  |
| `conversation_source_window_cardinality` | bend | seed K +-10% | `src/core/thin.bend` |  |
| `conversation_final_envelope_not_refused` | unported | seed R +-10% | `src/core/json.bend` |  |
| `conversation_paging_uses_actual_window` | unported | seed R +-10% | `src/core/json.bend` |  |
| `conversation_floor_retains_source_row` | unported | seed R +-10% | `src/core/json.bend` |  |
| `conversation_floor_visible_text_nonempty` | bend | seed K +-10% | `src/core/thin.bend` |  |
| `conversation_root_reference_envelope_bounded` | unported | seed R +-10% | `src/core/json.bend` |  |
| `conversation_root_region_retained` | unported | seed R +-10% | `src/core/json.bend` |  |
| `conversation_root_wire_picture_constant` | unported | seed R +-10% | `src/core/json.bend` |  |
| `autosettle_owned_work_held` | unported | seed R +-10% | `src/core/model.bend` |  |
| `autosettle_pending_ask_held` | unported | seed R +-10% | `src/core/model.bend` |  |
| `autosettle_unread_held` | unported | seed R +-10% | `src/core/model.bend` |  |
| `autosettle_failed_held` | unported | seed R +-10% | `src/core/model.bend` |  |
| `autosettle_interrupted_held` | unported | seed R +-10% | `src/core/model.bend` |  |
| `autosettle_report_held` | unported | seed R +-10% | `src/core/model.bend` |  |
| `autosettle_grace_held` | unported | seed R +-10% | `src/core/model.bend` |  |
| `autosettle_ready` | unported | seed R +-10% | `src/core/model.bend` |  |
| `autosettle_new_assistant_message_reopens` | unported | seed R +-10% | `src/core/model.bend` |  |
| `autosettle_new_turn_reopens` | unported | seed R +-10% | `src/core/model.bend` |  |
| `conversation_root_preserves_navigation` | unported | seed R +-10% | `src/core/json.bend` |  |
| `home_missing_artifact_has_no_panel` | bend | seed K +-10% | `src/core/project-home.bend` |  |
| `home_owner_round_trip` | bend | seed K +-10% | `src/core/project-home.bend` |  |
| `home_split_requires_saved_documents` | unported | seed R +-10% | `src/core/json.bend` |  |
| `home_3d_uses_saved_board` | bend | seed K +-10% | `src/core/project-home.bend` |  |
| `websocket_orphan_continuation_is_rejected` | bend | seed K +-10% | `src/core/ws-fragments.bend` |  |
| `autosettle_delegation_seed_not_personal` | bend | seed K +-10% | `src/core/autosettle.bend` |  |
| `autosettle_human_followup_is_personal` | bend | seed K +-10% | `src/core/autosettle.bend` |  |
| `context_tool_keeps_text_allowance` | bend | seed K +-10% | `src/core/message-context.bend` |  |
| `context_closed_run_is_one_display_row` | client | seed C +-10% | `src/core/fold.bend` |  |
| `stored_wire_cache_is_empty` | bend | seed K +-10% | `src/core/wire-retention.bend` |  |
| `memory_wire_reference_is_exact` | bend | seed K +-10% | `src/core/wire-retention.bend` |  |
| `autosettle_finished_work_wake_not_running` | bend | seed K +-10% | `src/core/autosettle.bend` |  |
| `autosettle_live_thread_wake_held` | bend | seed K +-10% | `src/core/autosettle.bend` |  |
| `home_viewer_size_keeps_owner` | client | seed C +-10% | `src/core/client.bend` |  |
| `home_viewer_size_is_local` | client | seed C +-10% | `src/core/client.bend` |  |
| `timeline_follow_grown_viewport` | bend | seed K +-10% | `src/core/timeline-virtual.bend` |  |
| `timeline_follow_grown_viewport_away` | bend | seed K +-10% | `src/core/timeline-virtual.bend` |  |
| `json_object_put_fields` | unported | seed R +-10% | `src/core/json.bend` |  |
| `json_object_put_null` | unported | seed R +-10% | `src/core/json.bend` |  |
| `pair_cache_hit` | unported | seed R +-10% | `src/server/pairing-info.bend` |  |
| `pair_cache_miss` | unported | seed R +-10% | `src/server/pairing-info.bend` |  |
| `desktop_connection_snapshot` | unported | seed R +-10% | `src/core/json.bend` |  |
| `adopt_live_entries_kept` | bend | seed K +-10% | `src/core/adopt.bend` |  |
| `mail_credit_recharged` | unported | seed R +-10% | `src/server/wire.bend` |  |
| `mail_json_credit_recharged` | unported | seed R +-10% | `src/server/wire.bend` |  |
| `mail_terminal_credit_recharged` | unported | seed R +-10% | `src/server/wire.bend` |  |
| `mail_wire_text_charge` | unported | seed R +-10% | `src/server/wire.bend` |  |
| `cache_effect_slots` | unported | seed R +-10% | `src/server/cacheio.bend` |  |
| `cache_effect_budget` | unported | seed R +-10% | `src/server/cacheio.bend` |  |
| `cache_effect_global_budget` | unported | seed R +-10% | `src/server/cacheio.bend` |  |
| `static_effect_coding` | unported | seed R +-10% | `src/server/host.bend` |  |
| `static_effect_piece` | unported | seed R +-10% | `src/server/host.bend` |  |
| `static_http_paths` | unported | seed R +-10% | `src/server/static-http.bend` |  |
| `static_http_landing_aliases` | unported | seed R +-10% | `src/server/static-http.bend` |  |
| `static_http_cached_chunks` | unported | seed R +-10% | `src/server/static-http.bend` |  |
| `static_http_fresh_pages` | unported | seed R +-10% | `src/server/static-http.bend` |  |
| `static_http_frame_policy` | unported | seed R +-10% | `src/server/static-http.bend` |  |
| `review_window_budget` | unported | seed R +-10% | `src/server/db.bend` |  |
| `review_window_progress` | unported | seed R +-10% | `src/server/review-window.bend` |  |
| `review_window_read_failure` | unported | seed R +-10% | `src/server/review-window.bend` |  |
| `review_window_cursor_failure` | unported | seed R +-10% | `src/server/review-window.bend` |  |
| `auth_cache_hit` | unported | seed R +-10% | `src/core/model.bend` |  |
| `auth_cache_miss` | unported | seed R +-10% | `src/server/auth-cache.bend` |  |
| `auth_cache_key_sequence` | unported | seed R +-10% | `src/core/model.bend` |  |
| `auth_empty_threads_waits_none` | unported | seed R +-10% | `src/core/model.bend` |  |
| `native_locale_composition` | bend | seed K +-10% | `src/core/client-language.bend` |  |
| `autosettle_live_work_subscription_never_expires` | bend | seed K +-10% | `src/core/autosettle.bend` |  |
| `autosettle_active_file_subscription_never_expires` | bend | seed K +-10% | `src/core/autosettle.bend` |  |
| `catalog_lookup_common_cancel` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `catalog_lookup_parameterized_absent` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `catalog_lookup_unknown_absent` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `catalog_lookup_root` | bend | seed K +-10% | `src/core/msgs-lookup.bend` |  |
| `catalog_metadata_complete` | bend | seed K +-10% | `src/core/i18n.bend` |  |
| `catalog_keys_nonempty` | bend | seed K +-10% | `src/core/msgs-meta.bend` |  |
| `model_catalog_legacy_passthrough` | bend | seed K +-10% | `src/core/accounts.bend` |  |
| `cad_install_visible_action` | bend | seed K +-10% | `src/core/cad-install.bend` |  |
| `cad_install_monotonic` | bend | seed K +-10% | `src/core/cad-install.bend` |  |
| `mcp_subthread_link` | unported | seed R +-10% | `src/core/model.bend` |  |
| `mcp_subthread_parent` | unported | seed R +-10% | `src/core/model.bend` |  |
| `mcp_subthread_manual_move_wins` | unported | seed R +-10% | `src/core/model.bend` |  |
| `mcp_subthread_seed_batch` | unported | seed R +-10% | `src/core/model.bend` |  |
| `fm_zip_bounded_bytes` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_bounded_entries` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_skips_credentials` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_skips_vault` | bend | seed K +-10% | `src/core/fm.bend` |  |
| `fm_zip_credentials_ignore_case` | bend | seed K +-10% | `src/core/fm.bend` |  |
