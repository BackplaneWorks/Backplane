# Goal
Find how much smaller and faster Backplane can be with a GPUI desktop UI and the Bend backend compiled to native machine code, preserving today's layout and theme.

# Parts
GPUI experiment — native/gpui/ — Rust desktop client; software, nothing to order.
iOS experiment — mobile/ios/ — native SwiftUI iPhone/iPad app, separate experimental identity; nothing to order.
Android experiment — mobile/android/ — native Jetpack Compose app, separate experimental identity; nothing to order.
Bend native research — docs/gpui/ — backend options, benchmarks and migration plan; nothing to order.

# Notes
Use the gpui worktree/branch; never merge into main without approval. The local Mac (hs-mac-mini) builds only what needs it: macOS GPUI and iOS. Android builds run on Boat or the desktop, other heavy builds on Boat, capped at 8 GiB for big jobs or 3.7 GiB otherwise, with zero swap. Experimental hubs use separate homes and ports, never the live ~/.backplane-bend store or port 3787. Use native phone screens rather than a web shell or streamed desktop. Phone apps connect to desktop.tail9e0cf2.ts.net over the tailnet. Network and database performance must be seamless, with no memory leaks.
