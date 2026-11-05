# M6 files stream: what is ported, how it is checked, what is not

Code: `native/host/src/files/` (Files tab, /img, git, file.open, pdf.page, mech.*, history.*), `native/host/src/kicad/` (plot actor, project files, boards, KiCad IPC).
Registered through api.rs by `files::start(hub)`. Raw numbers: `build/gpui/results/files/` (not in git).

## Decisions
- **Shell text is the Bend hub's.** The git, step, refs, resolve, pdf, mech-list, file-open and init scripts are extracted from the `.bend` sources into
  `native/host/src/files/scripts/*.sh` by `test/gpui/files/extract_scripts.py` (`--check` fails when they drift), so the history.git trust registry,
  `bp_safe` filter blanking and the capped diff are the same text. The hub's git runs with Proc.git.env (no fsmonitor, hooks or pager).
- **Path rules in Rust with a hostile corpus** (`hostile_paths.tsv`, Fm.safe) plus real-symlink containment tests (`files/fm.rs`); golden input for the later rules process.
- **Plots**: the Rust hub owns watch, poll (150 ms), delta, frames. The parser is the Bend child `--plot` until the Rust generator lands (sub-task); IPC transport the Bun `backplane-kipc` until the Rust nng/protobuf client lands. Both are Bun/JS in the process tree today.
- **Deliberate differences from the JS hub** (all refusals): unknown thread ids list/run nothing (the JS hub lists `/` and runs git in its own cwd); `kicad.watch`, `kicad.warm`, `kicad_ipc`
  and `mech.render` name only folders/files under known projects; NUL in a path is refused with the Fm.safe error; `file.open` is refused on a copied log.

## Tests (all on Boat)
`sh test/gpui/files/boat-test-job.sh` (harness crate of pure modules + the main crate's `files::` tests), `sh test/gpui/files/boat-build-job.sh` (release build, binary to `build/gpui/files-bin/`).
End to end against the JS hub on the desktop (finished binaries, own ports 3850-3859): `e2e_parity.mjs` (files.dir, /img), `e2e_plot.mjs` (kicad.watch/info frames byte for byte), `e2e_git.mjs`, `bench.mjs`, `bench_rpc.mjs`.

## Not ported
web stamps (`stamps`), split views, history `PWarm` beyond versions, `kicad.export10`, `kicad.install`, `review.*` (needs the rules process and a Rust BOM/ERC/release on the Rust parser),
`/files/get`, `/files/zip`, `/view/*` (no native client uses them), `kicad.components`, `kicad.renderAsset`, multi-account gh choice in git.act, model-written commit messages (agents stream).
