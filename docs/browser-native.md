# Embedded Chromium on Linux X11

The native window can embed CEF Chromium in its Browser pane. Agents use
CDP on the same page. CEF is the default on Linux X11; an explicit Chrome
choice remains available. When no native window or usable runtime exists,
Chrome frames continue with a visible fallback reason. Web clients, phones, and windows viewing a
remote hub receive JPEG frames through the existing watcher leases.

Linux x86_64 release builds include the native runtime. Other platforms keep
Chrome frames. To supply CEF from a source build:

1. Build the prebuilt CEF distribution with `scripts/build-cef.sh`, under the
   coordinator's normal resource admission. This builds the small CEF host;
   it does not compile Chromium.
2. Start Backplane with `BACKPLANE_CEF` set to the absolute path of
   `build/cef/Release/backplane-cef`. Keep its adjacent runtime files.
3. Open the Browser pane; Embedded is selected automatically. Settings →
   Browser → Chrome explicitly switches back. Engine changes reload all
   owned tabs while retaining selected tabs, cookies, local/session storage,
   non-password form drafts and scroll. Profiles remain separate. JavaScript
   heaps, history and file selections cannot transfer between processes.

The Linux x64 public archive has suffix `linux-x64-cef.tar.gz`. Its installer
verifies and promotes an immutable matched helper/host/runtime bundle with
notices. Existing explicit Chrome settings and browser profiles are retained.
Older public self-updaters cannot install the bundle: they safely find no
matching old archive name. Run the new release's `install.sh` once to bootstrap;
subsequent self-updates use the same matched installer. These install/update
operations require Python 3; running the installed browser does not. The .deb
declares CEF library dependencies; tar/AppImage users need those system libraries.

Runtime discovery also accepts `cef/backplane-cef` beside the browser helper
or a runtime under `$XDG_CACHE_HOME/backplane/cef-runtime/<version>/`. Without
a usable runtime, Embedded reports the error and Chrome continues to work.

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
engine switching. The window-driven test (`test/tools/cef_window.ts`) went with the native window; helper checks live under
`tools/browser/test/` and `tools/cef/test/`.

Real Japanese input is checked with IBus Mozc, an independent GTK editor and
native X keys: input/textarea/contenteditable preedit, candidate selection,
commit/cancel, hide/show and teach recording. A desktop Japanese engine must
be installed and selected; Unicode paste alone is not IME acceptance. Native
GTK file/text drags, Unicode file names/bytes, outbound text, owner switching,
shield rejection, outside-panel attachments and stale/hidden targets are
covered by `tools/browser/test/native-drag.ts` using the real window effect.

GPU acceleration remains unverified on the workstation; private Xvfb uses
software rendering. The retained five-page [visibility study](../tools/cef/VISIBILITY.md)
fixes hidden scheduling and validates watcher animation, but CEF PSS remains
about 100–120 MiB higher in that workload. These limits were not reclassified
as solved when the user requested the default change.
The normal hub deployment updates the optional host and helper together on
machines with CEF installed, using a verified immutable bundle; it leaves
Chrome-only installations alone. Explicit `BACKPLANE_CEF` overrides still need
a matching updated host. Script-created `window.open` stalls under browser-level
CDP attachment in the pinned runtime: a button's opening script/automation can
hang, affecting popup login flows. Popup links are covered and pass. The
[upstream CEF issue](https://github.com/chromiumembedded/cef/issues/4308) matches
the reproduced stall; its proposed fix remains an unvalidated draft. The release
workflow runs `tools/browser/test/popup-cef.ts` as a required functional gate;
this remains a release blocker, with no waiver or Chromium source build.
macOS, Windows, and native Wayland embedding are later work.

Build a Linux x64 release with `BACKPLANE_CEF_RELEASE=1 scripts/build.sh`,
then `scripts/package.sh VERSION linux-x64`. The package refuses missing or
unmatched CEF payloads. AppImage/deb layouts also retain the adjacent runtime.
`scripts/install-release.sh` serves the public installer and native updater;
`scripts/cef-runtime.py` validates files before promotion and keeps the previous
pair for rollback. Local dev deployments retain their optional-runtime policy.
Bundles include pinned `LICENSE.txt`, the upstream CEF redistribution README,
and `resources.pak` containing `about:credits`.
