# iOS loading performance

The verified 34de artifact is preserved. Product work continues with a measured native loading path, hidden-body materialization changes and current-source accessibility checks. This document does not replace the source-specific 34de qualification receipt.

## Native baseline

Work 699159 observed real board/Bub content in all 15 initial screenshots. Normal-launch screenshot completion was 3.94–4.35 s and thread-switch completion 2.53–3.75 s, including XCTest launch/tap/query/screenshot overhead. Freshness sampling occurred after screenshots; those samples are not exact transition timings. No draft was changed or sent. Frames containing an existing Bub draft were kept private rather than exported.

Instrumentation-only app source `2eccfe9e236edbce12f3007fdd180765b6e079e3`, bridge `d4d3eaa649c060694a53eecbfc01c7f3cac908c778c4495ba4364f12bea53e13`, passed SDK build and seven Foundation trace checks. Work 699847 collected 1,004 numeric records in three real-hub sessions, with no pairing or selection override. Same-owner readiness checks passed.

| Native interval | Three-session observation |
| --- | --- |
| Model initialization → cached timeline frame opportunity | 506–567 ms |
| Model initialization → first fresh matching model | 1,285–1,902 ms |
| Model initialization → fresh timeline frame opportunity | 3,200–5,023 ms |
| Foreground → fresh frame opportunity | 79, 563, 79 ms |
| Initial bridge context | median 112 ms |
| Cache read and native screen decode | median 20 ms |
| Capability discovery | median 34 ms |
| Receive bridge invocation | median 1.94 ms, maximum 119.71 ms |
| Receive output decode | median 0.14 ms, maximum 7.28 ms |

These native intervals use one monotonic clock. A display marker follows matching layout and two display-link callbacks; it bounds a frame opportunity, not physical pixel onset. Missing display markers are missing measurements, never zero. External Python timestamps have a different origin and are not subtracted from these records. Request timing, freshness changes, main-actor work and repeated configuration can contribute to the model-to-display gap. Bridge invocation accounts for 159–340 ms and output decoding 9–13 ms within the observed 1.35–3.74 s gaps; those costs do not explain the entire gap.

Detailed private receipts: `build/ios-qualification/performance/baseline-34de.json`, `trace-2ecc-receipt.json`, `trace-2ecc-analysis.json` and numeric log SHA `d09e6705f135dd26ce1089996736de598818669ff4f34f30d421cf4f56b6dbc6`. Measurement semantics are in `test/ios-vc/performance-measurement.md`.

## First product optimization

Source `15b8fc5cccb1dc471347f187ab0b3c07363e1dac` checks a native entry's nested fold role before constructing its hidden body. Fold headers, actions, expanded content, cross-thread fallback and partial-message authority are preserved. The change uses the existing role contract and does not adopt unfinished shared chat source.

Twelve source/emitted equivalence checks pass. In a synthetic emitted-Bend benchmark with 31,800 hidden UTF-8 bytes, warmed legacy samples took 14/11/8/10/13/8 ms; the candidate took 1/0/0/0/0/1 ms at 1 ms clock resolution. Both produced exactly the same 366-byte JSON output. This is evidence of avoided hidden-body work, not a native overall latency claim.

The actual new bundle SHA is `c7aab6184cab1d8b5484fd22325797ff5d7325086292fea80f68961f34bebf4a`. Integrated emission work 700409 passes 105 actual fixture checks, 15 captured-history checks and outbox isolation/reconnect. Native SDK work 700633 passes. Comparison work 700780 stopped before navigation because the restored presentation was Home while the harness expected a board composer; its 337 numeric records remain a separate failed setup, not an A/B result.

Corrected work 701227 selected and confirmed the board in an unmeasured preparation, then passed three matching board/Bub/foreground sessions with no selection or pairing override during measurement. It collected 1,080 numeric records. App executable SHA is `0ce5e2f0d16b4f53d4ec559b1aa06a1ea4dd0a2877096fdec00e509253e7af65`; measured harness commit is `6e7e5169ccccb0b14f442d3c8aaccf1e488202a0`. Strict frames and freshness gates are unchanged.

| Median native interval | 2ecc baseline | 15b8 candidate |
| --- | --- | --- |
| Startup cached frame opportunity (3 each) | 524 ms | 519 ms |
| Startup fresh matching model (3 each) | 1,852 ms | 1,368 ms |
| Startup fresh frame opportunity (3 each) | 3,539 ms | 3,986 ms |
| Navigation fresh matching model (9 each) | 2,285 ms | 1,762 ms |
| Navigation fresh frame opportunity (6 each) | 2,175 ms | 3,366 ms |
| Receive bridge invocation | 1.94 ms | 1.45 ms |
| Receive output decode | 0.14 ms | 0.10 ms |

This small, sequential live-hub comparison does not establish an overall speedup or causal effect. Some frame markers are missing, and owner-free numeric navigation records cannot independently identify the destination. Model readiness improved in these samples while fresh frame observations worsened. The remaining model-to-display interval needs separate investigation; it must not be treated as entirely bridge execution or physical paint latency. Detailed candidate receipt is `build/ios-qualification/performance/trace-15b8-receipt.json`; numeric log SHA is `df163909d189536e43617da3cbd0134ce613d2597d721ecc827011e1904fd5dc`.

Source review identifies two measurement limits: tracing cancels a pending display-link observation on every timeline configuration, and the callback requires freshness at the current accepted screen revision. A first fresh model can become unready again during a later refresh. Existing records cannot separate those transitions from actual layout cost or establish whether cancellation delayed a particular marker. Neither observation justifies relaxing freshness checks.

## Shared loading compatibility

The shared chat owner's unfinished candidate2 remains outside native source. Its SHA-pinned synthetic old-native opt-out fixtures pass the strict source validators and both actual emitted bridges (frozen d4d3 and candidate c7aab), with 16 bridge checks each. Closed folds omit all six tool bodies; explicit expansion preserves partial prefixes; explicit body reads retain incomplete status and carry no presentation-readiness authority. Neither bridge automatically fetches a body. Malformed partial metadata is rejected and stale epochs are ignored. Only outer reply IDs and request revisions are rebased to the generated requests; captured body and authority fields remain intact.

The candidate's `src/core/cbor.bend` is byte-identical to its baseline and deployed406 source, SHA `81463f1d4ad8c0d99cb71f35caa50742e4009f57e196f70075a02a08d139a56c`. This excludes dictionary additions/reordering in that source; fixture replay is still compatibility evidence, not native UI or overall performance acceptance.

## Accessibility and build preparation

On 2ecc, current compact light/dark tab labels and selected traits, plus project-search keyboard clearance, pass. Automated audits report Dynamic Type, clipping and contrast issues. A crop from the same audit recording places four repeated issue frames over Home metric labels; exact element identifiers remain unproven and other contrast issues remain unattributed. The separate 34de small-phone accessibility-medium run passes tabs and keyboard clearance but fails its audit. Its original app and text size were restored. No comprehensive VoiceOver or reduced-motion pass is claimed.

The existing archive script exports to App Store Connect. The local-only unsigned Release validation script performs no archive/export/upload, installation or account changes. Exact 15b8 simulator Release validation passed as work 701693 after confirming a 2 GiB disk reserve. Its executable SHA is `41ef2d08a6e85cd325e558f5452cd2dfccea893828ee56d83fec151c6db1e2b6`, with arm64/x86_64 simulator slices, minimum iOS 17, SDK 26.5 and Xcode 26.6. This is not physical signing or distribution acceptance. Its separate receipt is `build/ios-qualification/performance/release-15b8-unsigned-receipt.json`. Physical-device signing remains unavailable; no identities will be added.

Shared bounded preloading, metadata windows, lazy sidebar branches and server body hydration remain with the existing chat owner until immutable qualification and native opt-out compatibility are ready. Viewer asset/material work remains with the viewer owner. No native material-parity improvement is claimed from web-only changes. Repository proof still fails at the pre-existing `Laws.post_running_delivers` mismatch; no law was weakened.
