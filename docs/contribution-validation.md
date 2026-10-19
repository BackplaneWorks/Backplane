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
| `browser` | Synthetic pages through the real helper and hub; ownership and credential boundaries |
| `phone` | Production bridge, recent-history/replay regressions at the phone's no-JIT stack, live bridge tests |
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

Run `build`, `phone`, and `scripts/build-wire.sh` first;
`native` requires that exact production phone bridge for reconnect/save/load coverage.
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
contains no deployment, signing, release, TestFlight or settings changes. A simulator
build is compile/package evidence; simulator functional and physical-device
acceptance must be reported separately. Mac runtime tests need an actual Mac and
compatible published KiCad assets.

For the October 19 contribution run, upstream's latest observed Actions run
`38623094261` remained queued since October 14. The Actions-policy read returned
HTTP 403; that does not establish a cause. Local and fork runs remain independently
identified and do not create upstream required statuses. Native performance uses
`docs/perf-bench.md` and its original budgets; acceptance lanes do not waive them.
