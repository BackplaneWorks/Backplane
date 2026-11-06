# The phone apps as a shell around the web client

The Android and iOS apps show the paired hub's phone web client
(docs/phone-web.md) in a WebView (Android WebView, iOS WKWebView) as their
main screen. The page does everything a page can; the app keeps what a page
can't: the pairing (kept in the app's own store), the sockets that raise
notifications and the live status while the app is away, the camera and
file pickers, the share sheet, links out, haptics and the system Back.
The old native screens stay one release behind **Classic screens** in the
app's settings.

## What the app does

- **Pairing** stays native: the app's pairing screen (scan, paste a link,
  type a short code) and `backplane://pair?url=…` links exchange the
  invitation for the device token as today (`/auth/enroll`, native client).
  The app then loads `<hub origin>/?phone=1#token=<token>` the first time
  (the page trades the token for its HttpOnly cookie, `/auth/pair`) and
  `<hub origin>/?phone=1` after that. Android flushes `CookieManager` when
  that page has loaded and on every pause, so a kill right after pairing
  keeps it; iOS keeps the default `WKWebsiteDataStore`. A page whose token
  is refused says so (`pairingFailed`) and the app shows its re-pair screen.
- **Several hubs**: the app keeps every paired hub (its hub list) and
  shows one hub's page at a time (the one opened last). `hubs()`,
  `switchHub(key)` and `addHub()` reach the list from the page.
- **Notifications and the live status** come from the app's own client
  and sockets as before, plus the page: the hub tells the desk used last
  (here, the page) of a finished or waiting thread, and the page hands that
  to the app (`notify`). (On current hubs the app's own client runs in
  thin mode, which raises the live status but no turn alerts; the page's
  are what alert on Android. iOS also has the hub's APNs push, shown in
  front too unless the same alert just showed.) The rest
  as before (src/mobile/notify: a turn that ends,
  an ask, a reminder; the ongoing "agents are working" notification / Live
  Activity). A tapped alert brings the app up on that hub and sends the
  page `open-thread`. The page tells the app which thread it shows
  (`setState`), so that thread raises no alert while it is on screen.
- **Back**: Android's key calls `window.backplaneBack()` (phone-shell.js:
  true when the page shut something or went back a screen); false leaves
  the app. iOS leaves Back to the page: its ‹ and its own swipe in from the
  left edge (the screen follows the finger); WebKit's back gesture is off,
  so the two never race.
- **Files**: a page's `<input type=file>` gets the system picker, and the
  camera when the input asks for `capture`. Downloads of hub files go to the
  system's downloads with the page's cookie.
- **Links**: the hub's own origin stays in the WebView; any other address,
  `target=_blank` and `window.open` go to the system browser (iOS: a new
  window on the hub's own origin opens in a sheet with the page's cookie,
  and a link to another paired hub's page switches to that hub).
- **Text size**: Android passes the system font scale as the WebView's text
  zoom; iOS zooms the page with Dynamic Type (`WKWebView.pageZoom`, 85 % to
  125 %, so the phone layout still fits).
- **Keyboard and insets**: the page draws edge to edge (`viewport-fit=cover`
  with the safe-area insets); Android resizes the WebView for the keyboard
  (`adjustResize`), iOS shrinks the visual viewport; the page follows both.
- **Offline**: a hub that does not answer the first load gets the app's own
  "Can't reach <hub>" screen with Retry, Hubs and Classic screens; once
  loaded, the page's own "Reconnecting…" handles losses.
- **Dark/light**: the window background and the system bars follow the
  page's theme (`setState({theme, bg})`), so there is no white flash.

## `window.backplaneApp` (bridge 1)

One script, `mobile/shell/backplane-app.js`, is injected by both apps at
document start into the hub's origin only (Android:
`WebViewCompat.addDocumentStartJavaScript` with the hub's origin;
iOS: a `WKUserScript` at `.atDocumentStart`, main frame only, whose
handler checks the frame's origin). The app replaces `/*@info*/null` in it
with `{platform, version, build, features, hub, textScale}` before injecting.
The transport underneath differs, nothing above it does:

- Android: `WebViewCompat.addWebMessageListener(webView, "BackplaneNative",
  {hub origin})`: the page posts with `BackplaneNative.postMessage(json)`;
  the app answers through the listener's `JavaScriptReplyProxy`, which the
  shim receives as `BackplaneNative.onmessage`.
- iOS: `webkit.messageHandlers.backplane.postMessage(json)`; the app answers
  with `evaluateJavaScript("window.backplaneApp._receive(" + json + ")")`.

Page → app: `{"id": 7, "op": "share", "args": {…}}` (`id` only when the
page awaits an answer). App → page: `{"id": 7, "ok": true, "value": …}` or
`{"id": 7, "ok": false, "error": "…"}`, and events `{"event": "open-thread",
"data": {…}}`.

| member | |
|---|---|
| `bridge` | `1` |
| `platform` | `"android"` or `"ios"` |
| `version`, `build` | the app's version name and build number |
| `features` | the ops this app has, e.g. `["camera","share","open","haptic","hubs","notify","files"]` |
| `hub` | `{key, name}` of the hub this page is (`host:port`) |
| `textScale` | the system text size the app applied (1 = 100%) |
| `requestCamera()` | Promise<bool>: true once the app holds the camera permission (asks the system). The page then calls `getUserMedia`; the app grants the page's request. |
| `setState({thread, needs, theme, bg})` | no answer. `thread`: the id of the thread on screen ("" for none; the page's own ids, no hub prefix); `needs`: how many threads need the person (iOS: the icon badge); `theme`: `"dark"`/`"light"`, `bg`: the page's background colour. Send when any changes. |
| `share({title, text, url})` | Promise<bool>: the system share sheet was shown |
| `openExternal(url)` | Promise<bool>: http(s)/mailto opened in the system browser |
| `haptic(kind)` | no answer; `"light"`, `"medium"`, `"selection"`, `"success"`, `"warning"`, `"error"` |
| `hubs()` | Promise<[{key, name, origin, current, online}]> |
| `switchHub(key)` | Promise<bool>: the app loads that hub's page |
| `addHub()` | the app's pairing screen (a new hub) |
| `openAppSettings()` | the app's own settings: hubs, Classic screens, notifications, version |
| `notifications()` | Promise<{allowed, asked}> |
| `requestNotifications()` | Promise<{allowed}>: asks the system (or opens its settings once refused) |
| `pairingFailed(cause)` | no answer: the page could not trade its token: `hub_unreachable` (the app shows its "Can't reach" screen and retries), or refused (`pairing_required`, …: the app re-pairs) |
| `notify({title, body, thread, key, kind})` | no answer: an alert the hub gave this page. The app posts it as its own notification (same channel and id rule as its other alerts: `key`, else `turn-<thread>`); a tap opens the thread (`open-thread`). None while that thread is on screen in front. |
| `on(type, fn)` | listen for an event; returns a function that stops listening. Events sent before the first listener of their type are kept and given to it. |

Events: `open-thread` `{thread}` (a tapped notification; the page selects
it), `share-in` `{text, files: [{name, type, data (base64)}]}` (shared into
the app: the page puts the text in the composer and attaches the files),
`resume` `{awayMs}` and `pause` `{}`. (iOS has no Share Extension yet, so
no `share-in` there.)

The page uses each op only when `backplaneApp` and the op are present
(`features`), so the same page works in a browser, in an older app and in
these apps; an app works with an older hub's page too (no `setState`: it
alerts for every thread, as a phone with the page closed would).

## Where

- Shared: `mobile/shell/backplane-app.js` (the shim).
- Web: `src/web/app-shell.js` (uses the bridge), phone-other.js's
  `inWebView`, the Settings row **This phone app** (view's
  `View.settings.phone.app`, shown only in an app).
- Android: `WebShell.kt` (the WebView, its clients, the bridge),
  `MainActivity.kt` (which screen), `AppSettings.classic`.
- iOS: see docs/ios-build.md.
