# Goal
Reach full feature parity with today's Backplane on a Rust hub and native apps (GPUI desktop, SwiftUI, Compose), keeping for small pure rules and today's layout and theme.

# Parts
GPUI experiment — native/gpui/ — Rust desktop client; software, nothing to order.
iOS experiment — mobile/ios/ — native SwiftUI iPhone/iPad app, separate experimental identity; nothing to order.
Android experiment — mobile/android/ — native Jetpack Compose app, separate experimental identity; nothing to order.
Rust hub — native/host/ — port of the hub, same protocol as today; software, nothing to order.
native research — docs/ — backend options, benchmarks and migration plan; nothing to order.

KiCad inside Backplane — native/host/src/kicad, native/gpui/src/viewers, ../Backplane_KiCad (fork) — the embedded KiCad editors as the obvious way to use KiCad: fast, comments by hotkey and voice, BOM / lifecycle / DRC-ERC windows, AI routing floorplanner with zones, embedded Gerber viewer and footprint/symbol editors, automated design review, fab capability profiles; software, nothing to order.

# Notes
Work off main: worktrees branch from origin/main and merge to main; the gpui and gpui-next branches are retired. Linux is the desktop target, tested on this machine. Builds run on Boat (8 GiB for big jobs, 3.7 GiB otherwise, zero swap) or the local Mac (hs-mac-mini), never on the desktop. Experimental hubs use separate homes and ports, on a copy of the store, never the live ~/.backplane-bend store or port 3787. Load tests simulate agents. Native apps only: no web page, web shell or streamed desktop. Phone apps connect to desktop.tail9e0cf2.ts.net over the tailnet. Network and database performance must be seamless, with no memory leaks. KiCad inside Backplane must work on Linux, Windows and macOS; Linux first, gated by a bare-install run on a clean Boat. CI stays on our own machines; GitHub runners only for signing or publishing. Fab limits merge into a project's DRC rules, never overwrite them.
