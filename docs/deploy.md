# Safe deploy and restart

A restart must not interrupt a running turn. Batch delivery once per qualified
milestone, then wait for all live turns, tool calls and build locks to clear.
The actual server handoff adds this stronger idle-turn guard to the existing
tool/build-lock contract. Agent adoption remains a recovery mechanism, not
permission to cut a turn.

## Asking

- An agent: the `deploy_restart {stage?, note?}` MCP tool. It answers at once
  ("scheduled") and the agent ends its turn; after the restart it is woken by
  one message, "Backplane restarted for a deploy ...: <from> → <to>."
  `deploy_status` and `deploy_cancel` read and cancel it.
- A client (the person): the requests `deploy.restart {stage?, note?}`,
  `deploy.cancel` and `deploy.status`.
- `scripts/dev.sh install [h@host]`: builds, stages the package and posts
  `deploy_restart` to the running hub's loopback MCP route
  (`POST /mcp/deploy`; "deploy" is no thread, so the person is the requester).
  It prints "deploy queued" and ends. With no hub answering, or with
  `--force`, it installs and restarts now, as before.

`stage` is an absolute folder holding the package (`backplane`,
`backplane-serve`, `web/`, ...). Empty means the files are already in place.

## How it waits

The request is the private setting `deploy.wait`, one JSON line (`by`, `note`,
`stage`, `from`, `since`, `noted`, `locked`, `open`). Each tick, while it waits:

1. The hub's open tool calls, in memory only (core/deploy.bend `Open.*`; the
   server's `Bn.tools`). A call opens where its Act is posted (`ToolOpen`:
   Claude's `tool_use` block, Codex's `item.started`, Grok's/Muse's logged
   call) and shuts at its result (`ToolShut`: the `tool_result` block, Codex's
   `item.completed`, ACP's `tool.done`); a turn's end, an archive, a delete or a
   stopped agent shuts every call of the thread (`ToolsGone`). None of it is in
   the log, and after a restart the list is empty (the old processes are
   gone). A thread between two calls may have none open but is still running, so
   its turn continues to hold the actual server handoff. The requester's own `deploy_*` call is never opened;
   a subagent's Agent/Task call stays open until its result. The wait line
   records what was open at the last look (`open`) for the plane row,
   `deploy_status` and the 30-minute note.
2. With none, the server asks the locks (`flock -n` on each path of
   `BACKPLANE_DEPLOY_LOCKS`, colon-separated, default the two build locks;
   without flock(1) they count as free) and answers `Deploy.check.idle`, using the authoritative current turn state.
3. Clear: `deploy.done` is written (the same line plus the threads cut) and
   `deploy.wait` cleared in one step, then the server moves the staged package
   into the running install's folder as `scripts/dev.sh`'s `install_into` does
   (renames; the old `web/` is kept as `web.old`) and restarts the hub in
   place (the updater's `Upgrade.restart`, same PID). A failed swap clears
   `deploy.done` and tells the requester; nothing restarts.
4. A turn or call that starts during the asynchronous swap is checked again
   at the final `Reexec` boundary. One bounded private request is held until
   normal turn-completion or tick processing observes all idle. No agent is
   stopped, resumed or signalled by this hold.
5. After 30 minutes the requester is told once what still blocks.

The Status plane shows a "Restart pending" row in Running while one waits
("waiting for 2 tool calls and the build lock · Cancel"; a click cancels:
action `deploy-cancel`), and Settings' update line says the same.

## After

At start `Deploy.after` reads `deploy.done`: it clears it and tells the
requester once. A thread the restart cut gets the words appended to its
continue message instead (`Deploy.suffix` in `Carry.k`), so no thread gets two.

Laws `deploy_*`, `tool_*`; tests `test/deploy_test.bend`, `bun test/tools/deploy_e2e.ts`.

## Deploying origin/main

Restarting into a new build is the hub's job, not any one agent's. The
`deploy_main {note?}` MCP tool (and `POST /mcp/deploy` from loopback, which is
the person) asks this hub to build origin/main and restart into it through
the safe path above. Pure logic: `src/core/ship.bend`; steps: hub.bend's
`Ship.*`; IO: server.bend's `ShipJob.*`.

Settings (all off or empty by default):

- `deploy.repo`: the source checkout to build from. Unset, the tool answers
  "not configured" and nothing runs.
- `deploy.wrap`: a command prefix the build runs under, e.g.
  `~/Documents/i2cjak/bp-coord/bin/bp-admit.sh --mem 14 --cores 4 -- systemd-run --user --scope -q -p MemoryMax=16G --`.
  Resource policy lives there, never in the hub. Empty means
  `systemd-run --user --scope -q --` when the user's systemd answers, else
  nothing.
- `deploy.follow`: `on` makes the hub read origin/main on every minute's
  tick (when nothing is under way) and deploy when it moved: for a
  development install that tracks main. Merges coalesce only while a build
  runs: the first tick after it takes the newest main (law
  `ship_follow_every_minute`). Each deploy restarts the hub (about two
  minutes of it not answering while it replays its log). Off, it does no
  work at all.

A main-track install whose desktop app requires an exact release version
(the 0.12 Electron app checks the service reports exactly `0.12.0`) builds
the service alone, stamped with that release, by putting the environment in
`deploy.wrap`, e.g.
`env BACKPLANE_DEV_SERVICE_ONLY=1 BACKPLANE_VERSION_STAMP=0.12.0 ~/Documents/i2cjak/bp-coord/bin/bp-admit.sh --mem 14 --cores 4 --`.
Every build stamps the commit it was made from beside the version
(`BACKPLANE_COMMIT_STAMP`, `scripts/dev.sh` sets it), and `GET /hello`
answers it as `"commit"`, so builds with the same version are told apart.

What happens:

1. The server fetches origin/main in `deploy.repo` (off the hub loop) and asks
   git which of the commits being built, staged (while its deploy still
   waits) and live contain it (`merge-base --is-ancestor`).
2. The tool answers at once: **skipped** when one of them covers it (the answer
   names the commit and whether it is building, staged or live), **queued**
   otherwise. One build runs at a time. Requests that a running build does
   not cover make one follow-up build, of the newest origin/main then, however
   many came.
3. The build is a job of its own under `<home>/deploy/<commit>/`: a temporary
   detached worktree of the repo (`wt`, removed after), `scripts/dev.sh stage
   <dir>/pkg` (the build and stage steps of `dev.sh install`, nothing
   installed and no hub asked), the log `out`, the exit status `code`. It is
   started with `setsid`, inside `deploy.wrap` (or the default scope), so a hub
   OOM or the restart it leads to never cuts it. Its pid and start time are
   recorded (`DeployBuilt` state `pid`). Stale worktrees of ended builds are
   removed at the next launch.
4. The hub looks at the job every 5 s. Exit 0 with a package: staged, and the
   hub asks for the safe restart itself (`deploy.wait`, note
   `deploy_main <commit>`, by the person). A deploy does not wait for a
   deploy_main build: the build runs in its own scope with `BP_BUILD_LOCKED=1`
   (it holds no build lock), so a restart never cuts it, and the next hub
   re-adopts it (step 6). Anything else: failed, with the log's last lines.
5. Everyone who asked since the last report is told once, with the commit,
   the version and the result: a thread the way watched work reaches it (it
   wakes, or the note waits behind its turn); the person with an alert
   (record `alert.deploy`). Skipped requests covered by the build under way
   are told at its end.
6. At start, a build that was under way is watched again: its job outlived the
   hub, and its exit status is read when it ends. It is "lost" (failed) only
   when its process is gone and it left no exit status. A follow-up still
   wanted is asked for again. The restart a staged build asked for makes it
   live.

State: the events `DeployAsked{by, note, commit, cover}` and
`DeployBuilt{commit, state, version, detail}` (states building, pid, staged,
failed, skipped, live), folded into the setting `deploy.ship`.
`deploy_status` (and the `deploy.status` request) answer it under `main`.

Laws `ship_*`; tests `test/ship_test.bend`, `bun test/tools/deploy_main_e2e.ts BINARY`.

Optional CEF runtimes use the same staged deployment. `scripts/dev.sh stage`
only builds/stages a CEF host if this machine already has the optional runtime;
Chrome-only installations do not fetch or install CEF. The pinned CEF libraries
are reused and checked against the distribution used to build the small host.
A staged `browser-runtime/manifest.json` covers the host, helper and all runtime
files by SHA256, with a content-derived identity. The normal swap validates the
payload before moving the app/web files, copies it into an immutable
`browser-runtimes/<identity>/`, and promotes one `browser-current` symlink for
the matched helper and sibling `cef` runtime. Previous bundles remain available
under `browser-previous`, including a snapshot of a pre-manifest installation;
the original runtime cache is untouched. The isolated lifecycle tests are
`uv run --no-project python test/tools/cef_runtime_test.py`. An incompatible
existing directory at an alias path is rejected before activation.

This support must itself be live before a later deployment can activate a new
CEF host through it. An older hub's fixed-file swap cannot install optional
payloads. Bootstrap uses two separate normal hub deployments: install support,
verify that commit live, then deploy the matched helper/host change. Do not
claim the host fix live just because the helper or staging build is newer.
Explicit `BACKPLANE_CEF` overrides still select the user's supplied runtime.
