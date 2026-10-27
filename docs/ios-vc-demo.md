# iOS product qualification

Latest installed source is `4eb6d74f05083bc56164eb8455561bebe7c9656a`, executable SHA `b6adcf701dfefad7d7d92cd643a0b8658373e4c110062c418ba735bb9898482f`, with the same bridge below. All 86 Foundation checks and SDK/data-preserving installs passed (`696258`/`696463`). Actual viewer timeout became actionable at **30.219 s** across reconnects (`696743`), with Retry and Close visible. **Real 3D mesh recovery still failed after one Retry**. Do not treat this source as a qualified post-update 3D demonstration. A read-only bridge validation probe is investigating the repeated view failures; no cause is assigned yet.

## Preserved packaged baseline

The preserved private arm64 simulator ZIP uses source `0aff625379f727a7705f4cc5de1350a754f80a8d`, built with Xcode 26.6 (17F113) for iOS Simulator 26.5. It includes persistent pairing, reconnect and draft ownership fences, branded navigation, bounded viewer loading with Retry, local offline Close and guarded same-source restoration.

| Artifact | SHA-256 |
| --- | --- |
| `build/ios-qualification/Backplane-0aff6253-simulator.zip` | `c95421cca70c8492d3dcede5fe5413a7d767164bc50b61a52eb0a639313775f7` |
| Bundled executable | `115aad6cbc0fb767c896f0f79068843ff3a7948ebbf1c110afaf33f3f48ee8ee` |
| Bundled bridge | `be82d28cde401b4497a0c7dcb94b9fd809905303b4531e9c9c5cd0eeda86164c` |

Parent verification confirmed ZIP integrity, 6,565,014 bytes, all 24 entries under `Backplane.app`, no XCTest bundle, simulator platform, bundle ID `dev.backplane.mobile`, and both bundled hashes. This is not a signed physical-device app. Source-specific acceptance and retained failures are recorded in `build/ios-qualification/final-0aff6253-receipt.json`.

## Prepared Mac walkthrough

The dedicated iPhone 17 Pro simulator is `D73B6652-765E-4C7C-AB09-1F13522FCEAC`, already paired to `http://desktop.tail9e0cf2.ts.net:3787`. The smaller iPhone 17e is `6AA4F268-08F6-4E9B-BBD2-C9FF58AB5DB1`. Preserve their data.

1. Launch Backplane normally without pairing, selection or qualification environment variables. Loading and “Catching up” indicate refresh; cached content alone does not prove freshness.
2. In Threads, find **PoE_LoRa_Tube**, expand its Settled shelf and open the board review thread. Open Board, Close, then choose 3D and orbit the populated board. Close returns to the conversation.
3. Open Home → Bub → Chat, open the keyboard, then switch between Bub and Threads. Keep any demonstration draft unsent and clear it afterward. Qualification sent no live messages.
4. Use the recorded isolated disconnect/reconnect evidence rather than restarting the hub. Qualification disconnect parks only the app's sockets across relaunch until explicit reconnect. Leave the app unparked.

To reinstall, extract `Backplane.app` and run `xcrun simctl install D73B6652-765E-4C7C-AB09-1F13522FCEAC /path/to/Backplane.app`. Launch from a normal environment with `xcrun simctl launch --terminate-running-process D73B6652-765E-4C7C-AB09-1F13522FCEAC dev.backplane.mobile`. Hold `ios-vc-mac`; do not erase simulators or install onto a physical phone.

## Real data and deployment boundary

Design project `p1790229698-1087`, thread `t1790229700-1088`, uses `/home/h/Documents/i2cjak/PoE_LoRa_Tube/hardware/PoE_LoRa_Tube.kicad_pcb`, saved SHA-256 `ce3ac2dd3e0e782d4f8f1266d29bb8c8ea843e588d0ae7d86be5b3d05aab5630`. No Git design revision is invented for this non-Git project. Bub's conversation is `t1790292962-8017`.

Checks through `692818` used service SHA `cd194ef329c5326b07696dfb40f49fb8d19bed4d7170c6f926288adcf6bfb107`, version `0.12.4-dev.chat.f54c6ed34.6ae0d568`. Its hello commit `b6cc6ed9abfdf15e272ed6dd59225fc4829b1f28` was retained metadata; accepted service source was separately pinned as `6a02112f1dd9ca01b404b9ad1f5b37ade211b0ef`.

During an explicit pause between iOS tests, the viewer owner applied a supported Linux service/web update. Verification at **2026-10-27T20:54:55.772Z** recorded installed/running SHA `ae735e6da2bb9faa75db0a34ec782b648ed8920ca5aaccdc169cec7c185aaa28`, version `0.12.4-dev.viewer.v7.3fa0591ffd56`. Hello commit `c0844250d3c4f5c0289549a40af57b4651049a54` is a base stamp; accepted dirty-source snapshot SHA is `6f32eac9b1fa341647d45738960eb8c5ec2bd2e7a752dcfb77e0a29f224ec77a`, candidate receipt SHA `62ba49a0eca01a85cdd729bd20f7461375068a789af6bd47bd3877d102e49189`. The exact exec instant was unmeasured. No iOS/native source changed. Post-update checks start at `694227`; copied receipt: `hub-post-deploy-20261027.json`.

## Results on packaged baseline `0aff6253`

| Check | Result |
| --- | --- |
| SDK build, both preserved-data installs, Foundation | PASS `690541`; 76 checks: load 9, plot 8, drafts 12, flush 13, resume 34 |
| Missing board geometry → timeout → Retry → real geometry | PASS `690690`, 66.075 s |
| Base without mesh → timeout → Retry → decoded mesh | PASS `690930`, 60.700 s |
| Close/reopen gets a new loading deadline | PASS `690930`, 69.350 s |
| Actual Bub/Chat and thread navigation, keyboard, repeated switches, foreground, Back/reopen | PASS `691443`, 108.644 s; final capture fresh and online |
| Normal decoded mesh and orbit | PASS `692818`, 21.444 s; parent visually reviewed perspective change |
| Client-only disconnect, local Close, parked relaunch stays closed | PASS `692368`, 41.531 s |
| Explicit reconnect, fresh same-thread content, retained pairing, viewer closed | PASS `692547`, 29.618 s |
| Post-update Hubs native Paste presence/hittability | PASS `694227`, 28.576 s; empty field, visible pale control; no clipboard interaction |
| Post-update normal app launch | `694227` exit 0, pairing retained, unparked; not a fresh-content assertion |
| Post-update controlled 3D/zoom | FAIL `694677`, 107.688 s: fresh correct thread, viewer cover/base present, decoded mesh absent for 90 s; pinch not reached |
| Normal UI after launch without environment | `695214` mechanical assertions passed, but screenshot showed a restored viewer covering the thread: visible thread journey inconclusive |

Screenshots in `shots/0aff6253/` include board timeout/recovery and 3D before/after orbit. Some show “Catching up”; these passes do not establish continuous freshness. `viewer.ready` proves geometry only; mesh needs separate `viewer.mesh.ready` evidence.

Earlier `a121` Retry lost the viewer; `c0b3` offered an actionable error but rejected the normal live history bar. Current source fixes that guard. Current-source normal 3D/zoom attempts in `691443` stopped at initial readiness before opening a viewer, with no qualification overlay; their cause remains unresolved. The separate post-update zoom attempt `694227` also stopped before opening a viewer: its exact PID 33919 native launch line at 16:56:13 recorded qualification=false/select-present=false. The separately classified simctl launch plus XCTest activation in `694677` reached fresh correct-thread data, then opened the viewer but never reached decoded mesh readiness. Its original recording frame shows base geometry, “Loading 3D model…” and “Catching up.” The same app PID logged reconnects through epoch 12; this does not prove one uninterrupted 30-second loading deadline. Transport/backend delivery investigation is open. Passing `692818` logged qualification/selection flags present; that does not establish earlier sessions' flags. Retain all failures in `candidate-composer-receipt.json`.

## Earlier evidence and limits

`final-946efaf6-receipt.json` and `shots/946efaf6/` preserve earlier navigation, owner-bound draft/cleanup, light/dark keyboard, small-phone accessibility-size, Settings reachability and zoom results. The **946-source** recording `Backplane-946efaf6-orbit-demo.mp4` is 49.618 s, SHA `78841074bd95097e3cdaa58f9dae333477fd7dfae647b7dff6adfce3e05ed5df`. Keep screenshots, recordings and result bundles private; they contain real project content.

* Actual UI paste is unverified: a nonsecret `simctl pbcopy` probe exited 139/signal 11. No secret paste was attempted. The private helper exercised normal persisted pairing, followed by launches without pairing environment. Camera/QR is not physically qualified.
* No Apple Development identity was available. Physical-device networking, Metal performance/thermals, iPad and comprehensive VoiceOver remain unqualified.
* Native compact rendering is separate from web GLB/PBR/solder work; functional passes do not establish material parity or web visual acceptance.
* Automatic viewer restore conservatively supports matching default sources. Custom/history/parts/layer/sheet changes offer Close/reselect. Claimed-restore interruption and late receipt after Close have source/helper coverage without a dedicated native runtime result.
* Live sends and attachment uploads were not exercised against the shared hub; ownership/queue regressions have source/emitted checks.
* Repository proof remains red at existing `Laws.post_running_delivers` pending/waits mismatch (`final-0aff-proof.log`, `693427`). No law was weakened or deleted.

Repeat tests from an isolated exact source copy and verify bridge bytes under `ios-vc-mac`. The target generator modifies only a disposable copy. Use `-collect-test-diagnostics never -parallel-testing-enabled NO`. Keep credentials out of arguments, logs and captures. Do not reset hub authentication, send test messages or mutate customer designs.
