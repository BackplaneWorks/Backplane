# gpui-next: merge, build, relaunch

Work lands on `gpui-next`; a builder follows it and the app relaunches on each build. `main` is the guarded branch: promote a reviewed `gpui-next` to it with `land.sh` (`gpui-next` branches from `main`).

    scripts/gpui/next-land.sh TIP        fast-forward gpui-next to TIP (rebase on gpui-next first), under the gpui-next-merge lock
    scripts/gpui/next-build.sh           the builder: stays running; --once builds the tip now if it is new

## The builder

- Builds in its own checkout (`~/Documents/i2cjak/gpui-next-build`, detached at the tip) with one target directory per crate (`~/.cache/gpui-next`), so no worker's untracked files are in a build and every build is incremental.
- Per new tip: the tests of both crates (minus `scripts/gpui/gate-skip.txt`), then release builds of `backplane-hub` and `backplane-gpui`, then `dev-cut.sh`. A red test or a failed compile publishes nothing, marks that tip bad, and leaves the bar red with the reason until a newer tip lands.
- Always the newest tip: a merge while a build runs drops that build.
- 28 cores (`JOBS`), at lower priority.
- Runs without `BACKPLANE_AGENT_OWNER` (the hub reaps marked processes).

## The bar

`$GPUI_DEV_DIR/BUILD` is the builder's status (testing, building, failed), `STATUS` the watcher's (a build waits for turns to end: green, with Restart now and a confirming click). The app shows the watcher's first.
