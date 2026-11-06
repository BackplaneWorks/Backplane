# Goal
Reach full feature parity with today's Backplane on a Rust hub and native apps (GPUI desktop, SwiftUI, Compose), keeping for small pure rules and today's layout and theme.

# Parts
GPUI experiment — native/gpui/ — Rust desktop client; software, nothing to order.
iOS experiment — mobile/ios/ — native SwiftUI iPhone/iPad app, separate experimental identity; nothing to order.
Android experiment — mobile/android/ — native Jetpack Compose app, separate experimental identity; nothing to order.
Rust hub — native/host/ — port of the hub, same protocol as today; software, nothing to order.
native research — docs/ — backend options, benchmarks and migration plan; nothing to order.

# Notes
Work off main: worktrees branch from origin/main and merge to main; the gpui and gpui-next branches are retired. Linux is the desktop target, tested on this machine. Builds run on Boat (8 GiB for big jobs, 3.7 GiB otherwise, zero swap) or the local Mac (hs-mac-mini), never on the desktop. Experimental hubs use separate homes and ports, on a copy of the store, never the live ~/.backplane-bend store or port 3787. Load tests simulate agents. Native apps only: no web page, web shell or streamed desktop. Phone apps connect to desktop.tail9e0cf2.ts.net over the tailnet. Network and database performance must be seamless, with no memory leaks.
