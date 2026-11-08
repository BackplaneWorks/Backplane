# Backplane

An agentic hardware development environment: an agent harness with deterministic
viewers for KiCad boards, schematics and STEP models. The hub is Rust (`native/host`),
the desktop client is GPUI (`native/gpui`), the phone apps are native (`mobile/`).
There is no JavaScript hub and no web client in this tree.

## Branches

- Work happens in your own worktree on your own branch: `git worktree add .backplane/worktrees/<name> -b <name> main`.
- Threads merge to `main` themselves: when your tests pass, `git rebase main`, then fast-forward `main` from the checkout that has it (`git -C <that checkout> merge --ff-only <branch>`). No script, lock or review step. Never force, never merge-commit, never push unless the person says so.
- Every move of `main` in this repo is built on this machine (`scripts/main-build.sh`, started by a hook from `scripts/install-hooks.sh`; `docs/MAIN-BUILD.md`). After the merge, `scripts/main-build.sh status` shows `built:` reach your tip, and `~/.backplane-gpui-dev/main-build.log` says `published` or `FAILED`. A red test publishes nothing and leaves the bar red until the next merge fixes it, so run the tests before you merge.
- A published build reaches the running app (the fresh hub on `~/.backplane-gpui-fresh`, `scripts/fresh.sh`) only through its watcher, `scripts/fresh-watch.sh`: it shows the build bar, swaps the window at once, and offers Restart now while turns run (`docs/HUB-RESILIENCE.md`, `docs/DEV-RELOAD.md`). The watchdog timer (`scripts/hub-watchdog.sh install`) brings a dead watcher back. Both act on the person's live app, so the person starts them in their own terminal; an agent never does (a watcher started from an agent turn carries `BACKPLANE_AGENT_OWNER` and the next hub restart kills it). If `~/.backplane-gpui-dev/CURRENT` names your build but the running `backplane-gpui` (`readlink /proc/<pid>/exe`) is older, the watcher is not running: tell the person, and check `scripts/hub-watchdog.sh status`; never relaunch the app yourself.

## Layout

- `native/host/`: the Rust hub (`backplane-hub`). HTTP + WebSocket, event store, agent processes (Claude, Codex, Grok, OpenCode, Muse, Antigravity, Pi, Oh My Pi), KiCad (in-process IPC, plots, STEP), bots and rooms, pairing, far threads, terminals. The presenter (`present/`, `presenter.rs`) is Rust only; there is no helper process and no `--policy-bin`. The CBOR dictionaries are in `src/wire_dict.rs` (append-only forever, the wire depends on the order).
- `native/voice/`: the voice helper (`backplane-voice`), a child process of the hub: speech to text by OpenAI or locally through sherpa-onnx's C API, NDJSON on stdout (its `README.md`).
- `native/gpui/`: the GPUI desktop client (`backplane-gpui`); `vendor/` is vendored crates. Its colours come from `assets/web/style.css`, its logo from `assets/web/favicon*.svg`.
- `mobile/ios`, `mobile/android`: the phone apps. They speak the hub's data/1 protocol (`docs/PHONE-DATA.md`).
- `tools/`: helper programs that are not Rust: `browser` (headless Chromium over NDJSON), `cef`, `step2glb`, `voice`, `platform`. Built by `scripts/build-*.sh`.
- `order/`: the order server (`fabsrv`, Python) and deploy. `skills/`: agent skills. `deploy/kistack`: KiStack's skills, pinned by `deploy/kistack.lock` (`python3 scripts/kistack.py bump <commit>`). `i18n/`: message catalogs. `assets/`: fonts, icons, web colours.
- `scripts/`: dev, build, fresh/watch, side-by-side, switch and device scripts; `migrate-from-bend.sh` copies an old `~/.backplane-bend` home into a Backplane home (the old one is only read). The hub's own import (`native/host/src/import014/`, `docs/IMPORT-0.14.md`) is what a person uses: setup and Settings offer it, and it copies into a home already in use. `test/`: fixtures, oracles and e2e drivers; `test/tools/claude_standin.ts` and `perf_standin.ts`: the stand-in agents.
- `docs/`: how this tree works (`BUILD.md`, `DEV-RELOAD.md`, `MAIN-BUILD.md`, `SIDE-BY-SIDE.md`, `COPY-GUARD.md`, `HUB-API.md`, `PHONE-DATA.md`). Other `docs/*.md` describe features the hub keeps.

## Commands

```sh
cd native/host && cargo test --offline          # hub tests
cd native/gpui && cargo test --offline          # client tests
scripts/dev-build.sh --gate --no-cut       # both crates' tests, then fast-profile builds; no publish
scripts/dev-build.sh                       # build and publish to the person's `watch` terminal via dev-cut.sh
scripts/main-build.sh --once               # build and publish the tip of main now
scripts/bp-build.sh                        # build/test on a remote builder (Boat or h@backplane), when the desktop is busy
backplane-hub --home DIR --port N --no-tailscale --foreground   # run a hub (temp home!)
```

`dev-build.sh` honours `BUILD_ROOT` (the checkout to build), `PROFILE` (default `fast`), `HOST_TARGET`/`GPUI_TARGET`. Tests that need agents use stand-ins only.

## Rules

- Stand-in agents only in tests: set `BACKPLANE_AGENTS=<dir>` (the stand-ins go first on PATH; a missing CLI is blocked, never the real one). Real model turns only on temp homes, with the person's go.
- Never write to live user data (`~/.backplane*`, `~/.local/share/backplane`). Tests use a temp home, a port 3799+ and `--no-tailscale`. A hub on a copy of a real log runs with `BACKPLANE_COPY=1` (`docs/COPY-GUARD.md`); set `restart.continue` to `off` first so it does not resume real turns.
- Never run the installed `backplane`/`backplane-hub` to probe it; ask `/hello`.
- Never kill processes by name or pattern; only PIDs you captured.
- Persisted events stay decodable forever: add event variants, never change the meaning of one. CBOR dictionary entries are append-only.
- Every feature matches everywhere it applies: GPUI client and both phone apps, driven by the hub's data.
- The UI is dumb: decisions (order, settle state, what is visible) come from the hub or a pure function with a test.
- No continuous repaint loops: animate only while a fade or stream is in flight.
- Square corners, minimal copy, no decorative headings.
- Build and test through `scripts/cargo-q` (`scripts/cargo-q test --offline` in `native/host` or `native/gpui`), not bare `cargo`: it waits for one of the machine's few build slots (two here, from cores and free memory), uses the shared `sccache` (compiled dependencies are shared by every worktree, so a new lane does not compile tokio, naga, zbus... from cold) and sizes `-j` from free memory. `scripts/dev-build.sh` does the same. When your branch has landed, `git worktree remove` it (or run `scripts/worktree-gc.sh --apply`: it removes merged, clean, idle worktrees and idle `target/` dirs; `main-build.sh` runs it when the disk is short).
- **Offload to Boat.** When every local build slot is taken, `scripts/cargo-q` (`test`, `build`, `check`, `clippy`, `doc`, `bench`) and `scripts/dev-build.sh` (`--gate`, cuts) do not wait: they run the same command on a Boat sandbox (`bp-build-N`, pool of `BP_BOAT_POOL`=2, 8 vCPU) and behave like a local run: same output on stdout/stderr, same exit code, built programs copied back to the same `target/` paths. So lanes call `scripts/cargo-q`, never bare `cargo`, and the offload happens on its own; never start a Boat for a build by hand. `BP_NO_BOAT=1` forces local. How: the exact worktree (tracked and untracked files, committed or not; no `.git`, nothing ignored) is rsynced to the box, the same script runs there (sccache and `target/` stay warm on the box between jobs), and programs come back after a sha256 check. A `dev-build.sh` cut made on a box is hashed here and always smoke-started here before it is published (`SKIP_SMOKE` is ignored for it). Nothing of the person's goes to a box: no home, no tokens, no `.git`; the box comes from the snapshot `bp-build-base` (made on a sandbox with no environment, `scripts/boat-pool.sh base`) and the job gets only build knobs. Idle boxes are stopped after `BP_BOAT_IDLE` (300 s; `scripts/boat-pool.sh status|reap|stop|delete`); the disk is kept, so the next job resumes warm. If Boat cannot be used (offline, no quota) the build waits for a local slot as before. Commands that need this machine (`cargo run`, `--no-run`, `--target-dir`, `--manifest-path`, absolute paths outside the worktree) stay local. **iOS and macOS builds stay on the Mac mini under `mac-heavy`**; they never go to a Boat. Details: `docs/BOAT-BUILD.md`.
- `/tmp` is a 30 GB RAM tmpfs with an inode quota; lanes filled it twice (EDQUOT: Claude Code could not write its temp files). Lane temp space is on disk: `. scripts/lib-bptmp.sh` sets `BP_TMP` (default `~/.cache/bp-tmp`), `TMPDIR` and `CLAUDE_CODE_TMPDIR` to it; `cargo-q`, `dev-build.sh` and the test runs already do. Put temp homes, logs, hub copies and scratch binaries in `$BP_TMP` (or `mktemp -d` with `TMPDIR` set), never under `/tmp`. Never set `CARGO_TARGET_DIR` under `/tmp` (`cargo-q` and `dev-build.sh` refuse); use the worktree's `target/` or `$BP_TMP/target-<lane>`. Remove your scratch when done; `scripts/worktree-gc.sh --apply` also sweeps stale `bp-*`/`bpt-*` dirs in `$BP_TMP` and `/tmp`.
- Keep disk headroom (`df -h /` first; 20 GB free); delete your own build leftovers. Builds of `main` run in the builder's own checkout and target dirs, at low priority; a build from a worktree uses its own target dir, not another's.
- Commits made on our machines have author AND committer `build@amemb.com` (name `i2cjak`), on every machine and Boat. jlcjak's GitHub account (`206324213+jlcjak@users.noreply.github.com`, with GitHub as committer of web merges) may also commit; nobody else: `scripts/install-hooks.sh` sets that in the repo's own config and installs the pre-commit, pre-push and main-move checks (`scripts/check-author.sh`; `scripts/land.sh` and CI use it too). Never change the global `~/.gitconfig`, never set `GIT_AUTHOR_*`/`GIT_COMMITTER_*` or `git -c user.email=` to anything else. A clone made elsewhere (another machine, a Boat setup) runs `scripts/install-hooks.sh` before its first commit.
- Before pushing, verify the GitHub account (`i2cjak`), the remote (`BackplaneWorks/Backplane`), and the branch. Push only when asked.
- Process spawning goes through `native/host/src/proc.rs` (its kinds decide what a copy run refuses); no other non-test `Command::new` under `src/`.

## Links

- A link you give the person to something served on a machine (a dev server, preview, review page) must open from their other machines on the tailnet: `localhost`/`127.0.0.1` open only on the serving machine. Your own probes stay on loopback.
- Name the machine by its MagicDNS name `<machine>.<tailnet>.ts.net` (its `Self.DNSName` in `tailscale status --json`, no trailing dot), with real names filled in. Keep the server on `127.0.0.1` and share it with `tailscale serve --bg --https=<free port> http://127.0.0.1:<port>` after asking; turn it off when done. Leave 443 alone, never `tailscale serve reset`, never `funnel`, never bind `0.0.0.0` or drop sign-in/TLS for a link.
