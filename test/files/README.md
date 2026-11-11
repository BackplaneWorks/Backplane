# M6 files stream: what is ported, how it is checked, what is not

Code: `native/host/src/files/` (Files tab, /img, git, file.open, pdf.page, mech.*, history.*), `native/host/src/kicad/` (plot actor, project files, boards, KiCad IPC).
Registered through api.rs by `files::start(hub)`. Raw numbers: `build/gpui/results/files/` (not in git).

## Decisions
  `native/host/src/files/scripts/*.sh` by `test/files/extract_scripts.py` (`--check` fails when they drift), so the history.git trust registry,
  `bp_safe` filter blanking and the capped diff are the same text. The hub's git runs with Proc.git.env (no fsmonitor, hooks or pager).
- **Path rules in Rust with a hostile corpus** (`hostile_paths.tsv`, Fm.safe) plus real-symlink containment tests (`files/fm.rs`); golden input for the later rules process.
- **Plots**: the Rust hub owns watch, poll (150 ms), delta, frames and the reader of every source: boards and schematics (`kicad/{sexpr,board,schematic,plotgen,plotread}.rs`), 3D models (kinds 2, 3, 4: `kicad/{glb,solid,solidread}.rs`) and `mech.render` (`kicad/{mesh,png,render}.rs`), each the same bytes as child. `BACKPLANE_PLOT_NATIVE=0` sends them all back to the child (`backplane-serve`, `BACKPLANE_PLOT_BIN`). The KiCad IPC transport is the hub's own Rust nng/protobuf client (`kicad/kipc.rs`, `nng.rs`, `proto.rs`), with the Bun `backplane-kipc` only when `BACKPLANE_KIPC_BIN` is set.
- **STEP has a host tool; which one is explicit** (`kicad/stepglb.rs`, called straight from Rust, never through a shell; meshing a B-rep is out of scope for this crate). `backplane-step2glb` first: native OpenCascade (`native/step2glb`, built by `scripts/build-step2glb.sh`), colours through XCAF, solids healed and meshed on every core, streamed as they are done (`convert_streaming`). FreeCAD is the last resort, running its own two scripts (`files/scripts/freecad_*.py`). `BACKPLANE_STEP2GLB=<path>` forces that tool; `BACKPLANE_STEP2GLB=off` forbids it (FreeCAD alone). No Bun anywhere in this path.
- **Bounds on a 3D read**: one model at a time; `Solid.most` (12 MB of GLB, the product's rule, kept with its words) before the wait for that turn; 8M triangles and 4096 JSON levels (`glb.rs`) past which a model is refused, where reader ran out of memory instead; 1 GiB of file for a render; `nice -n 10` on kicad-cli and the mesher, and on the render's worker threads (Linux).
- **Deliberate differences from the JS hub** (all refusals): unknown thread ids list/run nothing (the JS hub lists `/` and runs git in its own cwd); `kicad.watch`, `kicad.warm`, `kicad_ipc`
  and `mech.render` name only folders/files under known projects; NUL in a path is refused with the Fm.safe error; `file.open` is refused on a copied log.

## Tests (all on Boat)
`sh test/files/boat-test-job.sh` (harness crate of pure modules + the main crate's `files::` tests), `sh test/files/boat-build-job.sh` (release build, binary to `build/gpui/files-bin/`).
End to end against the JS hub on the desktop (finished binaries, own ports 3850-3859): `e2e_parity.mjs` (files.dir, /img), `e2e_plot.mjs` (kicad.watch/info frames byte for byte), `e2e_git.mjs`, `bench.mjs`, `bench_rpc.mjs`.
3D and renders: `glb_fixtures.py DIR` (48 hand-made GLBs, malformed ones included), `glb_corpus.sh OUT RUNTIME` (demo boards through kicad-cli), `mesh_compare.py OUT ORACLE BPPLOT DIR...`, `mesh_bench.py`, `e2e_solid.mjs` (JS hub against Rust hub, ports 3890-3899: 3D watches, `mech.render`, the process tree; `SOLID_FREECAD=1` meshes a STEP with no Bun, `SOLID_CHILD=1` is the fallback). `boat-step2glb-job.sh` builds the tool for the STEP oracle. `harness/tests/kicad_mesh_parity.rs` holds Rust against goldens binary wrote (`fixtures/mesh/`), so Boat checks parity with no . A fitted camera stands outside the model, so near-plane clipping is never reached by a render: only `mesh.rs`'s unit test covers it.

## Not ported
the hub binary's own `--render` flag (agents are told to run `<hub> --render`: main.rs must call `kicad::render::run`, not this stream's file; `bp-plot --render` does the same today), web stamps (`stamps`), split views, history `PWarm` beyond versions, `kicad.install`,
`/files/get`, `/files/zip`, `/view/*` (no native client uses them), `kicad.components`, `kicad.renderAsset`, the bundled library overlay (`effects/kicad_library.js`, only a build with a baked library has it), multi-account gh choice in git.act, model-written commit messages (agents stream).

## Design review and KiCad 10 copies
`files/rv.rs` (review.bom, checks, tick, comment, release; the parts change on a new pull request) and `files/export10.rs` (kicad.export10, and `mcp` for the agents' kicad_export10) run in the hub, with no `--review` child and no Bun. The rules are pure twins in `kicad/{bj,cad,bom,checks,release,review,kexport}.rs`; `bj.rs` is core/json exactly, because serde_json is neither as lenient nor keeps a number's text. Their shell text is extracted like the rest.
Deliberate differences, all refusals, listed at the top of `rv.rs`: a version must name the thread's own repository and a commit id (the JS hub runs `git show` anywhere, and a commit word starting with "-" is an option to it: `--output` writes a file); no checklist write through a link; a comment needs a thread with a folder; a hub on a copied log ticks nothing, releases nothing and runs no project's own checks.
`e2e_review.mjs` compares both hubs on the same paths (ports 3880/3881) and exits 1 when release, gate, publish or consent did not really run: equal failures are not parity. It must not override XDG_RUNTIME_DIR, which `systemd-run --user` needs. Results: `build/gpui/results/files/README.md`.

## KiCad IPC checks

- `sh test/files/boat-kipc-job.sh` (Boat): the transport's tests through `kipc-probe`, the binary, then the hub's `cargo test kicad::`. It exits 0 on the probe alone; read `HUB_TESTS=` for the hub.
- `golden_runtime_schema` skips where the runtime is absent, Boat included, because KiCad's descriptor set is not in this repository. Run `kipc-probe --golden test/files/fixtures/kipc_golden_runtime.json --desc <runtime>/share/backplane/api.desc` on the desktop.
- `bun test/files/kipc_hub_e2e.mjs HUB RUNTIME PORT WORK`: the hub, a stand-in agent and real KiCad through the MCP tools.
- The goldens in `fixtures/kipc_golden*.json` were written by protobufjs (the Bun helper, removed; `git show v0.14.1:test/files/kipc_golden.ts` has the generator). They are checked in and `kipc-probe --golden` compares proto.rs against them.

## Message images and attachments (gaps, for PARITY.md)
- `/message-image` serves only a snapshot a ref names. A message older than any ref answers 404 here; hub re-reads the text from store.db and snapshots on request.
- A ref hub wrote has no `thread` field, so the deleted-thread check cannot apply to it: that image stays readable with the root token after its thread is deleted. Refs this hub writes carry the thread.
- `POST /attach`: unknown, empty or deleted thread ids get 404 JSON (#960); size is decided by `files.max.mb` (the transport limit is being lifted).
