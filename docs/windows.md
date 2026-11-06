# Windows

> Paths under `src/` below (`src/core`, `src/server`, `src/web`, `src/mobile`, `src/app`) name the Bend JS tree, removed from this checkout (tag `main-bend-final`). Where the current Rust code for each is, is unverified; look under `native/host/src/` and `native/gpui/`.

> The text that used to be here described the Electron app over `backplane-serve.exe`, built with `scripts/build-js.sh`, released with `scripts/service-release.py` and set up with `scripts/build-windows-helpers.sh`, `scripts/install-windows-providers.ps1` and `scripts/setup-windows.ps1`. None of those scripts or `desktop/` exist in this checkout any more, so those steps are gone.

What the current files confirm:

- The hub is `native/host` (`backplane-hub`) and the window is `native/gpui` (`backplane-gpui`), both Rust. The release workflow (`.github/workflows/release.yml`) builds them with `cargo build --release` for `linux-x64`, `linux-arm64` and `darwin-arm64` only. **There is no Windows target in it.**
- `scripts/dev-build.sh` builds the same two crates and publishes them with `scripts/dev-cut.sh`, which accepts ELF programs only (it checks for the `\x7fELF` header), so it does not publish a Windows build.

Unverified, needs a real Windows build: whether `cargo build --release` in `native/host` and `native/gpui` succeeds on Windows, how a Windows package would be made, and whether provider CLIs, the shell tools the hub runs, KiCad and CAD (windows-cad.md) work there. Do not treat a Linux build as Windows validation.
