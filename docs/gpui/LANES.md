# Rust port lanes

One owner per file. New lanes work in their own git worktree, so an unfinished file in one
lane cannot break another lane's build.

| Lane | Thread | Owns |
|---|---|---|
| core (lead) | `bend` | `native/host/src/{main,hub,model,store,auth,cbor,presenter,api}.rs`, copy guard, the module split |
| sidebar | `slim` | `sidebar.rs`, `shelf.rs`, `signal.rs`, `state.rs`, `test/gpui/parity`, `scripts/gpui/law-map.*` |
| files | `files` | `native/host/src/files/`, `native/host/src/kicad/` |
| agents | `agents` | `native/host/src/agents/` except `providers/`, MCP tools |
| parity-session | new | `native/host/src/session/` |
| parity-edge | new | `native/host/src/wire/` |
| prov-acp | new | `native/host/src/agents/providers/{acp,grok,opencode}.rs` |
| prov-msp | new | `native/host/src/agents/providers/{msp,muse,agy}.rs` |
| prov-pi | new | `native/host/src/agents/providers/{pi,omp,dialog}.rs` |
| state | new | `native/host/src/state/` |
| soak | new | `test/gpui/soak/` |

## How a lane works

- Worktree: `git worktree add ~/Documents/i2cjak/bp-gpui-lanes/<lane> -b gpui-<lane> gpui`.
  Copy nothing real into it. Binaries in the main worktree's `build/gpui/` can be used by
  absolute path.
- Build and test from that worktree with `scripts/boat-job.sh`. Boat or the Mac only.
- Pick up others' work with `git rebase gpui` in your own worktree.
- Land small and often: take the named lock `gpui-merge`, run
  `git -C /home/h/Documents/i2cjak/backplane-gpui cherry-pick <sha>...`, release the lock.
  If a cherry-pick refuses, abort it and tell the owner of the file. Never force anything.
- To change a file you do not own, ask its owner by `thread_send`.
- A shared seam (one line in `main.rs`, one `mod` line) goes through the lead.
- Before landing a batch, send the `reviewer` thread its commit range and wait for its OK. Land small batches, not every commit.
