# main: merge, build, relaunch

A thread lands its own work on upstream main (`origin/main`). Every move of `origin/main` is built here, on this machine, and the running app relaunches on the result.

    scripts/install-hooks.sh           once per clone: a hook that starts the builder whenever main moves
    scripts/main-build.sh              the builder: stays running; --once builds the tip now if it is new
    scripts/main-build.sh ensure       start it detached unless it already runs (the hook calls this)
    scripts/main-build.sh status       running or not; main, the tip built last, the tip that failed

## Merging

From your own worktree: `scripts/land.sh`. It fetches, rebases onto `origin/main`, tests, pushes to origin main as a fast-forward, then fast-forwards local `main` (`docs/git.md`, Landing). No lock, no review step. Never force, never merge-commit, never move local `main` by hand.

## The builder

- Follows `origin/main`, never local `main`: a commit that is not on GitHub is never built. It fetches origin every `MAIN_FETCH` s (60), and the main hook starts it when `main` or `origin/main` moves. Builds in its own checkout (`.backplane/worktrees/main-build` in the main checkout, detached at the tip) with one target directory per crate (`~/.cache/main-build`), so nobody's untracked files are in a build and every build is incremental.
- Per new tip: the tests of both crates (minus `scripts/gate-skip.txt`), then builds of `backplane-hub` and `backplane-gpui`, then `dev-cut.sh`. A red test or a failed compile publishes nothing, marks that tip bad and leaves the bar red with the reason until a newer tip lands. So a bad merge never reaches the app, and the next merge fixes it.
- Coalesces, never drops: a merge while a build runs lets that build finish and publish (if it passed), then the newest tip is built.
- Waits (bar red) when under 20 GB is free (`MAIN_MIN_FREE_GB`), after trying `scripts/worktree-gc.sh --apply`. Takes one build slot like every `dev-build.sh` and `scripts/cargo-q` (`scripts/lib-buildslot.sh`), `-j` from free memory, at low priority; compiled dependencies come from `sccache`.
- Runs without `BACKPLANE_AGENT_OWNER` (the hub reaps marked processes).

## Where the time goes, and what is skipped

Every phase logs `time: PHASE Ns` in `~/.backplane-gpui-dev/main-build.log`. A full gate is about: compile tests 10 s, run tests 75 s (the hub's suite; one test, the restart storm, runs 48 s alone and is the tail, and fails when it shares the machine with the other suites, so it stays where it is), builds 15 s (guard, hub, voice and window side by side), smoke start 17-30 s (fixed waits).

A suite is not run again when its inputs are byte-identical to the last run that passed: the key is git's index of its files (the hub's: everything but `native/gpui`, `native/voice`, `mobile/`, `order/` and the top-level `docs/*.md`; the window's: everything but the other crates and the same three; the voice's: `native/voice`), the compiler and `dev-build.sh`. The log says `skip: hub tests: inputs identical (KEY) to the run that passed at COMMIT`. The smoke start is skipped the same way when the hub's and the guard's inputs are those of the last passing smoke start. Keys are kept in `$GPUI_DEV_DIR/.gate-ok/` and written only after a pass. `GATE_FULL=1` runs everything. The hub is always rebuilt (it names its commit).

## The bar

While a build runs, `scripts/build-haiku.sh` has Haiku write a haiku on the commits since the tip built before (`MAIN_PREV`). It ships beside the build as `HAIKU` and the green bar shows it after the text. No `claude`, no commits or a slow answer: no haiku, the build is not held up.

`$GPUI_DEV_DIR/BUILD` is the builder's status (testing, building, failed); `STATUS` is the watcher's (a build waits for turns to end: green, with Restart now and a confirming click). The app shows the watcher's first. See `docs/DEV-RELOAD.md`.
