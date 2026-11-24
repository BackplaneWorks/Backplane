# Release playbook (one page)

This page comes from docs/release-retro-0.16.0.md. Times are relative to the planned tag time, T. A few steps depend on changes still awaiting the person's approval (see the retro's "Approve to implement" list). Those are marked *(pending)*.

## T-48 h: decide the scope

- [ ] **Write down the release bar once**: the platforms, the gate list below and the known-issue policy. Get the person's OK on it. Any change to the bar later goes in writing, with what it costs in time.
- [ ] **The Backplane release doesn't wait for a new KiCad.** It ships on the KiCad pins already on main. A new KiCad (bp.N) ships on its own schedule and reaches installs through `kicad-pins.yml` (one `gh workflow run` per platform, docs/kicad-pins.md), with no Backplane point release needed. Hubs that read the feed (the first release after 0.16.1 and later) take it within 6 h. Bumping the compiled pins (`scripts/kicad-pin.py`) is an ordinary change, made whenever convenient.
- [ ] **Credentials go in the vault now:** the Claude and OpenAI test tokens, plus anything else the gate rows need.
- [ ] **Machines are ready:** ≥100 GB free on this desktop, ≥40 GB on the Mac mini, ≥30 GB on h@backplane and bp-win. Inotify `max_user_instances` is at least 1024 (put it in `sysctl.d`). Boat is signed in. bp-win's auto-shutdown is suspended for the release window, and its idle watchdog held (`scripts/winvm/winvm-hold.sh bp-win 6h`; [winbuild-vm.md](winbuild-vm.md)). *(pending: #10)*

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

## The full suites: the release-candidate pass

A merge runs only the fast gate (`scripts/land-gate.sh`: `cargo check --all-targets` and the unit tests of the crates a PR touches, no e2e, on Boat). Everything else runs here, once per candidate, before the rows above sign it off. Each failure goes to the lead who owns its area, with the test name and the candidate's commit.

| Suite | How | Where |
|---|---|---|
| The hub's Windows tests | `scripts/land-windows.sh` from the candidate's checkout (`ci.yml`'s windows-latest set: `ci-hub-windows-filter.sh`, `--retries 1`; a failure not in `scripts/land-windows-known.txt` blocks) | bp-winbuild (bp-win only while `bp-win-ui` is free) |
| macOS builds and tests | `scripts/mac-build.sh`, `scripts/mac-hub-build.sh` | hs-mac-mini, under `mac-heavy` |
| Linux full gate, e2e suites included | `scripts/dev-build.sh --gate --no-cut` (all crates, e2e and smoke) | a Boat box |
| The screenshot and size matrix | the client's screenshot and size suites (`test/viewers`, the rail scorer) | a Boat box |
| The KiCad embed suites | the `kicad-embed-e2e` rows (Linux; Windows on bp-win under `bp-win-ui`; macOS on the mini) | as each row says |

A nightly run of the same list on our machines, reporting failures to the owning lead, is the next step (not yet scripted).

## Candidates

- **Batch fixes. One candidate per build cycle.** A fix found while a candidate builds goes in the next batch. Only a ship-blocker starts a new cycle early.
- **Push the next candidate onto the same branch** (for example `fix/bp13.1-final2`) to keep the cycle easy to follow. Since bp.13.2, new candidate branches can also restore the fork's compiler caches from main; wait for the previous candidate's promote before dispatching the next one. See [the measured cold/warm results](kicad-ci-cache-measurements.md).
- **Never cancel a build that's within 30 min of finishing.** Let it finish and use it to prove whatever it can.
- **Superseded runs are cancelled only by the fork integrator, by hand, with the guard.** Run `scripts/kicad-candidate-guard.py --new <branch>` (a dry run) when you dispatch the candidate, check what it says, then add `--apply`. It cancels only runs of an older SHA on a candidate branch (`^fix/bp\d+(\.\d+)*-final\d*$`) that are more than 30 min from their workflow's median finish. It never touches release/**, tags, runs of the new SHA, or a run named with `--keep <run id>`. Post its one-line message with the candidate's message.
- When a candidate changes, the release owner sends one message to every lane that has the SHA, the run ids and what changed. Nobody else cancels runs.
- Status reports say only *done*, *running* or *blocked*, with run ids. No ETAs.

## T: tag and publish

- [ ] **KiCad (bp.N) only: the last candidate's cache promote has finished before the tag is pushed.** Tag and release/** runs restore the cache on the fork's `main`, which the "Backplane KiCad cache promote" workflow fills from successful eligible candidate and release-line runs. Confirm each platform's promote succeeded and its cache entry exists. Windows compiler caching remains opt-in through `BACKPLANE_WIN_CCACHE`; with it unset, Windows does not produce a compiler-cache artifact.
- [ ] Main is frozen. The release owner's tag script tags the branch head.
- [ ] Self-hosted runners only if they're proven green on a nightly for this commit. Otherwise delete `RELEASE_*_RUNNER` before tagging (docs/release-runners.md).
- [ ] Watch the tag run. Publish fills the draft. Check the 14 assets and the signed `manifest.json`.
- [ ] iOS goes to TestFlight from the tag on the Mac mini, after the darwin build releases `mac-heavy`.

## After the tag

- [ ] Smoke one update from the previous release on a machine other than the mini.
- [ ] Lift the freeze. Re-enable `nightly.yml` if it was disabled. Unset `RELEASE_*` variables. Restore bp-win's shutdown and end its hold (`scripts/winvm/winvm-hold.sh bp-win off`).
- [ ] Clean up: delete the e2e run folders, drop worktrees that have landed, prune idle `target/` dirs.
- [ ] Within 24 h, record in docs/release-retro-X.Y.0.md the run ids, job times, candidates and cache hits.

## Reference times (0.16.0, GitHub runners)

| Build | Cold | Warm |
|---|---|---|
| Backplane tag run | 58 min (darwin 53, linux-x64 46: cache misses) | est. ~30 min |
| KiCad Windows | GitHub: 2 h 06 min – 2 h 47 min (no compiler cache). bp-winbuild: 53 min 30 s with empty caches (vcpkg 43 min, compile 8 min 36 s) | bp-winbuild: 5 min 53 s, fresh build folder (vcpkg 1 min 52 s, compile 1 min 49 s, ccache 99.96% hits); `scripts/winbuild-kicad.sh <commit>`, see [winbuild-vm.md](winbuild-vm.md) ; GitHub with ccache enabled (bp.13.2): 26.0 min, [details](kicad-ci-cache-measurements.md) |
| KiCad macOS | 76–104 min | 8–20 min |
| KiCad Linux | 83–131 min | 23–29 min |
