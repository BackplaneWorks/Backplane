# Release playbook (one page)

This page comes from docs/release-retro-0.16.0.md. Times are relative to the planned tag time, T. A few steps depend on changes still awaiting the person's approval (see the retro's "Approve to implement" list). Those are marked *(pending)*.

## T-48 h: decide the scope

- [ ] **Write down the release bar once**: the platforms, the gate list below and the known-issue policy. Get the person's OK on it. Any change to the bar later goes in writing, with what it costs in time.
- [ ] **The Backplane release doesn't wait for a new KiCad.** It ships on the KiCad pins already on main. A new KiCad (bp.N) ships on its own schedule, and its pin bump goes in a point release. *(pending: rule #1)*
- [ ] **Credentials go in the vault now:** the Claude and OpenAI test tokens, plus anything else the gate rows need.
- [ ] **Machines are ready:** ≥100 GB free on this desktop, ≥40 GB on the Mac mini, ≥30 GB on h@backplane and bp-win. Inotify `max_user_instances` is at least 1024 (put it in `sysctl.d`). Boat is signed in. bp-win's auto-shutdown is suspended for the release window. *(pending: #10)*

## T-24 h: cut and freeze

- [ ] Cut `release/X.Y` from `origin/main`. **After the cut, only fixes for blockers land, and only through the release owner.** Everything else waits for X.Y.1.
- [ ] Bump the version and draft the release notes on the branch. Create the GitHub draft release.
- [ ] Run the dry run (`release.yml`, tag `vX.Y.0-rc.1`) on all five platforms. Check that every build job's caches hit (rust-cache and OCCT). A miss means it will take 45–55 min again. *(pending: #4)*
- [ ] Run the **clean-install matrix on rc.1** right away, not on release night (see the gate list).

## The gate list: once per candidate, all platforms in parallel

| Row | Linux | Windows | macOS |
|---|---|---|---|
| Install from scratch (curl or installer), app starts, update feed resolves | ✓ | ✓ | ✓ (Backplane.app in Applications) |
| KiCad (pinned runtime) installs through the hub and Edit opens it in the pane | ✓ | ✓ (no-GPU too) | ✓ |
| A real agent turn (Claude and Codex) gets Backplane's tools and makes one `kicad_edit` | ✓ | ✓ (npm `.cmd`) | ✓ |
| FreeCAD installs and renders a STEP | ✓ | ✓ | ✓ |
| An update from the previous release offers X.Y and restarts into it | ✓ | ✓ | ✓ |

- **Green platforms are locked.** Once a platform passes on a candidate, it isn't rerun unless code for that platform changed. Say so in the candidate's message.
- On the Mac mini, GUI rows run only in the isolated test user or VM, never in the person's session. Screenshots are cropped to our windows. *(pending: #9)*

## Candidates

- **Batch fixes.** One candidate per build cycle. A fix found while a candidate builds goes in the next one, unless it blocks the ship.
- **Never cancel a build that's within 30 min of finishing.** Let it finish and use it to prove whatever it can.
- When a candidate changes, the release owner sends one message to every lane that has the SHA, the run ids and what changed. Nobody else cancels runs.
- Status reports say only *done*, *running* or *blocked*, with run ids. No ETAs.

## T: tag and publish

- [ ] Main is frozen. The release owner's tag script tags the branch head.
- [ ] Self-hosted runners only if they're proven green on a nightly for this commit. Otherwise delete `RELEASE_*_RUNNER` before tagging (docs/release-runners.md).
- [ ] Watch the tag run. Publish fills the draft. Check the 14 assets and the signed `manifest.json`.
- [ ] iOS goes to TestFlight from the tag on the Mac mini, after the darwin build releases `mac-heavy`.

## After the tag

- [ ] Smoke one update from the previous release on a machine other than the mini.
- [ ] Lift the freeze. Re-enable `nightly.yml` if it was disabled. Unset `RELEASE_*` variables. Restore bp-win's shutdown.
- [ ] Clean up: delete the e2e run folders, drop worktrees that have landed, prune idle `target/` dirs.
- [ ] Within 24 h, record in docs/release-retro-X.Y.0.md the run ids, job times, candidates and cache hits.

## Reference times (0.16.0, GitHub runners)

| Build | Cold | Warm |
|---|---|---|
| Backplane tag run | 58 min (darwin 53, linux-x64 46: cache misses) | est. ~30 min |
| KiCad Windows | 2 h 06 min – 2 h 47 min (no compiler cache) | not available yet *(pending: #2)* |
| KiCad macOS | 76–104 min | 8–20 min |
| KiCad Linux | 83–131 min | 23–29 min |
