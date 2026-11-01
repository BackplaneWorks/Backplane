# Local testing

Local builds need no tag, nightly, hosted workflow or release publication. Use
this checkout, pinned Bend, a local Bun, Python and the already installed Electron 44.9.2
runtime. The commands below do not change the installed profile or event log.

```sh
# One coordinated gate after the batch is implemented. Reuses exact-input passes.
python3 scripts/local-ci.py --dry-run
python3 scripts/local-ci.py

# Build a review stage independently of CI/release scheduling.
python3 scripts/local.py build --smoke
cat build/local/LATEST

# Preview the printed stage with a new isolated home; never the live profile.
python3 scripts/local.py preview /absolute/path/to/build/local/STAGE
```

`build --smoke` records service/web, desktop staging, phone bridge, Android compile (when its SDK is present) and UI test
times in `LOCAL.json`. It then qualifies that same server with the shared
wire fixture, full smoke, Mailroom triage, stack, Bun effects, heartbeat and
phone bridge checks. Each runtime command has its own log and timing under
`runtime-evidence/`. A matching exact-input runtime pass reuses those checks;
the new artifact still has to start and serve its complete web assets.
The existing content-keyed server emit and web caches
are shared across worktrees. Shell edits reuse both. An explicit
`--runtime /absolute/runtime` selects an installed Electron runtime; npm installs
only the pinned dependencies, without downloading Electron. The Linux sandbox
stays enabled, reusing an existing root-owned helper where already configured.
The preview starts with a separate home, blocked automation, no tailnet,
telemetry OFF and updates OFF. Closing the window leaves its test hub alive;
the acceptance test cleans up only its own captured child.

To apply a passing stage locally:

```sh
python3 scripts/local.py apply /absolute/path/to/build/local/STAGE
# After the harness reports its safe restart:
python3 scripts/local.py activate /absolute/path/to/build/local/STAGE
# Open Backplane again to load the new shell.
```

Apply requires both UI and runtime acceptance. It copies the reviewed stage into `~/.local/share/backplane-local/STAGE`,
saves the previous service/web/launcher there and asks the hub to deploy at a
safe point. It does not restart systemd directly. Activate requires the exact
expected service version and commit before replacing the desktop launcher.
The normal supervised headless hub, profile, event log and privacy/automation
settings stay in place. No UI close shuts down that hub.
`DEPLOY.json` records preparation/queue time and, when activated, the time until
the exact new greeting was observed. That latter interval is an upper bound
including the safe-point wait and any delay before activation, not an invented
restart duration.

Rollback uses the same safe hub deployment:

```sh
python3 scripts/local.py rollback /absolute/path/to/build/local/STAGE
```

Keep the installed local stage until its successor is accepted; it holds both
the active desktop and rollback. Do not delete it as a disposable worktree build.

The combined CI controller takes one machine-wide lock, admits one phase at a
time through `bp-admit --no-wait`, runs two test workers at most and
records every gate's elapsed seconds and reused input key in
`build/local-ci/TIME/RESULT.json`. An unavailable local reservation immediately
uses Boat under its existing opt-in/budget controls; it never waits indefinitely
or dispatches GitHub CI. A blocked build emits the payload on Boat, verifies its
source/compiler input key and returned hashes, then packages/tests the desktop
locally. Cheap Node guards and the focused Bend check can also split serially
across local/Boat. A completed phase is never duplicated. Initial per-phase
ceilings are explicitly provisional (focused 6, checker 16, proof/build 12,
fast tests 6 GiB; explicit two-worker legacy tests 12 GiB after a measured 10 GiB scope OOM); successful local measurements replace them with the
observed aggregate peak plus 20% rounded to GiB. A serial remote checker can
use its measured process peak plus 64 MiB of helper allowance, then that same
20% margin. Warm cache hits never lower a previously measured cold peak.
The initial checker budget follows the manifest OOM record, not a claim about
build RAM; successful measurements use that same 20% margin. Headroom stays
intact; no application is stopped to make a reservation fit.
For a large check, select serial manifests explicitly with
`CHECK_SHARDS=8 python3 scripts/local-ci.py --only check` (decimal 1–16,
default 1). `CHECK_JOBS` stays 1. The manifest generator and its independent
coverage reread still cover every tracked Bend file, with LAWS checked separately.
The selected layout is part of the local key and remote source/spec/receipt;
Boat must use that same layout. Sharding changes no verdict and is not evidence
that a particular layout will fit memory or pass.

`*.resources.json` records phase wall time and maximum process RSS, plus the
aggregate admission-scope peak for local phases. The stage's `measurements/`
separates Bend emit, stamping/minification, Bun compile/runtime embedding, web
codegen and desktop packaging. A reused emit has no fresh compiler peak. Bun
embeds its runtime; this path has no separate C link. Test timing output names
the expensive files rather than attributing test cost to the build. The
admission wrapper must support `--no-wait`; otherwise code phases use Boat.
Proof keys cover the actual import closure,
compiler/libraries and runner; test keys also include JS/Python drivers. A
changed input, corrupted record, incomplete run or failure cannot create a pass.
The build stage is always recreated and inspected, even when other gates reuse passes.
`--full` ignores passes. CI remains dispatch-only on GitHub; local testing never
dispatches hosted runners. Boat's existing opt-in, budget, start limits and
cleanup policy remain authoritative. No runner registration, allocation or
account limit is changed by this workflow.

For a measured historical comparison, hosted CI runs
[39806371387](https://github.com/i2cjak/Backplane/actions/runs/39806371387)
and [39805054562](https://github.com/i2cjak/Backplane/actions/runs/39805054562)
took 16m59s and 20m08s from creation to completion. Their longest test shards
took 1015s and 1148s; JS-service jobs took 202s and 205s. Those runs overlapped
push and PR triggers. The current dispatch-only policy prevents that duplication.
These are historical hosted measurements, not a claim about the new local run.
The first cold local suite took 1353.880s with two workers; it did not beat
those hosted wall times. After a staging-only fix, exact-input reuse reduced
the type-check/proof work from 249.382s to 0.294s of cache validation. Local
testing used Bun 1.3.10; the existing dev workflow warns that bundled Bun
notices name the release pin, 1.4.3. This is a local test qualification.

The passing combined stage measured 1.321s for cached service/web staging,
0.484s for the desktop, 0.120s for the cached phone bridge, 2.647s for the real
Electron UI acceptance and 29.419s for the full runtime checks (including
2.082s for Mailroom triage). The corrected full suite passed all 261 files in
1357.241s. The X11 check verifies the Backplane icon and launcher identity;
opaque pixels and alpha are exact, with at most one channel unit of rounding
allowed on translucent edges. Native macOS/Windows packaging remains untested.

The shared phone bridge is built or reused by its exact inputs in the same acceptance sequence; Android Kotlin is compiled when an SDK is available. iOS native
builds require a Mac; Android native builds require an SDK. Linux screenshots
at phone width qualify the web layout, not native phone binaries.

The local deployment target is at most 120 seconds end to end: invocation/source change through queue/transfer, changed-target build, staging and safe restart until the exact service is listening and the requested desktop/web is available. Report cold, warm and UI-only paths separately. Full check/proof/test qualification is a separate coordinated workflow; an unfinished or failed required gate is never waived to meet the iteration target. The earlier ~85-second sum of separately measured build and restart is an estimate, not an end-to-end pass.

The required default regression set is now eight files in `scripts/fast-tests.txt`, removing 253 of the previous 261 programs from the routine gate. It uses one compiler and a 120-second execution budget. The previous full run was cancelled at the user's explicit direction; it is not a full-suite pass. `TEST_FULL=1 scripts/test.sh` is the explicit legacy suite. A selected pass never creates a whole-suite verified receipt. Existing laws remain unchanged.

The reduced eight-file gate passed in 69.322s (71.958s including controller overhead), measured with one compiler. Its admission-scope peak was 5.395 GiB; largest-process RSS was 5.399 GiB, both below the 6 GiB cap. This is a selected regression pass, not a new full-suite pass.

Boat returns its unstamped compiler output with the exact compiler/source key and SHA256. The local workflow verifies both before importing it into the existing emit cache. When that cache and the phone bridge are valid, warm staging uses its separately measured peak plus20%, rounded to GiB. The latest pass reached6GiB aggregate while building an uncached web bundle, so its next reservation is8GiB; it keeps the cold-phase peak history. An unchanged verified local phone bridge is reused rather than compiled a second time on Boat.
