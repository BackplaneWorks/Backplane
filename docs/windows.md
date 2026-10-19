# Windows

Backplane's Windows target is a 64-bit Windows executable with a Win32 window.
It uses Cygwin's POSIX runtime for Bend's scheduler, files, sockets and child
processes. It does not use WSL, a Linux kernel, an X server or a browser window.
This target has been validated in a Windows 11 Pro QEMU/KVM guest.

CI builds a Windows zip on every pull request and push to main. Download the
`backplane-windows-x64` artifact from the GitHub Actions CI run, extract the
whole zip, run `Install-Runtime.ps1` once, then open `Backplane.cmd`. The
installer needs no administrator rights and puts Cygwin under
`%LOCALAPPDATA%\Backplane\cygwin64`; an existing `C:\cygwin64` can also be used.
Keep the executable, launchers and `web` directory together. The zip has a
SHA256 sidecar and `BUILD.json` records the source commit and file hashes.
Normal release/nightly builds attach the Windows zip beside the other assets.

The reusable `.github/workflows/windows.yml` workflow emits C with the pinned
Bend compiler on Linux, then compiles and tests the PE on `windows-2022`. It
checks runtime Unicode and randomness, child exits, binary duplex stdio/EOF,
lock handover, CLI validation, `/hello` and duplicate-hub exclusion in a
temporary profile. It also installs the real native Claude and Codex CLIs and
checks discovery and `--version` through Backplane's native process transport.
These checks do not authenticate or run a model turn. The installed CLI
versions are printed in the build log. Provider login and an authenticated
turn require a separate manual check; Codex has worked in the Windows guest
after the user logged in. Automated CI does not claim desktop input/rendering
or authenticated provider coverage. The native window and an offline provider
tool roundtrip were verified separately in the guest.

The package contains Backplane and its web files; third-party Cygwin binaries
and provider CLIs are installed separately. To reproduce the CI build, run
`scripts/emit-windows.sh` on a pinned-Bend builder, copy
`build/windows-emitted` and `dist/web` with the source to Windows, install
`scripts/setup-windows.ps1 -BuildTools` and
`scripts/install-windows-providers.ps1`, then run `scripts/ci-windows.sh` from
the Cygwin login shell. `BACKPLANE_VERSION` supplies an optional package version.

Install the x86-64 Cygwin distribution from https://cygwin.com/ with `clang`
(19 or later), `python3`, `git`, `make`, `libfreetype6`, `fontconfig` and
`libfontconfig1`. The current guest uses Cygwin 3.6.11 and Clang 22.1.8.
Run the build in Cygwin's login shell so `/usr/bin` is on PATH.

The pinned Bend 2.0.36 compiler currently ships Linux/macOS binaries. Generate its
platform-neutral C on a builder with that compiler, using the normal resource
admission wrapper on a shared workstation:

```sh
BACKPLANE_EMIT_ONLY=1 BACKPLANE_JOBS=2 BP_BUILD_LOCKED=1 \
  scripts/build-app.sh src/app/main.bend build/backplane-windows.exe
```

Copy `build/cc/backplane-windows.exe/all.c` and the repository to Windows, then
compile with the Windows toolchain:

```sh
BACKPLANE_JOBS=1 scripts/build-windows.sh all.c build/backplane.exe
```

The default is one compiler process. `BACKPLANE_UNITS` controls the number of
translation units (16 by default; use 32 on a small VM). The result is a Windows
PE executable. Run it with Cygwin's `bin` directory on PATH and a HOME directory
for the Windows user. Development and test runs must supply a dedicated home:

```sh
BACKPLANE_NO_UPDATE=1 BACKPLANE_PEERS='' \
  build/backplane.exe --home /home/Backplane/backplane-test
```

`build-windows.sh` prepares the emitted C with `windows-runtime.py`. The Windows
x64 ABI passes Bend's two-word `Env` indirectly; the pinned runtime's tail calls
lose that state on this target. Internal functions marked `PRESERVE` and their
matching function pointers use an explicit SysV ABI on Windows. Calls into
Cygwin, Win32 and foreign effects retain their normal Windows ABI. The Windows
preparation also guards Base's Linux syscall header and selects Windows system
randomness through BCryptGenRandom. It checks the expected
pinned runtime shape and fails if that shape changes.

The window owns its Win32 message loop on a separate thread and wakes Bend
through a nonblocking pipe. GDI presents the same Bend image quadtree as Linux;
input keeps the shared key/event contract. Clipboard text uses the Unicode
Windows clipboard and file drops become escaped `file://` URLs for Cygwin
paths. Fontconfig and FreeType load their Windows DLL names. Existing Linux
implementations are unchanged.

Cygwin does not provide `waitid(WNOWAIT)`. The Windows child-status observer
uses Cygwin's queued `SA_SIGINFO` notifications to distinguish normal exit
from signal termination for both Cygwin and plain Windows executables.
Observing an exit does not reap it; `Proc.wait` still calls `waitpid`.
The observer uses lock-free atomic stores in its signal handler.

Plain Win32 processes cannot use Cygwin's Unix socketpair as standard input
and output. The Windows spawn path gives them ordinary pipe handles; one
poll-driven worker bridges those pipes to the hub's duplex socket, with
16 KiB buffers in each direction. It preserves backpressure and EOF and
does not wait for or signal processes. `test/tools/windows-stdio.c` checks
real `cmd.exe` output and a 512 KiB binary roundtrip through native
PowerShell, including EOF:

```sh
clang -std=c11 -O2 test/tools/windows-stdio.c \
  -o build/windows-stdio.exe -lpthread
build/windows-stdio.exe
```

Run its native Windows regression test in Cygwin:

```sh
clang -std=c11 -D___wchar_t=__WCHAR_TYPE__ -O2 \
  test/tools/windows-child.c -o build/windows-child.exe
build/windows-child.exe
```

It checks normal exit 15 versus SIGTERM, exit-code truncation, repeated
observations before reaping, 48 concurrent completions and 100 fast exits.

The hub keeps its POSIX flock across restart. Cygwin's inherited descriptor
supports ordinary seek/write but its positional-I/O handle fails with `EBADF`
after exec, so the Windows metadata writer uses seek/write on that same locked
descriptor. The POSIX writer keeps `pwrite`. Test the real lock helpers,
including exact metadata, close-on-exec and competing-process exclusion:

```sh
clang -std=c11 -O2 -Wno-extra-tokens test/tools/windows-hublock.c \
  -o build/windows-hublock.exe
build/windows-hublock.exe
```

The initial Windows tests are `test/native/windows_runtime.bend` and
`test/native/windows_window.bend`. Compile their emitted C with the same script.
The first must print `WINDOWS_RUNTIME_OK 日本😀`, not merely exit zero. The second
must open a colored window, report `WINDOW_READY`, preserve Unicode clipboard
text, report Unicode/key/mouse/wheel/resize events and finish with
`WINDOW_CLOSED` when its real window is closed. Run UI automation in the same
Windows desktop session as the app; a WinRM service session cannot enumerate
an interactive user's windows.

Known initial scope: this backend handles committed text through `WM_CHAR`;
custom preedit payload injection and clipboard images are not implemented yet.
The embedded browser and other optional platform helpers require their own
Windows builds. Do not treat a successful Linux build as Windows validation.

The shared Google desktop client is supplied only to the native Windows
compile step through `BACKPLANE_GOOGLE_CLIENT_ID` and
`BACKPLANE_GOOGLE_CLIENT_SECRET`. `scripts/google-header.py` writes a mode0600
header beside the compile units; `build-windows.sh` includes it explicitly
and removes it on exit, alongside the separate telemetry header. Neither
header belongs in the portable C, shared emitted artifacts or ZIP. An absent
pair leaves the shared client empty; a partial or malformed pair fails.
`BACKPLANE_GOOGLE_BAKE=0` explicitly disables this bake. The native workflow
uses the same GitHub secret names as the Linux and Darwin release jobs.
