# Parity gate: 1621 differences (gpui 0688158f6, replay of parity-current/recA), by cause

Owner of this table: slim. Counts come from build/gpui/results/parity-rust7/replay-diff.txt (positional diff, before N20). Many lines are one
difference repeated: a view reply repeats the same fault in `shown` and in `overlay` (x2), in every later view of the session, and a sorted list
with one extra element shifts every later position. N20 (sets for `hk` and `logInfoKeys`) removes the second kind.

| Cause | Lines | Independent | Owner | What it is |
|---|---:|---:|---|---|
| A logInfoKeys | 208 | 1 | parity-edge (wire/, greeting `log` info) | Rust sends one info key more than JS (`cad.install`, sorted before `freecad`): 29 vs 30 keys; the positional diff counted 25 per client. Every replay shows it. |
| B housekeeping (raw `hk`) | 126 | about 12 | state + core | JS appends at a turn's end `alert.<t>`, `autosettle.{audience,output,quiet,read}.<t>`, `liveness.<t>`, `star.nudge.v1`, `summary.at.<t>`; Rust appends none of them and adds `autosettle.version`, `fallback.input.<t>`, `provider.resume.<t>`, `restart.retry.<t>`, `tok.basis.<t>`. The families exist in state/fam (liveness, star); the hub does not run them yet. |
| C picker.models | 312 | 1 | agents (model catalog) | JS lists 5 Claude models (`Fable 5.1 · claude-fable-5-1`, Opus, Sonnet, Haiku and the default); Rust lists `default` only. Repeated x2 (shown/overlay) in every view of the thread. It is a catalog, not the host's CLI list: my earlier note was wrong. |
| D session scratch keys | 246 | about 20 keys | parity-session | `drafts.@*` and `info.view.*` present or absent (JS "" keys, Rust `@away.<t>`, `@more.<t>`, `@bot.want`, `@cmp.*`, `@mech.at`, `@pill`, `@thread.want`), repeated per view. |
| E gpuid page | 684 | 1 | core (view.page) with slim (window logic) | In gpuid views 12 and 13 JS answers an older page (`earlier` 66, first entry Question 58) and Rust the newest window (`earlier` 114, first entry Question 100). Every entry id, text, stamp, fold value and `window.ids`/`historyUnits`/`messageIds` difference follows from that one choice, x2 for shown/overlay. |
| H raw deltas | 21 | 2 | agents / core (stream pacing) | Rust sends 8 delta frames where JS sends 12, and orders `delta`/`delta_end` differently; the text is the same once joined. |
| Total | 1621 | about 40 | | |

Not in this table any more: the bounded sidebar (landed), badauth junk replies, join commit, reconnect origin (gone from this run).

Order of worth: E (684) and C (312) are one cause each. A is one key. After A, C and E the count falls to about 440, and D and B are the rest.
