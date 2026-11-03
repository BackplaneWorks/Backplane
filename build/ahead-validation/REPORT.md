# Ahead-of-time UI validation

**Result:** useful historical catches, but **overall incomplete (exit 3)**. The prototype has 306 passing literal contracts, 37 unresolved computed bindings and 31 unresolved URL expressions. No hook is installed and no law was weakened.

## Historical bug controls

Each old program was exported from real Git history and compiled separately. These are historical negatives, not invented bug mutants.

| Reported bug | Buggy revision | What caught it |
| --- | --- | --- |
| Dead Copy | `7b0e9583389cece9784dccccf476dda51041cc38` (`0020fbc6e^`) | Explicit local clipboard-gesture contract: red before, green at `0020fbc6e`. Handler-name existence alone misses it; a Copy dispatcher entry already existed. OS clipboard success is outside this check. |
| Dead “6 new” pill | `e492b0072f5bdb05d9979b757fb6da209f3d3b3c` (`f85e2da01^`) | Actual `ThinHost`: older page in flight → newest intent → old reply. Old controller drops the click and misclassifies a restored older window as newest; current passes. |
| Phantom unread | Same `f85e2da01^` | Emitted UI: hidden empty reply incorrectly adds `1 new ↓`. Controller also moves the first unread boundary. Both old properties fail; current passes. |
| “View too large” / blank during turn | `208c90493f82b669125aa31557796238ca46132b` (`1b1c094a0^`) | **Partial catch:** old fitted pending bubble retains 79,999 characters; fixed output is 1,025. Full chrome capacity, server fitting/refusal and web text retention remain outside this simulator. |
| Composer wraps into several rows | `7982813b18c5b929249031be4b644f5d0ac7fa1c` | Yoga + real D.Node/CSS reports 75 px versus 36 px buttons at 320/390/430. Chromium agrees. This captured state is two rows, not a claimed reproduction of the exact human three-row screen. Fixed `92ddc7e46` gives 36 px on phones, 28 px at 1280; both engines agree in all 12 idle/running/long-label cases. |
| Remote images fail | No exact failing historical pair identified | **Miss:** `/img` and relative image URLs already exist in the tested trees. Presence cannot prove authentication, originating-hub ownership, file availability or body delivery. Missing-route/local-file-URL mutants are caught, but are not historical catches. |
| Surrogate hub crash loop | `2a29acbe8e72af2aadb19e86561d0dca3614da53` (`c81ca7506^`) | Production pure `Json.parse` throws on an escaped lone surrogate; current replaces it. No hub was crashed. |

Raw historical results: [history/results.json](history/results.json), SHA256 `fa187a865779e09849cba2a56892606d5d274553a8ab3c836f843c6a36145d2e`. Three serial historical emits took 33.8/36.1/37.1 s; invariant checks took 0.070–0.078 s each.

## Cost, false positives and fidelity

| Approach | Runtime | Limits / false positives |
| --- | --- | --- |
| Static source pass | 0.065 s | Literal actions, syntactically inert indexed handlers, explicit local clipboard ownership and `/img` presence. An early overly broad JS-comparison rule masked a removed handler; narrowing it initially flagged 10 legitimate host bindings, corrected with explicit effect ownership. Computed actions/URLs remain incomplete. Syntax-sensitive recognizers may need review after equivalent refactoring. |
| Deterministic simulator + codec fuzz | **1.65 s** final local run: 483 generated transitions, 104 JSON/CBOR cases, rendered 1,501-entry history and negative controls. Transport 0.006 s. | Drives real `Ui.act`/`recv_act`/`View.page` and `ThinHost`, not a replacement UI model. One generator false positive came from injecting duplicate raw canonical changes into an exactly-once fold. Duplicate/out-of-order view replies now run at the real transport boundary; reconnect uses canonical replay. Shrinking is deletion-based, 1-minimal, not globally shortest. Finite tests are not universal model checking. |
| Layout without browser | About 0.27 s/width including CSS adapter/Node startup; Chromium calibration about 0.5 s/four widths. | 0 px footer-height error in the calibrated cases. Both detect intentional long-label ellipsis, at different node boundaries. A naive browser child-bounds flag marks an ancestor-clipped effort span as overflow: geometric bounds do not certify visible overflow. Grid, pseudo elements, font/line breaking, media and real container discovery remain unsupported/advisory. |
| Private trace replay | 0.28 s; 6 structural events → 7 pure transitions | Capture library and replay fixture work; **live dispatch/export/intake is not installed**, so no live historical catch is claimed. Aggregate diagnostics contain zero replayable UI actions. |

Ten cheap negative-control/export tests pass (~1.15 s). Separate exploratory malformed JSON fuzz found **one throw and two malformed acceptances**. Security/parser owner t1793138895-703494 has the preserved receipt (`parser-receipt.json`, SHA256 `94f7f665c0693791130be9b23e52495bfb3c02a55748f988d734c4bd89e46e91`). Original stdout was not saved; `parser-original.txt` is explicitly a transcription. These failures are not baseline allowances or an all-input parser-totality pass.

## Pre-push and CI

Measured command (run from the caller repository, snapshot has **no `.git`**):

```sh
python3 SNAPSHOT/scripts/ui-validate.py --snapshot SNAPSHOT \
  --base BASE --head HEAD --changed-paths0 PATHS --report REPORT --budget-ms 3000
```

The immutable export is verified against the pushed Git blobs/modes; changed paths include deletions and both rename endpoints. Reports/cache stay outside; no bytecode is written into the export. Cold work never builds or waits on a lock. Reuse binds full dependency/spec/compiler-resource/actual-runtime identity and checks module/result/receipt bytes.

Final hot wall times: **0.266/0.285/0.274 s**, simulator cached/pass but overall **3** for unresolved coverage. Corrupt result/module: 0.263/0.267 s; runtime-options change: 0.263 s; changed imported helper in an actually committed head: 0.275 s; missing compiler: 0.133 s; mode mismatch: 0.256 s; omitted rename endpoint: 0.245 s; 1 ms timeout control: 0.041 s. Every invalid case returned 3. A broken literal contract separately returns 1. See [MEASUREMENTS.json](MEASUREMENTS.json) for source/tool/fixture identities; final input key `de9e03f2021e83445b503d3bf8f9dd21e9deda26db64e1eb6c397111113d9517`.

**Recommendation:** consume known literal contracts as scoped pre-push advisory findings. Keep hook wiring on hold until computed action domains and route ownership are resolved; a simulator-only pass must not become green. In CI, cold-emit the pure simulator, expand seeds, run malformed-input discovery and historical pairs, and calibrate more saved layouts. Retain service/browser checks for remote-image delivery and full fit/refusal behavior. Explicit `prepare.py --module ... --emission-receipt ...` can adopt an identical verified remote emit, then reruns all fixtures locally; the hook never invokes it.

## Trace and law boundaries

The new trace schema is **owned by this prototype**, not inherited from the stopped diagnostics thread. It caps 256 events, 128 session aliases and 64 KiB; exports closed kinds, numeric aliases and bounded relative timings only. No message text, raw IDs, paths, URLs or content hashes. Replay substitutes public synthetic text; scroll/page geometry and recorded delays are not reproduced by the pure fold.

The closed adapter for reviewed diagnostics `d92814f7eef950dfe5faea041b1abbe78480eb8e` excludes private exit rows and arbitrary payloads. A join/span is not inferred to be a reconnect or button press. Live capture/export integration and automatic regression intake remain required.

Alignment with t1793671361-1312951 uses its **uncommitted audit based on `92ddc7e46`**: real navigation, complete-draft-map error/adoption, bottom/seen unread and surrogate-transition laws. Its focused PASS13/PASS360 is not a full-repository receipt. `Cr.drained.k` counter commutativity does not imply arbitrary event commutativity; canonical pending preview does not prove JS optimistic lifecycle; entry-ID counts/order do not prove displayed text retention or byte capacity. Details: [docs/ahead-validation.md](../../docs/ahead-validation.md).

Qualification: local/Boat admission refusals (75) were not counted as passes. On authorized `h@backplane`, copied pinned tools and the exact recorded Bend snapshot passed `check.sh` in **603 s** (146 s type-check, then proof; only the repository-approved unsafe/foreign-effects notice, no proof assertion failure), followed by a 44.71 s emit and passing fixtures. Local adoption verified identical compiler/source bytes and reran tests on actual local tools. [QUALIFICATION.json](QUALIFICATION.json) records the hashes and work ID. Later Python-only refinements passed the 10 local tests and real export/cache controls. This certifies that recorded snapshot, not later unrelated main changes or live-app rollout. CI owner t1793650867-1261555 received the command/results; no placeholder hook or law changes.
