# Windows

> Paths under `src/` below (`src/core`, `src/server`, `src/web`, `src/mobile`, `src/app`) name the Bend JS tree, removed from this checkout (tag `main-bend-final`). Where the current Rust code for each is, is unverified; look under `native/host/src/` and `native/gpui/`.

> The text that used to be here described the Electron app over `backplane-serve.exe`, built with `scripts/build-js.sh`, released with `scripts/service-release.py` and set up with `scripts/build-windows-helpers.sh`, `scripts/install-windows-providers.ps1` and `scripts/setup-windows.ps1`. None of those scripts or `desktop/` exist in this checkout any more, so those steps are gone.

What the current files confirm:

- The hub is `native/host` (`backplane-hub`) and the window is `native/gpui` (`backplane-gpui`), both Rust. The release workflow (`.github/workflows/release.yml`) builds them with `cargo build --release` for `linux-x64`, `linux-arm64` and `darwin-arm64` only. **There is no Windows target in it.**
- `scripts/dev-build.sh` builds the same two crates and publishes them with `scripts/dev-cut.sh`, which accepts ELF programs only (it checks for the `\x7fELF` header), so it does not publish a Windows build.

## Status: Windows was never built (the requirement is dropped for now)

The Windows E2E lane (thread t1793985962-58) was stopped before it compiled or ran anything: its `cargo check` for `x86_64-pc-windows-msvc` was cancelled while queued, and nothing landed. What follows comes from reading the source at `ed50df6d35`, not from compile errors. **Whether either crate builds on Windows is unknown**, but the findings say it will not without a port.

Hub (`native/host`) blockers:

- Terminal is a POSIX pty only (`term/pty.rs`, `term/mod.rs`); it needs ConPTY.
- Unix sockets for KiCad IPC (`kicad/nng.rs`, `kicad/kipc.rs`, `kicad/ipc.rs`, `proc.rs`).
- Signals and process groups (`kill`, `setsid`, `pre_exec`) in `agents/proc.rs`, `agents/work.rs`, `term/pty.rs`, `settings/vault.rs`, `main.rs`, `copy.rs`, `agents/providers/mod.rs`; shutdown uses `tokio::signal::unix`.
- Unix-only file APIs (`PermissionsExt`, `symlink`, `MetadataExt` and others) and about 79 direct `libc` uses (`flock`, `getifaddrs`, `localtime_r`, `setrlimit`, `getuid`), tests included.
- Hard-coded Unix paths and tools in at least 25 files (worst: `files/git.rs`, `agents/providers/opencode.rs`, `proc.rs`, `files/misc.rs`).
- The secrets vault detects only a macOS keychain and a Linux keyring (`settings/vault.rs`); 18 `HOME`/`XDG_` uses.

Client (`native/gpui`) blockers: `Cargo.toml` declares `gpui` for Linux and macOS only; the vendored `gpui-0.2.2/build.rs` compiles the Windows shaders only on a Windows build host, so a Linux cross check proves little; `screens/place.rs` has a `cfg(unix)`-only `on_term`, `screens/improvements.rs` opens links with `open`/`xdg-open`, and `screens/capture.rs` and `screens/timeline.rs` have Linux-only paths. CEF is pinned at 154.5.0 and its packaging is unproven on Windows.

Packaging: `tools/cef/CMakeLists.txt` and the `tools/*/build.ts` files have no Windows handling, and `scripts/` has no Windows packaging apart from a Windows target mention in `build-step2glb.sh`. CI has no Windows job.

Still unknown (needs a real Windows build): the real compile-error count for either crate (unresolved imports can hide later errors), and whether the hub needs the Windows SDK (its lock shows none of `ring`, `aws-lc-sys`, `openssl-sys`, `cc`, `cmake`, `bindgen`; the GPUI lock has `ring`, `cc`, `cmake`, `bindgen`). Toolchain notes: `clang-cl` is on the desktop; `mingw`, `xwin`, `lld-link` and `wine` are not, and `h@backplane` has no `rustup`. Do not treat a Linux build as Windows validation.
