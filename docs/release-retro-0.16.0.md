# Release retro: Backplane 0.16.0 and KiCad bp.13 (Nov 16–17, 2026)

Backplane v0.16.0 was tagged at 03:06 UTC on Nov 17 and published at 04:04:54 (run 44007804652, 14 assets). It shipped on the **old KiCad pins**: Linux bp.12.4, macOS bp.12.5, Windows bp.10. bp.13 was still a draft at the time of writing (`backplane-kicad-v10.99.0-bp.13`, `isDraft: true`), and bp.13.1 was building (run 44032503579).

**Sources.** The data comes from `gh run` and the job and step APIs on both repos: 100 KiCad fork runs (130 jobs) since Nov 16 00:00, and every 0.16 release run. It also comes from the message timelines of six threads: release t1794855320-614, KiCad lead t1794519048-116, fork t1794446322-2, Carbon t1794379375-289, lane M t1794757955-68 and release check t1794847790-100. Two subthreads filled gaps: release runners t1794882755-2872 and Boat build t1794882505-2824. Other sources are the Actions cache API, `~/.backplane-gpui-dev/main-build.log` and `git log origin/main`.

Two gaps in the sources. Bub's own conversation isn't a thread in this project, so I read only what Bub sent into these threads (the 02:26 speed check). Each thread read is capped at its last 1,000 entries, which covers the whole window here.

Times are UTC. "CP" marks what was on the critical path at that moment.

## 1. Timeline

The person's "we'll continue with the 0.16.0 release, please continue" reached the release owner at **18:55** on Nov 16. The release-check prep thread had started at 16:49. The earlier "hold until we have the libraries" (bp.13 with KiCad's standard libraries) came at 18:10. From that "continue" to publish took **9 h 09 min**.

| UTC | What happened | CP |
|---|---|---|
| 18:15 | bp.13 batch 14 (`17e92c0bd0`) dispatched on all three platforms | **CP: bp.13 Windows build** |
| 18:55 | Release owner starts. 0.16.0 is held for bp.13 with the libraries | bp.13 Windows |
| 19:03–19:05 | The person: "there's no tonight: finish it now", then "do not ship non-working stuff". The Windows OpenGL box becomes a bp.13 blocker again | bp.13 Windows |
| 19:07 | Lead's estimate: cut around 04:00–04:30 if every gate passes first time, with about 2.5 h added for each failed gate | |
| 19:15 | Boat is signed in again (it was signed out until then). `85e09cda3b` dispatched (OpenGL and inotify fix) | |
| 19:17 | Inotify limit raised to 1024 (the desktop was at 125/128 and gates failed at random) | |
| **20:09** | **Batch 14's Windows build fails its own library gate after 115 min (CRLF in the library tables).** No valid Windows build exists, so three runs are cancelled | **CP slips** |
| 20:12 | `a76d1718ae` (CRLF fix) dispatched | bp.13 Windows |
| 20:35 | Backplane release-branch dry run 43935390035 is green on all five platforms and four smokes | (Backplane is ready apart from bp.13) |
| 21:02 | `77d78b8444` (Mac ⌫) dispatched speculatively | |
| 21:34–21:54 | Linux first-Edit stall (>180 s) found and root-caused | |
| 22:15 | `48440e2996` (global tables) dispatched. a76's Windows build is cancelled **at 123 min, about 10–40 min from done** | **CP restarts** |
| 22:45 | The person: "Stop estimating time. Do it now." | |
| 23:02 | Release branch lands on main (`0ece1664e`) | |
| 23:31 | The person: "Just build on our Mac" | |
| **23:33** | **`3a19ebb5bb` (autosave fix) dispatched, and 48440's Windows build is cancelled at 78 min** | **CP restarts** |
| 23:45 | **New Backplane blocker: macOS has no app at all** (no Applications, Dock or TCC identity) | Backplane macOS |
| 00:39 | Dry run 43981635524 fails on macOS. The cause is the new macOS-13 floor check dying silently under `pipefail`, not the archive | |
| 00:40 | `Backplane.app` lands | |
| 00:43 | rc.3 dry run 43989975423 starts (its Windows build takes 81 min) | |
| 01:16–01:50 | **Windows blocker: npm `.cmd` agents get an empty `--mcp-config`**, so agents have no Backplane tools. Fixed in `1ae54bfcb` | |
| 01:20 → 02:09 | Claude test token requested from the person and stored in the vault (49 min). The OpenAI key never arrived, so the Codex row didn't run | |
| 01:21–02:25 | **Four more bp.13 candidates:** `e4084fd786` (Mac menu keys), `fb42d14e89` (Cmd+O), `7fde68dc6f` (Windows click focus), `d2ce4ba5c6` (undo markers) | |
| 02:17 | **rc.3 is green on all five platforms.** Backplane is ready to tag on the old pins | |
| **02:20** | **3a19ebb5bb Windows is green: 2 h 47 min for one Windows KiCad build** | |
| 02:26–02:27 | Bub's speed check cancels the superseded runs. The person: "Can you not use GitHub CI? It takes forever." | |
| 02:29 | Finding: the release signing key exists only as a GitHub secret, so a fully local release isn't possible | |
| 02:37 | The person: "Stop building on Linux over and over." | |
| 02:51 | The person: "Just go, no final release check." Lane M stops all GUI runs on the Mac mini because the person's own windows came in front | |
| 02:56 | **Code freeze on main** (the first one of the release) | |
| 02:57 | Deadline: finish by about 03:30 (22:30 New York time) | |
| 02:58–03:03 | Warm-up build on the mini: two attempts fail in seconds (Homebrew not on the runner's PATH), and the third is cancelled about 4 min in | |
| 03:00 | The person: "Ship now" (bp.13 from 3a19ebb5bb) | |
| **03:05** | **The person: "Do NOT switch the hub, just finish and release."** 0.16.0 drops bp.13 entirely | |
| 03:06 | v0.16.0 tagged at `e06d7baec`, all builds on GitHub | **CP: Backplane tag build** |
| 04:02 | darwin-arm64 finishes, the last of the five (52 min 51 s) | |
| **04:04:54** | **Published** | |
| 04:31 | d2ce4ba5c6's Windows KiCad build is green (126 min), 27 min after the release | |

**The critical path in one line:** bp.13 Windows built three times in a row (failed, cancelled, cancelled), then once more for 2 h 47 min to green. At 03:05 the release stopped waiting for it, and the 58-min Backplane tag build ran.

## 2. Where the time went

The window is 18:55 → 04:04, 9.15 h of wall clock. "CP h" is time on the critical path. "Machine h" is runner time spent, whether or not it was on the critical path.

| Category | CP h | Machine h | Evidence |
|---|---|---|---|
| **Waiting on bp.13, which 0.16.0 didn't ship** | **~8.2** | — | Hold from 18:55 to 03:05. Backplane was green on everything except bp.13 by 02:17 (rc.3). The Backplane-side blockers found during the hold (no macOS app, Windows `.cmd` agents) were real, and they shipped. They were found by testing that ran late, not because of the hold |
| KiCad fork builds, Windows | 4.7 (1.9 failed + 2.8 green) | 18.5 | Successful builds took 130, 160, 167 and 126 min. Since 15:00: 401 min succeeded, **519 min cancelled, 188 min failed**. Breakdown of 43980304604 (166 min): vcpkg 12 min (147 packages from NuGet in 2.7 min; wxWidgets and libwebp rebuilt from source; the local files cache restored **0**), **MSVC compile 2 h 19 min** (2,728 ninja steps at `--parallel 4`, **no compiler cache**), test and package 12 min |
| KiCad fork builds, macOS and Linux | 0 (they finished before Windows) | 25.8 (+10.2 QA) | macOS: 76–104 min cold, **8–20 min with a ccache hit**. Linux: 83–131 min cold, **23–29 min with a ccache hit**. QA: 92–146 min |
| Superseded candidates | **3.4** (20:12 → 23:33: a76 at 123 min and 48440 at 78 min cancelled) | ≈24 cancelled + 4.5 failed | **10 bp.13 candidate SHAs** dispatched from 18:15 on. **52%** of the fork's runner minutes since 15:00 (1,715 of 3,267) were cancelled or failed. Backplane: 6 release runs (rc.1 to rc.6) plus 3 cancelled rc-warm runs |
| Bugs found late (each one a respin) | 2.0 (CRLF) + the respins above | | CRLF tables (20:09), Mac ⌫ (20:53), global tables (22:15), autosave (23:33), **no macOS app (23:45)**, the macOS-13 floor check's own bug (00:39, which cost a 1.5 h dry-run rerun), **Windows `.cmd` MCP (01:16)**, Mac menu keys and Cmd+O (01:21, 01:37), Windows click focus (01:52), undo markers (02:04), bp.13 update not restoring the pane (01:40) |
| Backplane release build | **0.97** (tag run) | ≈17.5 over 6 runs | Tag run: android 4.4, windows 11.3, linux-arm64 31.1, linux-x64 45.7, darwin 52.9, publish 2.2 min. The same jobs varied widely between runs: windows **11–81 min**, darwin **50–96 min**. See §3 |
| Waiting on GitHub runner queues | **~0.1** | 2.3 (fork), 1.1 (Backplane) | Queues weren't the problem. The worst single job queue was 22 min (KiCad macOS, 18:15). Tag-run queues were 0.4–3.4 min. In rc.3 the agent measured queues of 0–4 min |
| Test gates and reruns | small on CP | ≈6 h of desktop time | **162 first-parent commits landed on main** between 18:55 and 03:06, each through `land.sh` (local gate p50 133 s, p90 199 s, 457 runs in the log). Linux fork CI: 14 runs, **9 cancelled**. Local Linux GUI gates kept rerunning until the person stopped them at 02:37 |
| Machine contention | ~0.5 | | Disk full on this desktop at 09:29 (9 GB free, a git checkout failed). h@backplane at 9.5 GB free at 02:28. The Mac mini at 8–20 GB free (too small for a release build plus 3D models). Inotify at 125/128 at 19:07. Boat signed out until 19:15. bp-win's 07:00 shutdown moved by hand twice. bp-win's 4 vCPU were contested at 02:58 (Carbon's local KiCad build against the Backplane runner). The Mac mini GUI was banned from 02:51, and a test hub had rewritten the person's KiCad settings on the mini (found at 23:19) |
| Agent coordination | (hard to separate) | | **About 2,150 messages in six threads in 9 h** (about 4 per minute), with 1,167 relayed between agents. 48 corrections ("correction", "unverified", "turned out"). Carbon learned of a candidate change from the run list rather than a message (23:34). Two parties cancelled the same runs (02:26) |
| Decisions waiting on the person | ~0.8 | | The release bar changed **six times**: hold for libraries (18:10), everything working (19:05), no ETAs (22:45), no final check (02:51), ship bp.13 (03:00), don't switch the hub (03:05). Credentials took 49 min, and one never arrived |

## 3. Build-time deep dive

### Why linux-x64 takes 46 min and darwin 53 min in release.yml

| Job (tag run) | Rust `cargo build` | STEP mesher (OpenCascade) | Notices and packaging | Cache state |
|---|---|---|---|---|
| linux-x64 | 14.5 min | **26.1 min** | 3.8 min | rust-cache: "No cache found"; OCCT: "Cache not found" |
| linux-arm64 | 11.6 min | **15.6 min** | 2.8 min | both missed |
| darwin-arm64 | 25.8 min (gpui finishes at 25 m 44 s) | **21.3 min** | 4.4 min | both missed |
| windows-x64 | 7.6 min | 0.1 min | 1.7 min | rust-cache **full match**, OCCT **hit** |

- **The main cause is cache eviction, not compile speed.** Both repos are over GitHub's 10 GB Actions cache limit: Backplane at **10.88 GB** (31 entries) and the fork at **10.57 GB** (54 entries). Each Rust cache entry is 0.9–1.5 GB per target, and the tag ref writes its own copies (`refs/tags/v0.16.0`) beside main's. That pushes the oldest entries out on every run. main's linux-x64 and darwin Rust and OCCT entries from earlier runs were gone by 03:06, and were written again only by the GitHub-only run 44006756349, which was running at the same time (03:26 to 04:07).
- **OCCT is the largest single item.** It takes 16–26 min per platform when cold (61 min on Windows, per the workflow comment), yet its output is only 25–77 MB. It belongs in a long-lived artifact, not in an LRU cache next to 1.5 GB Rust targets.
- **Rust when cold:** gpui is `lto = "fat"`, `codegen-units = 1`, `opt-level = "s"` (native/gpui/Cargo.toml), and it builds with `--features cef`. hub and voice are `thin` with 4 codegen units. Even with a **warm** cache, Windows takes 7 min 20 s for gpui, because fat LTO with one codegen unit re-links and re-optimises the whole crate graph on every build. On darwin (the macos-latest runner, 3 cores) the cold gpui build takes 25.5 min. I haven't split CEF's wrapper build (CMake and Ninja inside `cef-dll-sys`) from the rest of the Rust build. It's in the same cargo step and isn't logged separately.
- **The fix is cheap.** With a warm cache, linux-x64 would be about 15–18 min and darwin about 28–30 min. That makes darwin the long pole at about 30 min, and the tag run about 32 min instead of 58.

### Why the KiCad Windows build takes 2–3 h

- `backplane-windows.yml` runs on `windows-2022` (4 vCPU), and `scripts/backplane-windows.ps1` builds with Ninja and `--parallel 4`. **There is no compiler launcher**: no sccache, no ccache and no `CMAKE_CXX_COMPILER_LAUNCHER`. So every Windows run compiles all 2,728 steps from scratch, which takes **2 h 19 min**. macOS and Linux use ccache and come down to 8–29 min when the cache hits.
- vcpkg takes 12 min. 147 of 149 packages come from the public NuGet feed (read-only) in 2.7 min. wxWidgets 3.3.4 and libwebp aren't in that feed, so they're built from source every time. The `C:\bp\vcpkg-cache` files source restored 0 packages, so the `actions/cache` entry keyed on `vcpkg.json` wasn't present, which is consistent with the fork's 10.57 GB eviction.
- `cancel-in-progress: true` per ref means that pushing a new candidate onto the same branch silently discards up to 2 h 30 min of work.
- These are always full builds. GitHub runners are ephemeral, so without a cache that persists there's no incremental build.

### Warm local builds

| Machine | Measured | Not measured |
|---|---|---|
| This desktop (32 cores, 59 GB RAM; disk 94% full, 118 GB free) | Backplane `main-build` gate with the fast profile and warm cache: **p50 133 s, p90 199 s** (457 runs). Window build alone: 20–32 s | A release-profile (fat LTO) build. KiCad Linux on this machine |
| Boat (`bx_46rg6925`, 16 vCPU, 32 GB) | d2ce4ba5c6's Linux package built with the job's own steps in `ubuntu:26.04` with a ccache mount. The result is **equivalent to CI's** (48,708 entries, only `sourceCommit` differs). Copying back is slow, about 0.4 MB/s | Wall time of the ninja step (it wasn't recorded in the thread) |
| Mac mini (10 cores, 8–20 GB free) | **The KiCad fork built in 17 min** (incremental, Homebrew, minOS 26, so not publishable) | A Backplane release build: the warm-up was cancelled 4 min in, and two earlier attempts failed on PATH |
| bp-win (4 vCPU, 16 GB) | — | Any release build. The KiCad estimate was 2.5–3.5 h for the first full build at idle priority with `--parallel 2`. bp-win has the same CPU count as GitHub's runner, so it helps only by keeping a persistent cache |

## 4. Recommendations, ranked by hours saved per release

The hours saved are estimates built from the measurements above. Anything marked "est." has not been tried yet.

| # | Change | Saves per release | Cost | Notes |
|---|---|---|---|---|
| 1 | **Split the KiCad release from the Backplane release.** A Backplane release never waits on a new KiCad. A KiCad release ships on its own, and the hub picks up the pin in a point release (the update row at Edit already offers newer KiCads) | **4–9 h** (8.2 h this time) | None (a process rule) | 0.16.0 waited 8 h for bp.13 and then shipped without it |
| 2 | **sccache for the KiCad Windows build**, with the cache on bp-win's persistent disk (a self-hosted runner) or in a dedicated, evict-safe store, plus vcpkg binary caching to a writable feed (GitHub Packages NuGet) so wxWidgets and libwebp stop being rebuilt | **4–6 h**: 2 h 19 min compile → est. 20–40 min per candidate, times 2–3 serial candidates on the CP. vcpkg 12 → about 3 min | 0.5–1 day; 5–10 GB disk | By analogy with the fork's macOS (76–104 → 8–20 min) and Linux (83–131 → 23–29 min) when ccache hits |
| 3 | **Batch fork fixes: one candidate per build cycle.** A fix found during a build goes in the next batch unless it blocks the ship, and a build within 30 min of finishing is never cancelled | **2–3.5 h** (3.4 h of respins this time) | Discipline | 10 candidates, and 52% of fork runner minutes were cancelled or failed. a76's build was cancelled at 123 of about 130–167 min |
| 4 | **Stop Actions cache eviction.** OCCT becomes a prebuilt release asset or a cache that's never evicted. `Swatinem/rust-cache` gets `save-if: github.ref == 'refs/heads/main'`, so tags and branches read main's cache but don't write their own. The same for the fork's ccache keys | **~0.5 h CP** (tag run 58 → est. 30–32 min) and about 2–3 machine-h across dry runs | About 2 h | Both repos are over 10 GB now |
| 5 | **A fixed gate list run once per candidate, on all platforms in parallel, with green platforms locked.** A platform that's green on a candidate isn't rerun unless code for that platform changed | 1–2 h CP, plus many machine-h | Write the list once (§5) | Linux fork CI had 9 of 14 runs cancelled. The person had to say "stop building on Linux" |
| 6 | **Cut a release branch early, with a freeze rule.** Cut it at T-24 h. After the cut, only fixes for blockers land, and only through the release owner | 1–2 h (fewer late bugs and respins) | Discipline | 162 commits landed on main during the window, and the first freeze came at 02:56 |
| 7 | **A clean-install matrix at the cut (T-24 h), not on release night.** | 1–3 h | Already scripted (the release-check thread) | The missing macOS app (23:45) and the Windows npm agents (01:16) were both obvious on a clean install |
| 8 | **Self-hosted runners for builds.** Linux x64 on this desktop in an `ubuntu:22.04` container (glibc 2.35), Windows on bp-win, macOS on the Mac mini. GitHub keeps signing and publishing | est. 0.3–0.5 h CP (tag run → 25–35 min), and Windows KiCad becomes incremental | 1–2 days. The mini needs ≥40 GB free | Routing exists (`8068bc9fd`, `286807b32`, docs/release-runners.md) but has **never completed a run**. Prove it on a nightly first |
| 9 | **Mac mini GUI isolation** (a separate macOS user for tests, or a VM), so tests never touch the person's session | 0.5–1 h, and the GUI rows stop being dropped | 1 day; disk | GUI rows were banned from 02:51. A test hub rewrote the person's KiCad settings (23:19) |
| 10 | **No daily shutdown on bp-win during release windows** (a release-window flag that suspends `az vm auto-shutdown`) | 0.1–0.3 h, plus the risk removed | The VM's hourly rate for the hours it would otherwise be off, for 1–2 days per release | It was moved by hand twice |
| 11 | **Request credentials ahead of time.** Claude and OpenAI test tokens are in the vault before the cut | 0.3–0.8 h | None | 49 min wait, and one key never arrived |
| 12 | **Signing keys in the vault, for a fully local release** | ≤0.05 h (publish takes 2 min on GitHub) | Risk: the update key leaves GitHub's secret store | **Not recommended.** Keep GitHub as the only signer |

**Approve to implement.** None of these is done yet, and each one changes CI or the release process:

1. [ ] Rule: Backplane releases don't wait on a KiCad release. Pin bumps ship in point releases (#1).
2. [ ] Fork `backplane-windows.yml` and `backplane-windows.ps1`: sccache (`CMAKE_C/CXX_COMPILER_LAUNCHER=sccache`, `/Z7`) with a persistent cache, plus a writable vcpkg NuGet feed (#2).
3. [ ] Fork: `cancel-in-progress: false` on Windows, and a written rule for batching candidates (#3).
4. [ ] `release.yml`: OCCT prebuilt as a pinned asset, and `save-if` main-only for rust-cache. The same for the fork's ccache keys (#4).
5. [ ] Adopt docs/release-playbook.md, with its gate list, platform lock, cut and freeze (#5, #6, #7, #11).
6. [ ] Prove the self-hosted runners on a nightly. Free 40 GB on the mini. Linux x64 in a 22.04 container on this desktop (#8).
7. [ ] A separate macOS user (or a VM) on the Mac mini for GUI tests (#9).
8. [ ] A release-window switch for bp-win's auto-shutdown (#10).
9. [ ] Measure first, then decide: gpui with `lto = "thin"` and more codegen units for a release, to compare binary size and start time against fat LTO (a possible 5–15 min on darwin).
