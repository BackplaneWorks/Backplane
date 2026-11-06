# Building the iOS app

The iOS app is native SwiftUI and speaks data/1 to the paired hub (build with
`GPUI_NATIVE DATA_APP`). It has no web view, bridge or Classic screens.

The project uses Xcode 16's synchronized folders
(`PBXFileSystemSynchronizedRootGroup`): every file in `Backplane/`,
`Shared/` and `Island/` is in its target without a `project.pbxproj` entry.
Every build uses `SWIFT_ACTIVE_COMPILATION_CONDITIONS` `GPUI_NATIVE DATA_APP`
(set by `mobile/ios/build.sh`, which passes it for all modes).

## Build it (on the Mac)

Needs Xcode (the scripts use `/Applications/Xcode.app`; the minimum version is
unverified, needs a real iOS build). `mobile/ios/build.sh` has three modes:

- `sim`: a Debug build into `build/dd`, installed and launched
  (`dev.backplane.mobile`) on a booted iPhone Simulator, or the first
  available iPhone, which it boots.
- `device`: a Debug build signed for development, installed with
  `xcrun devicectl` on a connected iPhone (`DEVICE=<id>`, else the first).
  Needs `mobile/ios/signing.env` (never committed) with `TEAM_ID` and
  optionally `BUNDLE_ID` (default `dev.backplane.mobile`).
- `archive`: a signed Release archive exported and uploaded to App Store
  Connect (TestFlight). `signing.env` also needs `ASC_KEY_ID`,
  `ASC_ISSUER_ID`, `ASC_KEY_PATH`; `BUILD_NUMBER` defaults to the date and
  minute. It signs by hand with the `backplane-signing` keychain and
  `~/.bp-sign` when present, else lets Xcode sign.

From a Linux checkout, with the Mac `hs-mac-mini` reachable through
`build/gpui/mac-ssh`:

- `scripts/ios-build.sh [clean|release]` rsyncs `mobile/ios` to
  `~/bp-gpui-ios/ios` and runs a simulator xcodebuild (`DATA=1` adds
  `-DDATA_APP`; `ADHOC=1` ad-hoc signs the Debug build so the keychain, which
  holds the pairing, works; `release` is an optimised build with
  `BP_MEASURE`).
- `scripts/ios-archive.sh check|upload`: a no-signing Release device build to
  see that it compiles, or an archive uploaded to TestFlight.

Other `scripts/ios-*.sh` helpers (run, pair, shots, measure) exist; none is
described here.

## Test on a real iPhone

Pair against a hub on the tailnet with a few threads, then check. Everything
below is unverified, needs a real iOS build:

- [ ] A fresh install pairs (scan the QR in Settings → Devices, a
      `backplane://pair` link from the Camera app, or a pasted link).
- [ ] Kill the app and open it: still paired.
- [ ] Notifications arrive when a turn ends, and tapping one opens that thread.
- [ ] Live Activity / Dynamic Island while a turn runs; a tap opens the thread.
- [ ] Keyboard: the composer stays above it; rotate to landscape and back.
- [ ] Attach a photo and a file from the composer.
- [ ] Several hubs: pair a second one and switch between them.
- [ ] Revoke this phone in Settings → Devices on the computer: the app asks
      for a new invitation.
- [ ] Dark and light appearance.
