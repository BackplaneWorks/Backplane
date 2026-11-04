# Deterministic standalone test-cache eligibility: research only

The build-speed coordinator assigned classifier hardening and one cheap
metadata rerun, not caching or skipping. Both inventories read exactly frozen
main `29c5bc13f3deeba992332afd5494806c376ed860`, tree
`fc1d1df12a9fc8c1f751c3701344a24255d4547a`, with the pinned official Bend2.0.36
binary/library/Base read as data. No native Bend test, compiler or proof runs;
no pass record is created and no runner, cache or skip policy changes.

## Counts and exclusions

| Scanner result | Initial provisional model | Hardened model |
|---|---:|---:|
| Inventory | 606 | 606 |
| Potential candidates | 370 | 294 |
| Required roots labelled candidates | 7/9 | 3/9 |
| Metadata classifier controls | none | 81 passed |

The hardened set removes76 former labels and adds none. RECLASSIFICATION.json
is a separate post-run comparison; native RESULT files remain unchanged.
"Potential candidate" is necessary, not sufficient, for deterministic native
execution or resource fit, and never permission to skip.

The scanner follows complete imported module bodies. It allows only Base's
console print/print_err/write native imports, propagates opacity through Base
references, and rejects custom effects, opaque operations, aliases and unknown
imports. Exactly one top-level `def main() -> IO(Unit)` is required after
removing strings/comments. Recognized `.mjs`/`.py` drivers are opaque; any other
matching `test/tools` prefix spelling is conservatively excluded too.

Nonexclusive reason counts (each test counts once per reason code):

| Reason | Tests |
|---|---:|
| Custom effect | 205 |
| Opaque Base operation | 83 |
| Missing supported IO main | 15 |
| Recognized opaque driver | 29 |
| Unrecognized driver spelling | 116 |

The prefix rule deliberately over-excludes. Largest groups: `view_test`19
matching tools, `mail_test`16, `telemetry_test`16, `mobile_loading_test`9,
`accounts_test`7, `review_test`7, `far_test`6 and `maildisposition_test`6.
For example, `view` catches `viewer_*`, and `mail` catches `mailrun` and
`maildisposition`. A matching prefix does not prove the file is an actual
driver. Resolving these ambiguities needs a separate reviewed contract;
this assignment does not relax them to increase the count.

## Required roots and historical timing share

| Required root | Hardened label | Principal reason |
|---|---|---|
| mailcontinuity | potential candidate | — |
| mailrun_model | potential candidate | — |
| r1_copy | potential candidate | — |
| mailboot | excluded | complete server closure: custom effects and Base IO |
| mailtriage | excluded | DB effect, IO.sleep and related tool spelling |
| maildisposition | excluded | related prefixed tool spellings |
| maildisposition_routing | excluded | related prefixed tool spelling |
| mailrun | excluded | mailrun_e2e.ts |
| secret_box | excluded | secret_box_e2e.cjs and secret_box_handler_test.py |

Fast-list count9 and SHA256
`be408fb1f8e8b417ab532f1c99a6ee5105867a396eda3f769a8f9b2f907bff9c`
are derived from the frozen list, not assumed. The separate extended list has
22 roots, SHA256
`002d600341d6ab4226b3ce88eeeb855fe1664bdf369218ac8097542be85f3f71`.

The timing source is **historical scheduling estimates**,
`scripts/test-timings.txt`, SHA256
`2f3807142a2bc689681afd5b787de42e0c7de7793b2f1c8776f33cef1b18b43c`,
from Actions run41444167520/revision8ec64e3d40c9dd1fb7b72a1374a05220f1a46be9.
These are not fresh timings on this source/VM or proof of any cached pass.

| Inventory | Timed / total | Candidate known test-seconds / all known | Share |
|---|---:|---:|---:|
| All tests | 513/606 | 5,830.8 /16,438.1 | 35.47% |
| Required | 8/9 | 35.1 /195.2 | 17.98% |
| Extended required | 22/22 | 137.0 /376.5 | 36.39% |

Only245 of294 potential candidates have known timings. No duration is imputed
for93 untimed tests, including required secret_box. No invalid, duplicated or
stale timing row was found. Percentages cover known test-seconds, not complete
parallel-job wall time. Even the optimistic required bucket leaves160.1 known
seconds outside it; this research alone cannot promise two-minute PR checks.

## Controls and remaining limits

All81 metadata assertions pass: Base allow/deny names; a positive print-only
fixture; literals/comments; each listed external/clock/random/env/args/channel/
process/file operation; both recognized drivers and other spellings; absent,
quoted, duplicate or unsupported-signature main; unsafe, custom effect, named
import, Base alias, unknown/missing dependency, transitive exclusion, unsupported
tool identity and combined exclusion reasons. Every applicable diagnostic is
retained. These fixtures test the scanner, not native compilation or execution.
Audio/window/network exclusions have no separately named fixture; all are
denied by the three-console-import allowlist. The classifier is not a Bend
parser, purity proof, cache trust implementation or gate.

A future cache would also need exact source/driver/verdict/compiler/Bun/platform
identity and stack/memory/time execution-profile keys, native admitted execution
before any pass, post-execution snapshot fences and authentic complete joins.
Resource receipts would retain wall time, RSS, cgroup peak/cap/events and CPU
allocation. Proposed policy is at least one rotating native sample every build
plus a complete nightly from-scratch run; sample timeout/OOM/missing receipt or
a separately preregistered regression budget must fail closed and invalidate
that profile. This is **unimplemented design**. A cached pass alone cannot
observe a new regression in an unsampled test; only complete fresh execution
observes all current resource behavior.

## Exact receipts, resources and cost

V1 `j20261104071418-485478`, work1561543: classifier26.295s, whole38.61s,
46credit-s/$0.00046, actual512MiB/swap0/oneCPU, peak387,555,328B, zero
max/OOM events, source/tool identities unchanged, deletion verified.

V2 `j20261104072250-574869`, work1566331: classifier1.796s, whole13.71s,
22credit-s/$0.00022, actual512MiB/swap0/oneCPU, peak388,624,384B, zero
max/OOM events, source/tool identities unchanged, deletion verified.
Its memoized module diagnostics reduce repeated metadata reads; the changed
classifier timings are not a CI/test speedup claim. Total68credit-s/$0.00068
before any eventual docs merge guard.

Reviewed V2 runner SHA256:
`418df8d0da3d737a9b9a45d735c2ccfbaf74a1bfb1bfd615823133fbe5505867`.
Raw inputs, results, controls, timing shares, manifests and provenance are kept.
Generated scanner `.bend` fixture files are deliberately not added to the
repository's canonical Bend module inventory; the archived runner specifies
their exact construction. No cache implementation, native tests, skip wiring,
product source, laws, proofs, install, build, release or telemetry change.
