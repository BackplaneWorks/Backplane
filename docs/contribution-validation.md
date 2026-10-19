# Contribution validation

Run `scripts/validate-contribution.sh LANE` from an assembled contribution
candidate. The entry point serializes proof, emission and native effects with
the machine's native build lock. Each lane saves `build/validation/LANE/result.json`
and full command logs: commit, complete source hash, dirty paths, tool versions,
build flags, host, command status and duration, and artifact SHA-256 values.
Changing source during the lane fails validation. A missing executable, nonzero
command, explicit test failure or skipped required test fails the lane. Evidence
is diagnostic data, not a signature or a required upstream GitHub check.

| Lane | Required work |
|---|---|
| `pure` | `scripts/check.sh` and `scripts/test.sh` |
| `build` | Current-source native window/hub, web and helpers via `scripts/build.sh` |
| `emit` | Current-source server C, web and wire inputs for compilation on a Mac |
| `native` | Synthetic release transactions, platform installer, integrated worktree preservation, event-log failure, reconnect and retirement effects |
| `renderer` | Linux native font/glyph and i18n fixtures, display settling/comparison controls and actual Xvfb pixel readback |
| `browser` | Synthetic pages through the real helper and hub; ownership and credential boundaries |
| `phone` | Production bridge, recent-history/replay regressions at the phone's no-JIT stack, live bridge tests and sanitized current-type Worker state |
| `real-cad` | Explicitly selected KiCad integration using the pinned, verified fixture |

`native`, `browser` and `real-cad` consume `dist/backplane` (override with
`--binary PATH`) and require a passing schema-2 build receipt. The default is
`build/validation/build/result.json`; `--build-receipt PATH` explicitly selects
another receipt. Its commit and complete source hash must match the current
checkout, and the actual consumed binary must match a recorded hub artifact.
A copied binary outside the checkout is accepted only when its bytes match;
its resolved path and hash appear in both `consumed` and `artifacts`. An old,
missing, failed or mismatched receipt fails the lane before tests start.
Changing the binary during validation also fails the lane.

The required Linux `renderer` lane depends on the final native-glyph harness
at `62fa318989b092aa8712825a4c75f1e4f0c6bcbf` or its integrated successor;
this tooling branch alone does not contain that feature. Missing source or tools
fail before execution. It requires all seven settling controls, 29 independent
display comparisons, X11-linked `xpoke` and presentation observer compilation,
and exact red/green RGB readback from an owned Xvfb fixture. It then uses
`scripts/build-app.sh` to compile the actual glyph and i18n fixtures, requires
all seven glyph assertions (including exact original RGlyph bytes and fresh
cold-to-warm pixels), all 705 uniquely indexed samples through the existing
strict parser, and all nine native i18n assertions. Missing assertions, samples,
compiled outputs or a skipped check fail even when a command exits zero.

Install clang, X11 development libraries, Xvfb, `fonts-noto-cjk`,
`fonts-dejavu-core`, `fontconfig` and `libfreetype6`. The lane records the selected
compiler, Bend/Bun/Xvfb/fontconfig executable hashes and an actual Japanese-capable
font's hash/languages. `CC`, `BACKPLANE_X11` and `BACKPLANE_TEST_XVFB` may explicitly
select local dependencies. Fixture processes use a fresh private HOME,
XDG_CACHE_HOME and BACKPLANE_HOME; compiler processes retain their toolchain HOME.
Prebuilt and emit-only shortcuts are disabled for these fixture builds. Each
compiled fixture is hashed before use and checked again at the end; emitted C,
binaries, raw logs and the 705-sample summary are retained under the lane's
evidence directory. Source changes invalidate the receipt as in every lane.

The Linux workflow requires this focused lane. It does not repeat
`scripts/check.sh all` or all unit shards, which remain separate CI gates.
The Mac target is the hub and does not run this Linux window-renderer lane.
These correctness checks and raw microbenchmarks do not establish whole-app
input/display performance budgets or physical-device performance.
The Linux native and Mac-emission jobs use `BACKPLANE_LOW_MEM=1`, two compile
workers, no baked Google credentials and a 4 GiB Bun heap-sizing hint. Before
emission they provision temporary runner swap only when at least 8 GiB of free
disk remains, capped at 16 GiB. The heap hint is not a memory limit; swap and
the recorded build flags are part of the reproducible CI configuration.

Run `build`, `phone`, and `scripts/build-wire.sh` first;
`native` requires that exact production phone bridge for reconnect/save/load coverage.
Its KiCad negative control also requires the historical source object at
`30884966ef59c383581bcd1b7653f8f739cf009e:src/server/kicad.bend`.
The runner checks this before compiling fixtures. For a shallow checkout, run
`git fetch --no-tags --depth=1 origin 30884966ef59c383581bcd1b7653f8f739cf009e`;
the Linux and Mac jobs fetch exactly that commit without importing tags or
unrelated history that could affect build version metadata.
Native and phone evidence records the bridge hash when consumed and checks
that its bytes remain unchanged for the rest of the lane.
The log-reader fixture is built and passed explicitly, so Unicode, large-file and
short-read cases cannot silently disappear behind an omitted optional argument. Browser tests
need Chromium, `BACKPLANE_CHROME` when it is not discovered, and Bun's helper
dependencies. `BACKPLANE_TEST_BROWSERS=chromium` selects the routine browser lane;
additional installed engines can be selected explicitly. The standalone fixture
and protocol tests use generated/synthetic data and stand-in providers.

For Mac compilation, run `scripts/validate-contribution.sh emit` on Linux and
transfer its `build/validation/emit/result.json` together with `dist/web`,
`build/wire` and `build/cc/backplane-serve/all.c`. On the matching checkout run:

```sh
BACKPLANE_PREBUILT=1 scripts/validate-contribution.sh build --prebuilt-receipt build/validation/emit/result.json
scripts/validate-contribution.sh native
```

The build validates the emission's source identity and exact C/web/wire file
hashes before compiling. Missing, modified or extra web/wire files fail this
check. The Mac build then creates its own receipt for the newly compiled hub;
the Linux emit receipt alone cannot authorize a native test binary. The
workflow uses this same sequence. Receipts document reproducible local work;
they do not authenticate an untrusted producer.

The separate `real-cad` lane requires **KiCad 10 or newer** for the pinned
fixture. It probes the selected CLI, records its executable hash/version and
sets `BACKPLANE_KICAD_CLI` so the hub uses those exact bytes. Missing, older,
unknown or failed version results fail before CAD tests run. Installing Ubuntu's
`kicad` package alone does not establish compatibility; if that repository
supplies KiCad 8, the optional workflow fails visibly until a compatible CLI is
provisioned. A local compatible stock or patched executable can be selected with
`BACKPLANE_KICAD_CLI=/absolute/path/to/kicad-cli`. Run
`python3 scripts/fetch-cad-fixture.py` to fetch KiCad's ecc83 demo at the exact
commit and hashes recorded in `test/fixtures/kicad-demo-source.json`. The original
files, source record, attribution and CC BY-SA 4.0 license stay together in
`build/fixtures/ecc83`. `BACKPLANE_CAD_FIXTURE` selects another explicitly supplied
fixture. No test depends on an arbitrary `/usr/share/kicad/demos` installation.
The native IPC real-CAD test separately requires the compatible published
patched runtime; stock CLI acceptance does not prove IPC capability.

`.github/workflows/contribution-validation.yml` runs affected native/browser and
phone lanes, then builds Android and an unsigned iOS simulator app from the same
bridge artifact. It verifies bundled bridge bytes. Existing CI remains responsible
for type checks, proofs, unit shards and Linux architecture builds. The workflow
contains no deployment, signing, release, TestFlight or settings changes.

The phone lane creates its Worker kept-state fixture from the exact production
bridge and three original synthetic events (project, thread and user message),
selects the thread, asserts its message is present, saves and reloads the current
types, and records bridge/state hashes. Returned commands are discarded. It
never reads a copied event store or requires a particular bot. The Mac job
downloads these exact bytes, verifies their hashes and runs the existing
`test/tools/ios-native-helpers.sh BRIDGE STATE`. That compiles actual
`HubConnections`, `WirePriority` and `ScreenCache` helpers, then the exact
production `Worker` prefix from `Engine.swift` with the existing lifetime
harness. Host JavaScriptCore runs with JIT disabled; returned RPC commands are
hashed and never executed. Source/Swift versions, inputs and lifetime logs are
always uploaded from `build/ios-native-helpers`.

After building the unsigned simulator package and checking bridge bytes, the
iOS job enumerates an available iOS runtime and compatible iPhone device type,
creates its own simulator, installs/launches the bundle identifier from its
Info.plist, checks the returned PID after a short wait and captures a PNG.
Cleanup targets only the newly created UUID, including on failure or SIGTERM;
existing simulators are untouched. The synthetic xcrun control suite tests
successful ownership/cleanup, install failure, process exit, termination and
missing runtime without invoking a real simulator locally.

These are separate results: production shared-bridge functional tests, Swift
helper compilation/execution, host-JSC Worker lifetime, simulator package build
and simulator startup smoke. Startup does not establish recent-history UI
acceptance, simulator frame budgets or physical-device behavior. Mac runtime
tests require an actual Mac; compatible published KiCad assets are a separate
dependency.

For the October 19 contribution run, upstream's latest observed Actions run
`38623094261` remained queued since October 14. The Actions-policy read returned
HTTP 403; that does not establish a cause. Local and fork runs remain independently
identified and do not create upstream required statuses. Native performance uses
`docs/perf-bench.md` and its original budgets; acceptance lanes do not waive them.
