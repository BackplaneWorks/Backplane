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
- The vault's master key is kept in the Credential Manager (`backplane-vault/<id>`).
- Paths: `\\?\` is dropped from resolved paths, a path is absolute as `C:\`, `C:/`, `\\host` or `/`, a relative one may not climb through `\` or name a drive, agent paths are recognised as `C:\x`, `C:/x` or `/c/x`.
- Ctrl+C, Ctrl+Break, closing the console, logoff and shutdown end the hub (exit 130).
- The client keeps its settings in `%APPDATA%\backplane-gpui`, its default hub home is `%APPDATA%\Backplane\hub`, its home is `%USERPROFILE%` when `HOME` is not set.

## Building on Linux for a Windows machine

```sh
rustup target add x86_64-pc-windows-gnu x86_64-pc-windows-gnullvm
# llvm-mingw (github.com/mstorsjo/llvm-mingw, ucrt-ubuntu-*-x86_64) unpacked into ~/.local/opt, or WIN_LLVM_MINGW=<dir>
scripts/windows-check.sh            # cargo check --target x86_64-pc-windows-gnu --all-targets, both crates
scripts/windows-check.sh --build    # build/windows: backplane-hub.exe, bp-sh.exe, backplane-gpui.exe, libunwind.dll (debug)
```

Copy `build/windows/` to the Windows machine (over the tailnet) and keep the four files together. These are debug builds linked for `x86_64-pc-windows-gnullvm` (llvm-mingw has no libgcc, which the `-gnu` target links). A client built this way compiles its shaders when its window opens (gpui compiles HLSL at build time only on a Windows host, and only in release) and has no embedded manifest; a release client comes from the release workflow or a build on Windows.

## Not done yet

- The updater picks archives by `.tar.gz`; a Windows install does not update itself from the zip yet.
- KiCad on Windows: the IPC transport and socket discovery are written and unit-tested, not run against a real KiCad. KiCad 10 export and the release scripts take POSIX paths (their tests are skipped).
- Tests skipped on Windows, each group with why: `scripts/ci-hub-windows-filter.sh`. "Debt" groups are to be ported; the others are Unix by design (Git sh's own pids and signals, symlinks and hard links, mode bits).
- CEF (the browser tab) is not packaged for Windows.
