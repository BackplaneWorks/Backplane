# Building the iOS app

The iOS app's screen is the paired hub's phone web client (docs/phone-web.md)
in a WKWebView; the app keeps what a page can't do (docs/phone-app-shell.md):
pairing, alerts and the Live Activity, the camera permission, the share
sheet, links out, haptics. Its earlier native screens are still there for
one release behind **Classic screens**.

## What changed in the app (for whoever builds it)

- `mobile/ios/Backplane/WebShell.swift` (new): the WKWebView, its
  navigation and UI delegates, downloads, the `window.backplaneApp` bridge
  (handler `backplane`, answers through `window.backplaneApp._receive`), the
  "Can't reach" screen, the re-pair screen, the app's own settings sheet,
  and `AppRoot` (web screen or the native ones).
- `mobile/ios/Backplane/PairingStore.swift` (new): the pairing links in the
  Keychain (`kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`). The first
  launch moves them from UserDefaults (`links`) and removes them there once
  the Keychain holds them.
- `AppModel.swift`: owns the shell (`web`), routes a tapped notification or
  island to the page (`open-thread`), keeps an alert quiet for the thread the
  page shows, and answers the page's hub list.
- `AppSettings.swift`: `classic` (UserDefaults `app.classic`), more haptics.
- `Tabs.swift`: a **Classic screens** toggle in the native Settings, to go
  back to the web screen.
- `BackplaneApp.swift`: the window shows `AppRoot`.
- `mobile/shell/backplane-app.js` is the bridge shim both apps inject. It is
  copied into `mobile/ios/Backplane/backplane-app.js` at build time (git
  ignores the copy, like `bridge.js`).

The project uses Xcode 16's synchronized folders
(`PBXFileSystemSynchronizedRootGroup`): every file in `Backplane/`,
`Shared/` and `Island/` is in its target without a `project.pbxproj` entry,
so the new Swift files and the copied `.js` need no project edit. No new
package, capability, entitlement or Info.plist key.

## Build it (on the Mac)

Needs Xcode 16 or newer (iOS 17 deployment target) and, to make

1. `git pull` in a checkout of the repository.
2. Simulator: `mobile/ios/build.sh sim`. It makes `bridge.js`
   (`scripts/build-mobile.sh --js`), copies it and `backplane-app.js` into
   `mobile/ios/Backplane/`, builds Debug for the Simulator, installs it on the
   booted iPhone (or boots one) and launches it. With a `bridge.js` already
   there (e.g. copied from a Linux build's `mobile/build/assets/bridge.js`),
3. A real iPhone over USB or Wi-Fi: put `mobile/ios/signing.env` beside the
   script (never committed) with at least `TEAM_ID=` (and `BUNDLE_ID=` if
   not `dev.backplane.mobile`), trust the Mac on the phone, then
   `mobile/ios/build.sh device` (`DEVICE=<id>` from
   `xcrun devicectl list devices` when several are connected). Xcode's
   automatic signing makes the development profile
   (`-allowProvisioningUpdates`); the account must be signed in to Xcode.
4. TestFlight: `mobile/ios/build.sh archive` with the App Store Connect key
   in `signing.env` (mobile/README.md, "TestFlight"). From Linux,
   `scripts/build-ios.sh archive` does the same over SSH (it copies the two
   scripts, rsyncs `mobile/ios` and runs `build.sh` there).
5. In Xcode's window instead: run step 2 once (so the two `.js` files are in
   `Backplane/`), open `mobile/ios/Backplane.xcodeproj`, pick the team for
   both targets (Backplane and BackplaneIsland) and Run.

Debug builds started with any `BACKPLANE_*` variable (the Simulator
automation in mobile/README.md: `BACKPLANE_LINK`, `BACKPLANE_ACTS`, ...)
show the native screens it drives; add `BACKPLANE_WEB=1` for the web screen.

If the bundle has no `backplane-app.js` the app still runs, but the page
gets no bridge (no notification taps into the page, no share sheet, no app
settings); the console says so at launch.

## Test on a real iPhone

Pair against a hub on the tailnet with a few threads (stand-in agents are
fine), then:

- [ ] First launch after the update keeps the pairing (it moves to the
      Keychain) and opens the web screen, not the pairing screen.
- [ ] A fresh install pairs natively (scan the QR in Settings → Devices, a
      `backplane://pair` link from the Camera app, a pasted link, a short
      code); the web screen opens on that hub without asking again.
- [ ] Kill the app (swipe it away) and open it: still paired, no flash of
      the page's pairing screen.
- [ ] Notifications: allow them (App settings → Notifications), lock the
      phone, let a turn end: the alert arrives; tapping it opens that thread
      in the page. With the app open on another thread a banner shows; on
      that same thread none does.
- [ ] Live Activity / Dynamic Island while a turn runs; tapping a row opens
      that thread in the page.
- [ ] Icon badge follows the Now tab's need-you count.
- [ ] Back: the page's ‹ and a swipe in from the left edge go back a screen
      (the screen follows the finger); nothing goes back past the hub's page.
- [ ] Text size: Settings → Display → Text Size (or Accessibility → Larger
      Text) bigger and smaller; coming back, the page is zoomed to match
      (85 %-125 %).
- [ ] Keyboard: the composer stays above it; nothing is hidden under the
      notch or the home bar; rotate to landscape and back.
- [ ] Attach a photo (library and Take Photo) and a file (Files) from the
      composer.
- [ ] A link in a message to another site opens in Safari; a link to a
      file on the hub opens in a sheet over the page; a download (Snap view
      in 3D, a report) offers the share sheet (Save to Files).
- [ ] Share from the page (where it offers it) shows the share sheet.
- [ ] Settings → This phone app → App settings opens the app's own sheet:
      Hubs, Classic screens, Notifications, the version.
- [ ] Several hubs: pair a second one (App settings → Hubs); the page
      switches to it; switch back from the page's hub list or Hubs.
- [ ] Turn the hub off (or leave the tailnet) and open the app: "Can't reach
      <hub>" with Try again, the other hubs and Classic screens; turn it on:
      Try again (or waiting 15 s) loads the page.
- [ ] Revoke this phone in Settings → Devices on the computer: the app asks
      for a new invitation; pairing again opens the page.
- [ ] Dark and light: no white flash on open; the status bar is readable
      with the page's theme dark while the phone is light and the other
      way round (Settings → Appearance → Theme); with the theme unset the
      page follows the phone's appearance.
- [ ] Classic screens on and off from App settings and from the native
      Settings; both work and keep the pairing.
