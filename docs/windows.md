# Windows

The hub (`native/host`, `backplane-hub.exe`) and the GPUI client (`native/gpui`, `backplane-gpui.exe`) build and run on Windows x64. They are proven on GitHub's hosted `windows-latest` runners, not yet on a machine of our own. What has run there is in `docs/e2e/MATRIX.md` (Windows).

## What runs where

- CI (`.github/workflows/ci.yml`), blocking: `hub windows-latest 1/2` and `2/2` (the hub's tests, less the named skips in `scripts/ci-hub-windows-filter.sh`), `window windows-latest` (the client's tests), `guard windows-latest`, and `windows smoke` (`test/windows/smoke.sh`: a hub on a temp home, the client connected to it on the runner's desktop over WARP, the hub's data drawn, both end cleanly).
- Release (`.github/workflows/release.yml`): `windows-x64` builds on `windows-latest` and ships `backplane-<tag>-windows-x64.zip` (the hub, the client, the guard `backplane.exe`, `bp-sh.exe`, `assets/`, `COMMIT`). Its sha256 is in the signed `manifest.json` with the other archives. The client is a release build: gpui's HLSL shaders are compiled there by the Windows SDK's `fxc.exe`, its manifest by `rc.exe`.
- `windows-debug` (by hand): chosen hub or client tests on a Windows runner with their output.

## Needs on the Windows machine

- Git for Windows. The hub's scripts are POSIX shell (git, folders, exports); `sh` and `bash` mean Git's (on PATH, or beside `git.exe`, or under Program Files; or beside a `git.exe` on the registry's Path, so a Git installed while the hub runs is found without a restart). A shell not found is looked for again at the next `sh`; until then bp-sh says Git for Windows is missing. Agents on Windows need it anyway.
- The hub's own helpers (`bp-sh`, `backplane-voice`, `backplane-step2glb`, `backplane-browser`) are started by full path from the hub's folder (`plat::helper`), never by name from PATH: an install's `builds\<tag>\` is not on PATH.
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
- The person's folders (`plat/dirs.rs`): HOME is usually unset, so the home is `%USERPROFILE%`; a POSIX value (Git's shell's `HOME=/c/...` or `/tmp`) is ignored, because the hub reads `/tmp` as `C:\tmp` and Git's shell as `%TEMP%` (issue 1102). Converted 3D models are cached in `%LOCALAPPDATA%\Backplane\cache\models` (`XDG_CACHE_HOME` still wins when it is a Windows path), and KiCad's model variables are read from its own profile, `%APPDATA%\kicad\<version>\kicad_common.json` (the running KiCad's version first). Converters run directly, never through `sh`, so they write where the hub looks.
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

## The embedded KiCad pane: the input shield

KiCad's editor is a child window of another process inside Backplane's window. While Backplane has the keys, a
transparent shield window (`native/gpui/src/viewers/native_child.rs`, `shield_proc`) lies over the pane, so a click
there is Backplane's own and Windows never loses the foreground to the other process (F3).

- A press on the shield hands KiCad the keys and is replayed into KiCad once its shown editor really has the focus
  (the system foreground is Backplane's window and the focus is inside the editor, two ticks running; at most 600 ms).
  The shield holds mouse capture until then, so a release that comes first reaches it, not KiCad, and the replay is a
  whole click; a press still held is replayed as its down and the person's moves and release follow.
- The press is posted, not injected: nothing feeds the system's key state, so the person's button state is theirs.
- Wheel and hover over the pane are passed on to KiCad's canvas.

Known limit: a middle-button drag that starts on the pane while Backplane has the keys pans only sometimes; the press
hands the keys to KiCad either way, and a drag once KiCad has them pans as usual. Measured on bp-win (Nov 15), ten
drags each: 4 of 10 pan at a 300 ms hold, 0 of 10 at 1 s. Right after the posted middle-down KiCad takes the mouse
capture for a different window than the one the down reached, and no move goes through the shield. Two changes were
tried and did not help, so neither shipped: forwarding every move and the release from the shield (KiCad took capture
before the first move, every time), and injecting a real middle-down through the input queue (about the same rate,
and left and right clicks at 120 ms holds then failed). On main before the shield did this (stage 1) such a drag never
panned: the shield consumed every first press. The injected variant also made the left and right click sweep worse, so
it is not to be tried again; a fix has to come from how KiCad takes capture after a posted press.

Seen once, not reproduced: the `took` rule in `kicad_pane.rs` (an X11 workaround: a window manager that focuses on
click hands the keyboard straight back to Backplane after a click in KiCad, so a focus notice within 600 ms pulls it
into KiCad again) also runs on Windows. In one run (bp-win, Nov 15, stage-2 build 3afc3f375 on bp.10, the click-acts
check, round "composer 2/10") a click into the composer handed the keys back to Backplane, the rule then focused
KiCad's frame in the same tick, the composer's mouse-down-out handed them back again, and at the next click on the
board KiCad still had the focus: no shield click, nothing selected. A race test that waits for KiCad's focus notice
before the composer click (`took_race`) arms in 1 of 5 rounds at best and showed nothing on the landed build or with
the fix, so the fix is not on main. It is on the branch `took-main` (`took_applies`: never on Windows, never after a
button went down in Backplane's own window; unit test
`took_never_pulls_the_keys_back_on_windows_or_after_a_click_into_backplane`); land it once a race that arms shows
the typing going to KiCad on the landed build and to the composer with the fix.
