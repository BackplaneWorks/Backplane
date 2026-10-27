# iOS VC demo — 2026-10-28

The simulator app is built and installed on the two dedicated simulators, paired with the existing Tailnet hub and real saved projects. Final source is `34de3a6f308f16f5a163161bd2c120c92ce4ddb5`. This delivers persistent pairing and reconnect fencing, owner-bound drafts, branded compact navigation and icons, opt-in notifications, auth recovery, native pairing Paste controls, bounded viewer loading with Retry, offline Close and guarded default-source restoration.

## Exact artifact

[Simulator app ZIP](../build/ios-qualification/Backplane-34de3a6f-simulator.zip) · [Qualification receipt](ios-vc-qualification.json) · [Package verification](../build/ios-qualification/final-34de3a6f-package-verification.json)

| Item | SHA-256 |
| --- | --- |
| ZIP, 6,579,274 bytes | `594080ed32675fe8463bfe365cfe4066fbb4f834c275f6dbb7d4ebcd703e38ce` |
| App executable | `1d95ecf87787afb4313270fc918d88967587aa90f6aefe9daae5afd1e2cc6429` |
| Bundled bridge | `d4d3eaa649c060694a53eecbfc01c7f3cac908c778c4495ba4364f12bea53e13` |

Built with Xcode 26.6 (17F113), iOS Simulator 26.5, arm64. Parent independently verified ZIP integrity, all 24 entries under `Backplane.app`, no XCTest bundle, simulator platform, bundle ID `dev.backplane.mobile` and both bundled hashes. This package cannot be installed on a physical iPhone.

## Demonstration

The dedicated iPhone 17 Pro simulator is `D73B6652-765E-4C7C-AB09-1F13522FCEAC`; iPhone 17e is `6AA4F268-08F6-4E9B-BBD2-C9FF58AB5DB1`. Preserve their pairing and app data. The Pro was handed back paired and unparked.

1. Open Backplane normally. In Threads, find **PoE_LoRa_Tube**, expand its Settled shelf and open the board review thread. Loading or “Catching up” labels mean refresh is still in progress.
2. Open Board, Close, then choose 3D. Wait for populated components, orbit and pinch, then Close and reopen. `viewer.ready` establishes base geometry; only `viewer.mesh.ready` plus the populated render establishes mesh delivery.
3. Open Home → Bub → Chat. Open the keyboard, switch to Threads and back, then background/foreground. Preserve existing drafts; do not send demonstration messages into the shared hub.
4. Use recorded client-only disconnect/reconnect evidence for recovery. No shared hub restart is needed. The qualification disconnect stays parked across relaunch until explicit reconnect.

For reinstall, extract `Backplane.app` and run `xcrun simctl install D73B6652-765E-4C7C-AB09-1F13522FCEAC /path/to/Backplane.app`. Launch without pairing, selection or qualification environment variables: `xcrun simctl launch --terminate-running-process D73B6652-765E-4C7C-AB09-1F13522FCEAC dev.backplane.mobile`. Coordinate through `ios-vc-mac`; never erase the paired simulators.

## Final-source results

| Check | Observed result |
| --- | --- |
| SDK, Foundation, preserved-data installation | PASS `697584`; 86 Foundation checks; both simulators |
| Native source and actual emitted bridge | PASS `697470`: 126 source, 105 emitted fixture, 15 captured-history checks; outbox isolation/reconnect |
| Real live protocol | PASS `697569`: separate 470,946-byte mesh payload at 10.052 s; strict plot-revision metadata consumed without changing freshness or loading deadline |
| Decoded mesh, orbit, pinch, Close/reopen | PASS `697779`, 42.211 s; parent visually reviewed populated components, perspective change and magnification |
| Actual Bub/Chat ↔ thread, keyboard, repeated switches, foreground, Back/reopen | PASS `697967`, 77.651 s; final Bub/chat snapshot online and fresh |
| Client-only disconnect, local viewer Close, parked relaunch | PASS `698081`, 38.543 s |
| Explicit reconnect | PASS `698176`, 27.513 s; same board fresh/online, viewer closed, pairing retained |
| Normal launch without environment overrides | Visible-content assertions `698264`; fixed 45 s observation `698725` (45.792 s whole test) showed thread/composer with both loading labels absent, parent visually reviewed |

The follow-up settling test `698475` reported PASS in 1.977 s, but visual review contradicted its AX text query. It is an invalid false positive and is excluded from acceptance; the flawed predicate was not integrated.

The replacement observational test is snapshot `19d654375688fef93665760a5fa745d608dbf85b`, file SHA `9e6111b7f7855cfffdb6fa78c4fbc3c3df43e2154b731e59fb88131c1febe628`. It waits 45 seconds and records a frame; it does not assert settling from missing AX text.

These are whole scenario durations, not individual tap or navigation latency. Viewer captures sometimes say “Catching up”; the passes do not establish continuous freshness. [Final screenshots](../build/ios-qualification/shots/34de3a6f/) contain private real project content.

The final bridge fixes a confirmed protocol failure: the prior bridge rejected the deployed hub's `plotrevision` metadata, triggering reconnects before mesh delivery. The native adapter now accepts only the exact validated ancillary shape and ignores it without granting selection, geometry, freshness or loading-success authority. Unknown and malformed frames still fail closed. No shared server, core or web changes were required. Earlier failures and source-specific results remain in `candidate-composer-receipt.json`; they were not relabeled as passes.

## Real hub and saved design

Hub: `http://desktop.tail9e0cf2.ts.net:3787`. Independently measured running Linux service SHA is `ae735e6da2bb9faa75db0a34ec782b648ed8920ca5aaccdc169cec7c185aaa28`, PID 3859, at `2026-10-27T21:15:36.263376Z`. Final checks report HTTP 200, version `0.12.4-dev.viewer.v7.3fa0591ffd56`. `/hello` commit `c0844250d3c4f5c0289549a40af57b4651049a54` is a base stamp; accepted composed source snapshot is `6f32eac9b1fa341647d45738960eb8c5ec2bd2e7a752dcfb77e0a29f224ec77a`, receipt SHA `62ba49a0eca01a85cdd729bd20f7461375068a789af6bd47bd3877d102e49189`.

The viewer owner performed the supported service/web update during an agreed iOS pause, verified at `2026-10-27T20:54:55.772Z`. Final 34de checks are after that boundary. No iOS/native source or pairing identity was changed by that deployment. Copied evidence: `hub-post-deploy-20261027.json` and `hub-running-executable-current.json`.

Real design project `p1790229698-1087`, thread `t1790229700-1088`: `/home/h/Documents/i2cjak/PoE_LoRa_Tube/hardware/PoE_LoRa_Tube.kicad_pcb`, saved SHA `ce3ac2dd3e0e782d4f8f1266d29bb8c8ea843e588d0ae7d86be5b3d05aab5630`. This is a non-Git design; saved-file SHA supplies its revision provenance. Bub is bot `b1790292962-8017`, conversation `t1790292962-8017`.

## Remaining qualification limits

- Simulator only: physical networking, Metal thermals/performance and camera/QR are unqualified. No Apple Development identity was available; no App Store/TestFlight publication occurred.
- Actual UI secret paste is untested. Native Paste compiled and was present/hittable on 0aff; a nonsecret `simctl pbcopy` probe crashed with signal 11. Secure private pairing and subsequent launches without pairing environment were exercised.
- Final normal-launch frame shows loading labels cleared at the fixed 45 s observation. Exact time-to-ready and continuous freshness were not measured; no startup latency target is claimed.
- Native compact rendering has no claimed original GLB/PBR/solder material parity with the separate web viewer effort.
- Custom/history/parts/layer/sheet restoration is conservative: Close/reselect. Interrupted claimed restore and late receipt after Close have helper/source coverage without a dedicated native runtime case.
- No live message sends or attachment uploads were tested on the shared hub. iPad and comprehensive VoiceOver remain unqualified. Light/dark, small-phone accessibility-size and draft-edit/cleanup evidence is from earlier 946/266 sources, recorded separately.
- `bend PROOF.bend` remains red at the existing `Laws.post_running_delivers` pending/waits mismatch; final precommit run `698709` retained the failure in `final-delivery-proof.log`. No law was weakened or deleted.

Earlier 946 app/video and 0aff/4eb failure candidates are preserved with their own hashes and receipts. Keep all screenshots, recordings and result bundles private. Future chat prefetch/lazy-loading changes belong to the separate chat owner and are not included in this app artifact.
