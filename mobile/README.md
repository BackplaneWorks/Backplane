# Phone apps

Native iOS (SwiftUI, `mobile/ios`) and Android (Jetpack Compose,
`mobile/android`) clients for a Backplane hub. Both speak data/1 to the hub
(`DataClient`, `DataModels`, `DataViews`) and draw it natively: lists, text
fields, markdown, the board viewer and plots. They make no product decisions.
There is no engine, bridge, web page or launcher in them.

Pairing: scan the code Settings shows (Devices, Add a device), paste the
hub's tailnet link (`http://host:3787/#token=…`), or open
`backplane://pair?url=<that link, URL-encoded>`.

Build: see docs/ios-build.md for iOS and `scripts/ios-*.sh`. Android builds
with `mobile/android/gradlew`. The `scripts/android-*.sh` helpers drive an
emulator: `android-emu.sh` starts a headless one, `android-fresh.sh` installs
an APK with fresh state paired to a hub, `android-pair.sh` pairs the
emulator's app with the shared hub, `android-measure.sh` takes emulator
timings (not device timing), `android-sweep.sh` runs a thread-count sweep
through the logging proxy, and `android-logo-cats-shots.sh` compares the logo
and cats against the web. `android-ui.py` reads the emulator's UI with
uiautomator. I did not run any of them.

## signing.env

`mobile/ios/signing.env` is read by `mobile/ios/build.sh` (`device` and
`archive`) and, through it, `scripts/ios-archive.sh`. It is never committed.
Keys:

```sh
TEAM_ID=<Apple team id>                # required
BUNDLE_ID=<bundle id>                  # optional, default dev.backplane.mobile
ASC_KEY_ID=<App Store Connect key id>  # archive only
ASC_ISSUER_ID=<issuer id>              # archive only
ASC_KEY_PATH=<path to the .p8 key>     # archive only
```

Detail: docs/ios-build.md.

Improvement reports: `mobile/IMPROVEMENTS.md`.

Push: a phone is alerted only by the uberbot, and only when the person is
needed, work they gave finished, or a reminder or digest they scheduled is
due. iOS gets it through APNs (the app registers its token with
`device.register`), Android on its socket while the app lives. Rules, quiet
hours and the APNs settings: `docs/phone-push.md`.
