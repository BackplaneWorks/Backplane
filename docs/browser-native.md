# Embedded Chromium on Linux X11

The native window can embed CEF Chromium in its Browser pane. Agents use
CDP on the same page. Chrome remains the default and the fallback when the
CEF runtime is unavailable. Web clients, phones, and windows viewing a
remote hub receive JPEG frames through the existing watcher leases.

To try the optional engine from a source build:

1. Build the prebuilt CEF distribution with `scripts/build-cef.sh`, under the
   coordinator's normal resource admission. This builds the small CEF host;
   it does not compile Chromium.
2. Start Backplane with `BACKPLANE_CEF` set to the absolute path of
   `build/cef/Release/backplane-cef`. Keep its adjacent runtime files.
3. Open Settings and choose Browser → Embedded. Chrome switches back to
   the existing helper. Switching engines closes the previous engine's
   pages; switching threads within CEF preserves each page's state.

The runtime is separate from the base installer. Runtime discovery also
accepts `cef/backplane-cef` beside the browser helper or a verified runtime
under `$XDG_CACHE_HOME/backplane/cef-runtime/<version>/`. Download/install
support is implemented; the release URL and checksum are not published in
this opt-in increment. Without an installed runtime, Embedded reports the
error and Chrome continues to work.

The page initially belongs to the agent. Clicking it consumes that first
click, takes control, and focuses Chromium. Hand back restores agent control;
agent preview actions are refused during takeover or teaching. Bot teaching
records native clicks. The window hides its Chromium child under overlays
and restores it after dismissal. An empty Browser tab creates no page.

Only the in-process window or a native link over loopback may embed a child.
Browser WebSockets with an Origin header and proxied/tailnet clients cannot
embed. Placement is accepted only from the window that supplied the parent.

Validation covers a real app and CEF on a private Xvfb: visible page pixels,
CDP click/type/snapshot on that page, swallowed takeover click, native typing,
handback, focus return, palette, resize, per-thread state, popup links, bot
teaching, remote JPEG delivery, no local JPEG decode in CEF mode, and repeated
engine switching. See `test/tools/cef_window.ts`; helper checks live under
`tools/browser/test/` and `tools/cef/test/`.

Before enabling CEF by default, verify Japanese IME composition, clipboard
interoperation, drag and drop, and real GPU rendering on the desktop. Measure
several background agent pages as well as the visible page and remote
watchers: the initial spike found higher CPU for a hidden animated CEF page
than for an unwatched headless page. Do not claim that cost is resolved from
the single-page measurements in `tools/cef/SPIKE.md`. Script-created
`window.open` also needs a regression check; popup-link adoption is covered.
macOS, Windows, and native Wayland embedding are later work.
