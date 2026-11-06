# Phone apps

Native iOS (SwiftUI, `mobile/ios`) and Android (Jetpack Compose,
`mobile/android`) clients for a Backplane hub. Both speak data/1 to the hub
(`DataClient`, `DataModels`, `DataViews`) and draw it natively: lists, text
fields, markdown, the board viewer and plots. They make no product decisions.
There is no engine, bridge, web page or launcher in them.

Pairing: scan the code Settings shows (Devices, Add a device), paste the
hub's tailnet link (`http://host:3787/#token=…`), or open
`backplane://pair?url=<that link, URL-encoded>`.

Build: see docs/ios-build.md for iOS and `scripts/ios-*.sh`; Android builds
with `mobile/android/gradlew` and the `scripts/android-*.sh` helpers.

Improvement reports: `mobile/IMPROVEMENTS.md`.
