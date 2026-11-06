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
- Always the newest tip: a merge while a build runs drops that build.
- Waits (bar red) when under 20 GB is free (`MAIN_MIN_FREE_GB`). 28 cores (`JOBS`) at low priority.
- Runs without `BACKPLANE_AGENT_OWNER` (the hub reaps marked processes).

## The bar

`$GPUI_DEV_DIR/BUILD` is the builder's status (testing, building, failed); `STATUS` is the watcher's (a build waits for turns to end: green, with Restart now and a confirming click). The app shows the watcher's first. See `docs/DEV-RELOAD.md`.
