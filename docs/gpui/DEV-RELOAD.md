# Dev reload: relaunch on each test build, keeping your place

Run this once, in a terminal, while the app is up:

    scripts/gpui/side-by-side-dev.sh watch      # on the integration branch; plain side-by-side.sh watch elsewhere

It asks your yes, then stays open. Ctrl-C stops following and leaves the app running.

Whoever cuts a build publishes it:

    scripts/gpui/dev-cut.sh DIR                 # DIR: backplane-hub, backplane-gpui, COMMIT, assets/
    scripts/gpui/dev-cut.sh list                # builds kept, * is current
    scripts/gpui/dev-cut.sh back                # go back to the build before

## What happens to a new build

- Same hub binary: only the window swaps, at once, even during a turn.
- New hub: it waits until no turn is running, because a restart cuts a turn and a copy resumes nothing. The old window stays up while the new hub opens the log. Enter in the watch terminal relaunches at once and stops those turns.
- A hub too old to have `/dev/turns` cannot say, so it waits for Enter.
- A build that does not come up is marked bad, what ran before is put back, and the next cut is tried.

It always runs on the same copy: `switch.sh reload` copies nothing and deletes nothing, while `start` makes a fresh copy and loses what was done on the old one. Every check a start makes still holds. On a real store that means your yes and a terminal, so no agent can start it.

## What comes back

- The view the hub last drew: thread or page, panel, viewer, folds (`gpui-place.json` beside the token, 0600, no drafts in it).
- The scroll place per thread; a thread left at its end opens at its end.
- Sidebar open or shut, and drafts (`gpui-drafts.json`, as before).
- The i3 workspace the window was on.

The client saves a moment after each change and on SIGTERM. A scripted run saves nothing unless `BACKPLANE_GPUI_PLACE=1`. A saved thread that is gone opens the list.

Test: `test/gpui/switch/reload_e2e.sh BUILD_DIR` (a private display, a seeded home, stand-in agents).
