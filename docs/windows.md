# Windows

The hub (`native/host`, `backplane-hub.exe`) and the GPUI client (`native/gpui`, `backplane-gpui.exe`) build and run on Windows x64. They are proven on GitHub's hosted `windows-latest` runners, not yet on a machine of our own. What has run there is in `docs/e2e/MATRIX.md` (Windows).

## What runs where

- CI (`.github/workflows/ci.yml`), blocking: `hub windows-latest 1/2` and `2/2` (the hub's tests, less the named skips in `scripts/ci-hub-windows-filter.sh`), `window windows-latest` (the client's tests), `guard windows-latest`, and `windows smoke` (`test/windows/smoke.sh`: a hub on a temp home, the client connected to it on the runner's desktop over WARP, the hub's data drawn, both end cleanly).
- Release (`.github/workflows/release.yml`): `windows-x64` builds on `windows-latest` and ships `backplane-<tag>-windows-x64.zip` (the hub, the client, the guard `backplane.exe`, `bp-sh.exe`, `assets/`, `COMMIT`). Its sha256 is in the signed `manifest.json` with the other archives. The client is a release build: gpui's HLSL shaders are compiled there by the Windows SDK's `fxc.exe`, its manifest by `rc.exe`.
- `windows-debug` (by hand): chosen hub or client tests on a Windows runner with their output.

## Needs on the Windows machine

- Git for Windows. The hub's scripts are POSIX shell (git, folders, exports); `sh` and `bash` mean Git's (on PATH, or beside `git.exe`, or under Program Files). Agents on Windows need it anyway.
- `bp-sh.exe` beside `backplane-hub.exe`. A Cygwin/MSYS program started by a Windows program splits its command line by its own rules, so every `sh` goes through `bp-sh`, which quotes for them (`src/bin/bp-sh.rs`).

## How the hub differs on Windows

All of it is in `native/host/src/plat/` (`unix.rs` is the calls the hub always made; `windows.rs` the nearest Windows meaning):

- Process groups are Job Objects named per leader (`Local\backplane-group-<pid>`); an agent's and a terminal's die with the hub (kill on close). Every signal ends the group: Windows has no SIGTERM.
- Terminals are ConPTY (`term/pty_windows.rs`); a terminal's key (`x:t1:...`) is saved as a file name with `%XX` for what Windows refuses.
- KiCad's nng `ipc://<path>` socket is the named pipe `\\.\pipe\<path>`.
- A `#!` script runs through the interpreter its first line names.
- Updates: a Windows install takes `backplane-<tag>-windows-x64-native.zip` (`rel::archive_name`), checks it like a tar.gz (size, sha256 from the signed manifest, every member listed and inside its folder before a byte is written), unpacks it with Windows' own `%SystemRoot%\\System32\\tar.exe` (bsdtar: Git's GNU tar, often first on PATH, reads no zip) and needs `backplane-hub.exe` at its top.
- The vault's master key is kept in the Credential Manager (`backplane-vault/<id>`).
- Paths: `\\?\` is dropped from resolved paths, a path is absolute as `C:\`, `C:/`, `\\host` or `/`, a relative one may not climb through `\` or name a drive, agent paths are recognised as `C:\x`, `C:/x` or `/c/x`.
- Ctrl+C, Ctrl+Break, closing the console, logoff and shutdown end the hub (exit 130).
- The client keeps its settings in `%APPDATA%\backplane-gpui`, its default hub home is `%APPDATA%\Backplane\hub`, its home is `%USERPROFILE%` when `HOME` is not set.

## Building on Linux for a Windows machine

The main path: the release target, `x86_64-pc-windows-msvc`, through cargo-xwin (Microsoft's CRT and SDK, fetched once into `~/.cache/cargo-xwin`; the licence is accepted).

```sh
rustup target add x86_64-pc-windows-msvc; cargo install --locked cargo-xwin
scripts/windows-check.sh --xwin [DIR]   # DIR (default build/windows): backplane-hub.exe, bp-sh.exe, backplane.exe (guard), backplane-gpui.exe, COMMIT; PROFILE (default fast)
```

It needs `clang-cl` and `llvm-rc` (an LLVM bin folder, default the newest `/usr/lib/llvm-*/bin`, or `WIN_LLVM`) and `lld-link` (on PATH, or llvm-mingw's). Copy the folder with `assets/` (and the generated `icons.json` and `cat-rigs/`) to the Windows machine.

The client draws with shader bytes compiled on Windows: gpui compiles its HLSL only when the build host is Windows, so `native/gpui/vendor/gpui` (gpui 0.2.2, `PATCH.md`) embeds `windows-shaders/shaders_bytes.rs` in a cross-built client without debug assertions, and refuses it if any `src/platform/windows/*.hlsl` changed since. `scripts/windows-shaders.sh HOST` remakes it on a Windows machine with the SDK (fxc.exe, gpui's own arguments). The manifest is embedded by llvm-rc. A build on Windows (CI, release.yml) still compiles the shaders itself.

The older path, debug builds for `x86_64-pc-windows-gnullvm` with llvm-mingw:

```sh
rustup target add x86_64-pc-windows-gnu x86_64-pc-windows-gnullvm
# llvm-mingw (github.com/mstorsjo/llvm-mingw, ucrt-ubuntu-*-x86_64) unpacked into ~/.local/opt, or WIN_LLVM_MINGW=<dir>
scripts/windows-check.sh            # cargo check --target x86_64-pc-windows-gnu --all-targets, both crates
scripts/windows-check.sh --build    # build/windows: backplane-hub.exe, bp-sh.exe, backplane-gpui.exe, libunwind.dll (debug)
```

A debug client compiles its shaders at run time from the path of gpui's sources on the build machine, which a Windows machine does not have; use `--xwin`.

## Testing on a Windows machine of our own

`test/windows/rig/` (no machine names in it; pass the ssh host): `autologon.ps1` (the password on stdin, kept as the LSA secret, never in the registry), `sesstask.ps1` (a logon task, "only when the user is logged on", that runs `sessagent.ps1` in the desktop session), `sess.sh HOST JOB.ps1` (runs a job in that session, where windows open; ssh's session 0 shows none), `up.ps1`/`down.ps1` (a hub on its own home and the client on its own settings), `shot.ps1` (a screenshot of the session).

## Not done yet

- KiCad on Windows: the IPC transport and socket discovery are written and unit-tested, not run against a real KiCad. KiCad 10 export and the release scripts take POSIX paths (their tests are skipped).
- Tests skipped on Windows, each group with why: `scripts/ci-hub-windows-filter.sh`. "Debt" groups are to be ported; the others are Unix by design (Git sh's own pids and signals, symlinks and hard links, mode bits).
- CEF (the browser tab) is not packaged for Windows.
