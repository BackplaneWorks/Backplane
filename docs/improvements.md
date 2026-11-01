# Improvements

Use **Improvements** in the desktop/web top bar. Phone browsers use its icon
and a constrained sheet; when tab capture is unavailable, choose an image.
The window shortcut is **Ctrl+Alt+I** on Linux and Windows, and **Ctrl+Cmd+I**
on macOS. Cmd+Option+I and Cmd+Shift+I remain available to DevTools. The shortcut
belongs to the Backplane window, including its Browser pane; it is not global.
IME composition and AltGr text entry keep their keys.
Press the same shortcut again to close the popup, keeping its draft and image.

The textbox receives focus on open. Write the change, check the screenshot
below the input, then press **Send** or **Ctrl+Enter**. Composition keeps its keys.
Drag the desktop header, or focus it and use the arrow keys; Shift moves it
farther. The popup stays within the visible viewport. Phones use a constrained
sheet. Escape closes the web/desktop popup and restores the previous focus.
Closing keeps the draft and image on this device.

**Capture again** replaces the preview without clearing the request. Desktop
captures its own app contents and attached native Browser panes. The popup is
hidden while the image is taken. Web capture requires a browser supporting
Capture Handle and verifies that the chosen tab is this Backplane page; other
tabs, windows and monitors are rejected without reading their pixels. When
capture is unavailable or cancelled, **Choose image** uses the normal image
picker. No desktop source enumeration or unrelated-window capture is used.

Every request changes Backplane application code, regardless of the project
on screen. The popup resolves the unique Backplane source checkout from the
hub's project roots; an ambiguous or missing match shows the existing project
selector, labeled **Backplane source project**, for a deliberate choice.
The origin thread, bot and project are frozen when the popup opens and kept
with the draft as reference IDs, names and Backplane links, together with the
view and file path. Closing/reopening keeps that origin. They are not work targets.

Send creates a normal visible thread in that Backplane project, uploads the
image through `/attach`, then calls normal `turn.start`. It inherits the normal
new-thread provider, model, permissions and environment configuration. The
request's exact text and whitespace lead the message; a labeled app/view
context block and the normal attachment line follow. The context explicitly
identifies the screenshot as reference data. Opening or capturing dispatches
no thread or agent request.

A 32-character random creation key gives `thread.create` a stable thread ID.
Repeating a creation in the same existing project acknowledges the existing
thread without changes or effects. A conflicting project or removed thread is
rejected. `turn.start` uses one stable message ID, preserving the existing
message deduplication. After an uncertain start, retries keep the original
message and attachment. No new agent harness, tool permission mode, approval
route, upload limit, or upload streaming protocol is introduced.

Drafts and screenshots stay in authenticated-hub-scoped IndexedDB, including
phone browsers. Uploads use the existing authenticated attachment endpoint.
RPC waits and capture/upload operations have bounded failure paths. Failures
keep the request and image and permit retrying the same submission.
Send reports missing text, screenshot or project with an actionable message.
During dispatch it shows thread creation, upload and agent start progress.

## Qualification

`desktop/test/improvements.test.cjs` covers shortcut exclusions, popup bounds,
retry identity, exact request preservation, normal attachment syntax, double
submission, upload failure, RPC timeout/disconnect, and rejection/cleanup of
unrelated web capture streams.

`desktop/test/improvements-capture-integration.cjs` uses a private Electron
runtime without the installed app/service and a private Xvfb display. It
executes the source popup and actual Electron capture APIs, verifies unobscured
pixels, native Browser child-pane capture, dragging/clamping, recapture failure
with preserved selection, Escape focus restoration, HTTP PNG upload, Send
retry/double-click handling, and phone-width layout. Its RPC responses are
fixtures, so it does not prove hub/provider execution or native phone capture.
The real planner is exercised by `test/improvements_test.bend`.

`test/tools/improvements-gates.sh` records the focused planner/worktree checks,
required regression set, repository type checks and mandated proof separately.
No installation or restart is part of these commands. Native iOS/Android
sheets are a separate unqualified source experiment and are excluded from
this desktop/web delivery. Preserve a separate source hash for every
qualification run.
