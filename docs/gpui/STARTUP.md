# Start-up: checkpoints, not folds

The hub used to read the whole log several times at start (the line index, the read model, the policies' state, the agents' and bots' worlds, the wake book, the command fold, the far feed and the peer table). On the 1.69M-event copy that was about 7 s to `/hello` and about 410 MB idle. Each of those is a deterministic function of the first N lines, so each is now saved with N and a fingerprint of the log at N, and a start folds only the lines past N.

## What is saved (`<home>/ckpt/`)

| file | holds | written by |
|---|---|---|
| `index.bin` | offset and length of every line (`Store`) | `Store::save_index` |
| `model.bin` | the read model: entry indexes, threads, projects, settings (`Model`) | `Model::save` |
| `state.json` | the policies' lean fold (`State`), which the native sidebar draws from | `statehost::refresh_state` |
| `agents.json`, `bots.json`, `commands.json`, `wakes.json`, `feed.json`, `peers.json` | the other folds | each module's `save`, registered with `ckpt::register` |

A checkpoint is used only when its version matches, it covers no more lines than the log has, and the fingerprint of the log at its line count (the count, the first line, the line it ends on; for the index, hashes of the first and last 4 KB it covers) is what it was when saved. Anything else is ignored and the fold runs over the whole log as before. Files are written beside and renamed. Delete `ckpt/` any time; the next start folds everything and the files come back.

## When they are written

`ckptrun::start`: 10 s after start, then every 2 minutes, a fold is written again once it is 5,000 lines (or more) past its file. The policies' state is brought forward from its own file by `fold_line`, so it is exactly what a fold of the log gives, never a copy of the live state. A start that had to fold 20,000 lines or more of state saves it at once.

## What a start does

1. `Store::open`: load `index.bin`, scan only the bytes past it.
2. `Model::open`: load `model.bin`, fold the lines past it.
3. In parallel from the moment the store is open (`ckpt::preload`): the agents' world, the bots' world, the policies' state.
4. The rest registers and the listener binds. Nothing waits on a fold of history.

## Proof and measurement

- `BACKPLANE_CKPT_CHECK=1`: after each checkpoint loads (and its tail is folded), the whole log is folded the old way too and the two are compared; the log says `ckpt-check <name>: equal` or names what differs. Run on a truncated copy so the tail is 391,000 lines: model, policy state, agents world, bots world, command fold and wake book all equal.
- Every start prints `startup: +<ms> <phase> (rss <MB>)` lines.
- Tests: `store::tests::a_saved_index_*`, `model::tests::a_saved_model_loads_and_takes_the_tail`.

## Numbers (1.69M-event copy of the real log, 3.7 GiB cap)

See the report that came with this change; run `scripts/gpui/startup-bench.sh HUB_BIN HOME` for your own.
