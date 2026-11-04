# Goal
Find how much smaller and faster Backplane can be with a GPUI desktop UI and the Bend backend compiled to native machine code, preserving today's layout and theme.

# Parts
GPUI experiment — native/gpui/ — Rust desktop client; software, nothing to order.
Bend native research — docs/gpui/ — backend options, benchmarks and migration plan; nothing to order.

# Notes
Use the gpui worktree/branch; never merge into main without approval. Heavy builds run only on Boat, capped at 8 GiB for big jobs or 3.7 GiB otherwise, with zero swap. Experimental hubs use separate homes and ports, never the live ~/.backplane-bend store or port 3787. iPhone access stays on desktop.tail9e0cf2.ts.net over the tailnet.
