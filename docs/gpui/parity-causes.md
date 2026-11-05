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
