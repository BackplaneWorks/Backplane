# Qualification status

PR #679 merged as `214cfd7fdcdf4090ee870b2d5540d98bc2137537`. Screen
recording merged in #734 (`8e083b45d84967e7dd3c094b8ea4ea994fc0eb71`),
with the Electron permission fix in #760
(`b12e72c243b23b752233519deeb6a5aad546530c`). Source merge is separate
from production compilation and installation, which belong to the installer.
These checks do not change installed hubs, live telemetry settings, credentials
or external issues.

## Rebased candidate

The selected-project report writer was rebased onto main after PR #680. Boat
job `j20261102183942-857627` checked the frozen v2 source and failed overall.
All four focused Bend suites passed, and backend storage tests passed 19/19.
The JavaScript phase passed 38/44 tests. Four startup cases lacked the VM
fixture clock, one identity retry case lacked the Improvements callback, and
the CSP browser test lacked Playwright in the sandbox. Policy and web emits
were not reached. The sandbox was deleted and verified.

The two fixture bindings were corrected without changing their assertions;
those 14 JavaScript cases passed locally. The remaining continuation later
passed as described below. Earlier passes in the historical source campaign
do not qualify this rebased tree.
Global check, global proof and the default suite are omitted from the current
Boat qualification because of its memory limit; they are not reported as pass.
No existing law statement has been weakened or removed.

## Reconstructed v3 continuation

After external changes to the old working tree, the continuation was rebuilt
from committed `a1d90d91b6fadd81065bd2d188afcf8159b3cc44` / tree
`cc66c25e2093e3a0dc89a7151b2ab6c52f0a0200`. A new receipt checked every
tracked byte against Git and preserved the retained controller. The original
four focused Bend and 19 backend storage passes were not replayed.

Boat job `j20261102203026-1020820` passed all 44 JavaScript cases, policy
emission, the desktop/phone public issue popup, and normal web emission with
production transforms. Playwright 1.64.1 and Chromium 156.0.8078.4 were pinned
and verified. Source hashes before/after and returned output hashes matched.
The sandbox was deleted and verified. This qualifies that source, not later
recording changes or a production package. The previous JavaScript failures
above remain part of the record. An earlier launch was refused before sandbox
creation because it incorrectly supplied a $0.50 budget override; the successful
launch used the configured Boat budget without that override.

## Historical source campaign

- The final source campaign ran serially in the shared `bend` queue, inside the
  original admission wrapper's 18 GiB memory scope with swap disabled and a
  4 GiB compiler sizing hint. It returned zero in 531.412 seconds. Its measured
  aggregate peak was 14,357,340,160 bytes.
- The campaign passed server type checking, the current Android
  `compileDebugKotlin` build with the existing Android 35 SDK, all four focused
  Bend test files, and `scripts/check.sh` including the complete proof.
- Focused files cover shared policy/prompt, exact report retrieval, the existing
  internal workflow/account generation, and the real diagnostics effect boundary.
- Fourteen browser and storage regression cases passed in the small admitted
  job before the Android SDK selection error. The public popup test uses the
  emitted production policy and stubbed agent/GitHub transport at desktop and
  phone viewport sizes. It checks evidence requirements, writer-to-editable-body,
  exact decoded issue content, explicit consent, reopen receipts, durable cleared
  choices, quota failures and prevention of duplicate handoffs.
- Storage fault tests exercise rotation/restart, retention, total logical and
  allocated file limits, no measurement-path IO, redaction, partial writes,
  disable/clear failure paths and actual Linux `/dev/full` ENOSPC.
- Android model and native bridge tests passed offline. No app was installed.

The first full campaign failed because the new policy import was below the law
block and this isolated baseline held an older sidebar projection proof. The
import was moved into the import section. The projection fixture now matches
`58c31b70:test/sidebar_input_proof.bend` exactly, including `order.wait`, `watch`
and `bg` map layers. This was a stale-baseline difference, not a new product
regression. No existing law statement was weakened or removed.

## Remaining qualification

Recording-specific focused Bend and policy emission passed in the continuation
below. The installer also completed the combined ACA production build, web
emission, service startup and desktop adoption. These are scoped receipts for
their frozen sources, not a qualification of later main.

The isolated-hub reporting E2E still needs a receipt against a final combined
server artifact and its matching wire output. The earlier source campaign did
not run this integration test. No additional app build or installation is
required merely to repeat the completed recording phases.

The iOS model executable, SwiftUI compilation and device flows are unqualified
on this Linux host. System pickers, native browser sign-in/chooser behavior and
the native-to-web fragment round trip have not been tried on a device. Native
production diagnostic export is unavailable; the native UI does not claim to
attach it. Hub/browser diagnostics can be selected in the web report form.

No real issue submission is part of qualification. The reporter must review
and submit through their own GitHub account.

## Screen recording addition

These results are separate from the reconstructed v3 source above:

- Initial local JavaScript suites passed 26/26 cases for recorder bounds and
  cancellation, permissions and selection, capped private storage and quota
  faults, exact WebM attachments and retries, and existing reporting/preload
  behavior. The later permission fix passed all eight recording unit cases.
- The broad recording JavaScript run passed 57/64. Seven startup fixture cases
  failed because their VM lacked `historyFillMore`; #746 added the binding
  without changing the assertions. All 16 startup cases then passed in Boat
  job `j20261102214216-1684587`. This is not a claim that the entire later
  combined tree passed a fresh 64-case run.
- The recording popup initially failed because its project path was not the
  canonical Backplane project. #755 repaired the fixture. Job
  `j20261102214938-1812289` then passed actual browser MediaRecorder/WebM and
  origin-private storage, playback before attachment consent, exact local
  attachment bytes, public opt-in, and hidden controls on mobile. Its picker,
  agent and GitHub transport were stubbed; it published no issue.
- Actual private X11 capture exposed Electron 44.9.2's legacy `media` request
  with empty `mediaTypes`, which #760 now handles narrowly. Camera/microphone
  request types and media permission checks remain denied. The callback's
  legacy API limitation is documented in README.md.
- Xvfb without a window manager listed its private monitor but not the test
  window. #761 changed the fixture to select that monitor through the real
  production picker and verify the captured pixels. Job
  `j20261102215910-1926442` passed actual Electron 44.9.2 getDisplayMedia,
  MediaRecorder, a 20,535-byte WebM with its expected header, 1152 by 720
  playback and the synthetic green pixels. Fake camera and microphone requests
  both returned NotAllowedError. No person's desktop or real device was used.
  All returned output hashes and source before/after hashes were verified;
  the sandbox was deleted and verified. This qualifies X11 monitor capture,
  not X11 window enumeration or an installed desktop flow.
- Recording compiler job `j20261102213509-1560712` was refused with exit 75
  before compilation: the unchanged guard required 12 GiB available and saw
  5.91 GiB. Focused Bend, policy emission and normal web emission did not run.
  No memory guard was bypassed and no compiler pass is claimed.

Wayland portal handling has unit coverage. A real compositor/portal runtime
and installed desktop capture remain untested. The passing frozen v3 web output
is qualification evidence only and must not be installed as a current-main
artifact.

## Completed recording continuation

The first admitted focused run failed exactly two stale image-only prompt
expectations. PR #770 changed those expectations to "image or recording";
production code and laws were unchanged. Later admission refusals did not run
a compiler and remain part of the evidence.

Boat job `j20261103011016-2975492`, work
`w-t1793635403-1205889-1300284`, passed the corrected
`test/issue_report_test.bend` and policy emission under the unchanged test
admission guard. Its frozen source was
`caf9d6e12e08e12d8308d799c38e6283e86c995a`, tree
`69135c723b87d62879ddd240ce2f7cf6442b8a47`. The receipt records 2,828
source files checked before and after, ten returned artifact hashes verified,
and sandbox deletion verified. Policy SHA-256 is
`dabb842c23bf7b643d95f71f7343226c385a5e92b56a3390e4ca0488ae543210`.
The actual executed Bend hash was
`b2b183bddc3c3fa4bc45d99f6a6bdbb192ebc616e6e697777f49e74c545b1643`;
Bun was `795b37d736a88d746b0e8bb5fcde92bf29bd0f9ac6a1dd1a950fd37440cf5822`.
The runner's provenance Bend hash differs from the executed binary; the receipt
retains both and relies on the command's actual toolchain attestation.

The installer preserved these results in
`RECORDING-QUALIFICATION-ADDENDUM.json`, SHA-256
`ee331d8ed1ce387874cce65197f910ab91cfb4b625f8a4761d4ecf3076b8e14c`.
During takeover, that receipt and all nine linked evidence files were read
back and their hashes verified. It updates the pending recording phases in
the immutable `FINAL-DELIVERY.json`, SHA-256
`10b0abbf296b0151208df0ced0e1da73165307792be2853c57274968476bcb86`.
That delivery used source `aca8c7b03372094ad00cba0e7e080210da1877cf`, tree
`6ef5b7c4dd91761240c34f02602bf715e6a5a3fc`. Its successful app-web evidence
and the prior popup/X11 passes were reused. No web rebuild, popup replay, X11
replay or installation was part of the continuation.
