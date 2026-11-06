# main: merge, build, relaunch

A thread merges its own work into `main`. Every move of `main` is built here, on this machine, and the running app relaunches on the result.

    scripts/install-hooks.sh           once per clone: a hook that starts the builder whenever main moves
    scripts/main-build.sh              the builder: stays running; --once builds the tip now if it is new
    scripts/main-build.sh ensure       start it detached unless it already runs (the hook calls this)
    scripts/main-build.sh status       running or not; main, the tip built last, the tip that failed

## Merging

From your own worktree, once your tests pass: `git rebase main`, then fast-forward `main` from the checkout that has it out (`git -C <that checkout> merge --ff-only <your-branch>`). No lock, no review step. If `main` moved while you rebased, rebase again; never force, never merge-commit.

## The builder

- Follows the `main` ref. Builds in its own checkout (`~/Documents/i2cjak/main-build`, detached at the tip) with one target directory per crate (`~/.cache/main-build`), so nobody's untracked files are in a build and every build is incremental.
- Per new tip: the tests of both crates (minus `scripts/gate-skip.txt`), then builds of `backplane-hub` and `backplane-gpui`, then `dev-cut.sh`. A red test or a failed compile publishes nothing, marks that tip bad and leaves the bar red with the reason until a newer tip lands. So a bad merge never reaches the app, and the next merge fixes it.
- Coalesces, never drops: a merge while a build runs lets that build finish and publish (if it passed), then the newest tip is built.
- Waits (bar red) when under 20 GB is free (`MAIN_MIN_FREE_GB`), after trying `scripts/worktree-gc.sh --apply`. Takes one build slot like every `dev-build.sh` and `scripts/cargo-q` (`scripts/lib-buildslot.sh`), `-j` from free memory, at low priority; compiled dependencies come from `sccache`.
- Runs without `BACKPLANE_AGENT_OWNER` (the hub reaps marked processes).

## The bar

While a build runs, `scripts/build-haiku.sh` has Haiku write a haiku on the commits since the tip built before (`MAIN_PREV`). It ships beside the build as `HAIKU` and the green bar shows it after the text. No `claude`, no commits or a slow answer: no haiku, the build is not held up.

`$GPUI_DEV_DIR/BUILD` is the builder's status (testing, building, failed); `STATUS` is the watcher's (a build waits for turns to end: green, with Restart now and a confirming click). The app shows the watcher's first. See `docs/DEV-RELOAD.md`.
