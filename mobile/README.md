# Phone apps

Native iOS (SwiftUI) and Android (Jetpack Compose) clients for a Backplane
hub. The client logic is the same Bend the web page runs: `src/mobile/`
turns `core/client.bend`'s `Ui` into a screen model (JSON), and
`bridge.js` carries strings between it and the app. The apps own the socket
and draw the screen with native lists, toolbars and text fields; they make
no product decisions. iOS runs the bridge in JavaScriptCore, Android in
QuickJS.

```sh
scripts/build-mobile.sh           # bridge.js + dist/backplane-android.apk
scripts/build-ios.sh sim          # build on the Mac, run in the Simulator
scripts/build-ios.sh archive      # sign and upload to TestFlight
```

`build-ios.sh` bundles bridge.js here (bend), rsyncs `mobile/ios` to
`~/backplane-ios` on the Mac (`BACKPLANE_MAC`, default `hs-mac-mini`) and
runs xcodebuild there. Debug builds read `BACKPLANE_LINK` and
`BACKPLANE_SELECT` from the environment (`SIMCTL_CHILD_…`) to pair and open
a thread without taps.

Pairing: scan the code Settings shows (Pairing link, QR code) from the
pairing screen or Hubs (iOS reads it with the camera, Android with Google
Play services' code scanner), paste the hub's tailnet link
(`http://host:3787/#token=…`), or open
`backplane://pair?url=<that link, URL-encoded>`, which is what the code
holds, so the phone's own camera app opens Backplane with it too.

Kept state: each app writes the Bend client's whole state down (a file
named `state-<key>.json`) and loads it at launch, then asks each hub only
for what came since. The key is a hash of the Bend type definitions
(`scripts/state-key.py`, stamped at the end of bridge.js by
`build-mobile.sh`), so a new build keeps the state unless a type changed.

Several hubs: the phone stays connected to every hub it pairs with, one
socket and one Bend client each (`src/mobile/hubs.bend`, keyed by
host:port). The screen is all of them in one: each hub's projects, named
by machine when there are several; alerts and the island from all; the
thread of the hub in focus (the one whose thread was opened last). Ids on
the screen carry their hub (`host:port|id`), and Bend routes each action
by that prefix (laws `hubs_*`, test `test/hubs_test.bend`). The link
button lists the paired hubs (unpair there), the owner's other machines
they know of (one tap pairs), and takes a new link. Only the hub in focus
draws plots. Debug builds take several links in `BACKPLANE_LINK`,
separated by spaces.

## A thread on the phone

The screen carries what the web client's thread view has, decided in
`src/mobile/screen.bend`: asks (approve, answer, implement a plan), runs
of tool calls folded (`fold`), subagents and links between threads,
attachments (the app reads a photo or file and sends it through `attach`
in the pieces the screen names), the images a reply names (served by the
hub at `/img` with the pairing token; only the newest 40 entries look,
since finding them walks the text), `$skill` completion, the diff and git
actions, the terminal (the hub's VT emulator, keys through `term-key` and
`term-paste`), search and the file picker (`find-open`), settings, and
the archived shelf. Debug builds read `BACKPLANE_ACTS`
(`action=value;;…`, two seconds apart once the thread is open;
`@attach` uploads a test image) to drive these without hands.

## Board viewer

A thread's toolbar opens its project's board, schematic or 3D model (the
chip icon). The hub sends a plot (`src/core/plot.bend`) as CBOR written
straight by Bend: chunks of geometry in paint order, then, on each save,
only the chunks that changed (laws `plot_delta_exact`,
`plot_unchanged_sends_nothing`). Plot frames go straight from the socket
to the renderer, never through bridge.js.

The phone uploads a plot to the GPU once; pan, pinch, double-tap and (in
3D) orbit only move a transform, so every frame costs the same whatever
the board. Each layer is drawn as coverage (tracks, arcs and dots as
instanced capsules with an analytic edge, fills by stencil) and laid over
the frame in its colour and opacity. Frames are drawn only while a finger
moves, a fling coasts or new chunks fade in. iOS: Metal (shaders compiled
on the device, so the build needs no Metal toolchain), up to 120 Hz.
Android: OpenGL ES 3.

A tap is inspected on the phone: it hit-tests its own geometry, Bend picks
the smallest piece under the finger (laws `pick_*`), and the hub answers
with that piece's info from the chunks this phone holds; the card offers
"Mention in chat". 3D draws the board's body and parts (kicad-cli, exported
by the hub in a child process) with the plot's layers laid sharp on its
faces; before the model arrives, a slab of the board's thickness.

3D moves like SolidWorks: the camera turns freely (no up axis, no limit)
about the centre of the board's rectangle (the plot's `edge`, its
Edge.Cuts box) halfway through its thickness, which keeps its place on
screen after a pan. One
finger turns the model under it (about the screen's axes), two fingers
drag it, pinch zooms toward the fingers, twisting two fingers rolls it
about the view axis, and a double tap fits it again. The model is drawn 4x
multisampled; the layers on its faces keep their analytic edge.

Debug builds also read `BACKPLANE_VIEW` (board, schematic, 3d) and
`BACKPLANE_TAP` (x,y in points) to open the viewer and tap without hands.

## TestFlight

`archive` needs `~/backplane-ios/signing.env` on the Mac (never committed):

```sh
TEAM_ID=XXXXXXXXXX                    # Apple Developer team
BUNDLE_ID=dev.backplane.mobile        # registered App ID; the app record uses it
ASC_KEY_ID=XXXXXXXXXX                 # App Store Connect API key (App Manager)
ASC_ISSUER_ID=xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
ASC_KEY_PATH=/Users/h/.appstoreconnect/AuthKey_XXXXXXXXXX.p8
```

## Notifications and the Dynamic Island

When a turn ends (Completed or Failed; a stop the user asked for is not
news), the phone alerts, and while turns run it shows a live status. What
counts is decided in `src/mobile/notify.bend` (laws `alert_*`).

- iOS in front: the app posts the alert itself and runs one Live Activity
  (lock screen + Dynamic Island) from the screen's `island`.
- iOS asleep: the hub pushes through APNs (`src/server/push.bend`): an
  alert, and Live Activity updates to the activity's push token. The app
  registers its tokens with the `device.register` rpc; tokens never leave
  the hub (laws `device_*`).
- Android: a foreground service keeps the socket open while turns run and
  shows an ongoing notification (promoted to a status-bar chip on Android
  16); alerts come from the same Bend `notify` commands.

APNs needs a key from developer.apple.com (Keys, with Apple Push
Notifications service enabled). Put the .p8 on the hub machine and set:

```
push.apns.key    /path/to/AuthKey_XXXXXXXXXX.p8
push.apns.keyid  XXXXXXXXXX
push.apns.team   <Team ID>
```

(settings via `setting.set`; only the path is stored, never the key).
Until all three are set (and the key file exists) the hub logs `backplane:
push: phones are registered but ...` at each turn end instead of pushing.
The key must belong to the team that owns the app's bundle id, or APNs
answers `TopicDisallowed`.

A turn's end can also POST elsewhere. Set `push.webhook.url` to an `http`
or `https` address (unset: nothing is sent). `push.webhook.secret` is an
optional path to a file holding an HMAC-SHA256 secret; with it unset,
`<home>/secrets/push.webhook` is used when that file exists. The notice
and the signature headers are in `docs/bots.md` ("Webhook"). A failed POST
is logged and does not affect the turn.
