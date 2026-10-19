# Native hidden-page scheduling, October 18

At this October 18 measurement, Chrome was the default and CEF was opt-in.
The later Linux X11 default and desktop-input acceptance are documented in
[the native browser guide](../../docs/browser-native.md); the measurements
and their version/workload limits below are retained unchanged.

Unmapping CEF's outer X window did not mark its inner Chromium widget hidden.
The host now updates `_NET_WM_STATE_HIDDEN` on create/hide/show, preserving other
state atoms. CEF's pinned [native X11 wrapper](https://github.com/chromiumembedded/cef/blob/14c5a089e8452874cb1a556dc3fde26ed2d87723/libcef/browser/native/window_x11.cc)
forwards this property to its child to reduce resource usage. `WasHidden` is
for windowless rendering; this remains a native child window with no external
begin-frame loop or paint callback.

Playwright's automatic focus emulation also prevented hiding: Chromium's
[pinned implementation](https://github.com/chromium/chromium/blob/154.0.8037.98/content/browser/devtools/protocol/emulation_handler.cc)
obtains a capture handle with `stay_hidden=false`. The CEF transport suppresses
that permanent override. Hidden-page input and live remote/shared previews
instead obtain independent CDP handles only for their operation or watcher
lifetime. Expiry, page replacement, shutdown and failed input release them.
Native X mapping and keyboard focus remain independent of remote viewing.

An intermediate candidate reduced CPU by incorrectly throttling watched
animation. It was rejected. The final helper tests require live RAF/JPEG
cadence, changing JPEG pixels, no native map/focus change, independent input
and watcher lifetimes, and hidden scheduling after watcher expiry.

Three alternating rounds used the retained five-owner workload in
`test/tools/cef_remaining.ts`: a visible dashboard, two background animated
charts and two polling pages; background snapshots once/sec. Each sample is
8 seconds after 2.5 seconds settling. CPU includes the helper and its browser
process tree (100% = one core); memory is summed PSS, avoiding shared-library
RSS double counting. The Bend window and private Xvfb are excluded. CEF
154.0.8037.98 and installed headless Chrome155 stayed unchanged across the
baseline and fix; the engines are not version matched. Xvfb uses software GPU.

| Scenario | CEF CPU | Chrome CPU | CEF PSS MiB | Chrome PSS MiB |
|---|---:|---:|---:|---:|
| Static foreground | 4.44% | 3.86% | 542 | 445 |
| Animated foreground | 10.55% | 40.56% | 539 | 449 |
| Background work, no viewer | 5.52% | 5.15% | 546 | 425 |
| Background work, static foreground | 2.87% | 3.45% | 539 | 437 |
| Animated foreground and remote watcher | 52.30% | 71.99% | 576 | 460 |

Previously CEF background-only/background-with-view CPU was 23.08%/22.96%.
The fixed remote case delivers 246 JPEG frames per 8-second sample (about
30/sec); the watched hidden-native page becomes rendering-visible, while
unwatched pages remain hidden. Chrome also streams its local foreground,
so its total includes two JPEG streams (~486 frames); CEF paints the local
foreground natively and streams only the remote page. These are equivalent
product workloads, not equal numbers of encoded streams. Hidden polling and
agent snapshots continue. A separate network probe observed six successful
HTTP responses in six seconds while hidden and full-rate animation on restore;
native background timer throttling remains enabled. No page freezing, script
rewriting, timer disabling or Chromium flag changes were introduced.

`VISIBILITY.json` records individual CPU rounds, memory means, frame counts
and measured visibility. Complete local logs, raw samples, source/binary
hashes and screenshots are retained in
`bp-coord/browser-cef/visibility-watcher-results/`; rejected intermediate
results remain in `visibility-results/`. Original native app/wire binaries
were reused. Changed host protocol, helper, native-window/shared-preview/
teach/takeover, lifetime and Chrome regression checks passed. No Bend/law
source changed, so the previous check/proof/suite evidence is retained.

Remaining limits: five-page CEF PSS is still about 100–120 MiB higher. Unicode
clipboard round trips pass between CEF and an independent X11 process, but
real Japanese IME composition, desktop GPU rendering and drag-and-drop remain
unverified. A Japanese engine is absent; the needed administrator command is
`sudo apt-get install ibus-mozc`, followed by activation and real desktop
composition testing. No privileged installation was performed. Script-created
popup adoption also remains outside the covered popup-link case. macOS,
Windows and native Wayland embedding remain later work.

The change requires both the updated browser helper and a host built from
this revision. Deployment support landed first in PR #189 and was verified
running in the #194 build before this update. The existing hub deployment
now stages a matched helper/host bundle for machines with CEF installed,
checks its manifest, and promotes one immutable bundle through a shared
pointer. The older cached runtime remains available for rollback. Machines
without CEF do not install it. Explicit `BACKPLANE_CEF` paths still override
discovery and must refer to an updated host. The CEF distribution and engine
version are unchanged.

Integration was also checked against a copy of the actual running phase-1
native app: preview/teach/takeover and engine-switch checks pass using the
same measured helper and host. The fixture opts into isolated agent stand-ins.
These checks and the deployment lifecycle gates required no new native build;
production helper/host hashes still match the measured candidate.
