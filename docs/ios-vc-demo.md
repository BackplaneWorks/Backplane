# iOS simulator qualification notes

This is a private, read-only qualification against an existing hub, using a dedicated simulator. A simulator result is not a device, thermal, physical Metal-performance, signing, or distribution qualification. Keep result bundles and screenshots private: they can contain real project and thread content.

## Reproducing safely

Use an isolated source copy from the exact integrated commit, and verify the emitted bridge hash independently before copying it. Run Mac builds and UI tests under the `ios-vc-mac` named lock. Install only on dedicated simulator devices; preserve their application data. Run XCTest with `-collect-test-diagnostics never -parallel-testing-enabled NO` and retain selected screenshots, logs, result bundles and hashes. The UI target generator changes only the disposable copy.

Pair with the existing hub using a private channel. The simulator-only private pairing helper invokes the normal persisted pairing path; subsequent launches omit the pairing environment. This is not a successful UI paste test. Never put credentials in test arguments, screenshots, recordings, logs or receipts. Use the simulator-only qualification disconnect/reconnect URLs only for this isolated client's sockets. Do not change shared hub state, send messages, create threads, or reset authentication.

## Observations through source 16819ddf

Source commit `16819ddfb3304c580e18130557f8b18aca0cd85b` used bridge SHA-256 `6c595d1e01bdaeaa64f06f0f3126ced89b219fc57b68f02e8f0783e21c97d21e`. Xcode 26.6 and iOS Simulator 26.5 built and installed on dedicated iPhone 17 Pro and 17e simulators. The XCTest-built executable SHA-256 was `9a0e39596bc2c55a3c0dea8281cacc0d6dc217907dfccbe1743ba9c013e938f0`.

* Foundation project-search and viewer-close runners passed 17 and 29 checks respectively.
* On the real saved PoE_LoRa_Tube board, 2D geometry opened, closed, and reopened; the 3D viewer reported decoded mesh readiness. This demonstrates simulator rendering, not physical-device or material-fidelity qualification.
* With that board open, the isolated client was parked. Close was enabled and hittable, dismissed the viewer, and remained closed after a parked relaunch. Explicit reconnect restored fresh content for the same existing board thread, retained pairing, and remained ready on relaunch.
* The strict Bub regression on this source observed the real Bub chat and a fresh presentation. A bounded wait found the composer enabled and hittable, but a subsequent tap did not produce a keyboard while the app was syncing. The full repeated-navigation regression is not passed for this source. An earlier `f1e3abd0` run passed its strict Bub and ordinary-thread navigation and keyboard test; it does not prove this source passes.
* The guarded normal project Return passed: the exact filtered PoE project and nonempty Settled shelf were present with fresh content before a single Return, which selected the existing thread `t1790229700-1088` and reached fresh content. An earlier attempt stopped before Return.

The real hub's `/hello` commit was `b6cc6ed9abfdf15e272ed6dd59225fc4829b1f28`. A separately owned chat deployment changed the hub version during the broader qualification; compare hello/version before and after runs and do not treat that external boundary as the app-only interruption. Existing board file SHA-256 was `ce3ac2dd3e0e782d4f8f1266d29bb8c8ea843e588d0ae7d86be5b3d05aab5630`.

UI paste pairing, current-source small-screen and accessibility coverage, full loader timeout/cancel and physical-device behavior remain open. A read-only signing inventory found no Apple Development identity. The repository-wide Bend proof remains red on the separately recorded existing `post_running_delivers` law failure; mobile-specific checks must not be represented as a full proof pass.
