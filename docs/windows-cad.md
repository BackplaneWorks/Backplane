# Windows CAD tools

Backplane runs as a Cygwin Windows executable. KiCad, FreeCAD and the Bun
helpers run as native Windows programs. Filesystem paths cross that boundary
explicitly; message text is never rewritten globally.

Build the two native helpers from Linux with Bun 1.4.3 or newer:

```sh
BUN=/path/to/bun scripts/build-windows-helpers.sh
```

`build/windows-helpers/` contains `backplane-step2glb.exe`,
`backplane-kipc.exe`, dependency licenses and a checksum manifest. Keep both
helpers beside `backplane.exe`, and ship the license directory with them.
The KiCad helper loads the selected runtime's `nng.dll` from its `bin` or
`lib` directory and uses that runtime's descriptor set. A separate system
NNG installation is unnecessary.

Set `BACKPLANE_KICAD` to the Backplane KiCad runtime root or its
`bin/kicad-cli.exe`; a stock KiCad installation does not provide all of
Backplane's IPC capabilities. The runtime's own manifest and handshake
remain authoritative. Windows runtime builds are prepared in the public
Backplane_KiCad repository; validate a downloaded package before treating
that platform as supported.

Set `BACKPLANE_FREECAD` to the installed `FreeCADCmd.exe`. For a portable
FreeCAD archive, select the actual `bin/freecadcmd.exe`. The GUI executable
must be beside it. Some bundles also provide root-level Chocolatey shims;
the STEP helper prefers the real GUI below `bin` because the shim can return
before its child starts. Native Windows
paths and Cygwin paths are accepted for this setting. Restart Backplane after
changing its environment.

From Cygwin, native CAD programs need native filesystem arguments. Use
`cygpath -w` for absolute script/file paths passed directly to FreeCAD. A
relative `mech/part.py` beside the current project works with both runtimes.
Within a native FreeCAD script, use Python's pathlib and native paths; do not
hard-code `/cygdrive` paths. Backplane's STEP helper performs path conversion
for its own input/output arguments and preserves colors through the GUI
FreeCAD fallback.

Native checks include `test/native/native_pid.bend` and
`test/native/windows_paths.bend`. Emit them with `BACKPLANE_EMIT_ONLY=1`
through `scripts/build-app.sh`, then compile their C on Windows with
`scripts/build-windows.sh`. Require the printed path checks to pass: the
Check helper reports failures in its output. Compare the PID test to the
actual native process ID, not a Cygwin PID.

`test/fixtures/windows-cad/mech/native_box.py` is a disposable real FreeCAD
GUI fixture. Run it with `QT_QPA_PLATFORM=offscreen`; it validates geometry,
FCStd save/reopen, colored STEP export/reimport and volume. Its generated
files belong in an isolated acceptance directory. Test the resulting STEP
with the native helper and `backplane.exe --render ...`, and inspect the
actual Windows PNGs. Linux output does not establish Windows acceptance.

Run the compiled helper's Windows Worker/error-path check with Cygwin Python:

```sh
python3 test/tools/windows-kipc.py /path/to/backplane-kipc.exe
```

This checks the real bundled Worker and a missing-runtime error; it does not
claim a working KiCad server. After creating the orange FreeCAD STEP fixture,
exercise the GUI fallback and process/pipe completion with:

```sh
BACKPLANE_FREECAD='C:\FreeCAD\FreeCADCmd.exe' python3 test/tools/windows-freecad.py /path/to/backplane-step2glb.exe /path/to/native_box.step
```

The test pads a disposable copy above the native fallback threshold and
requires the actual FreeCAD GLB generator and the original orange material.

The Windows CI emit job builds these helpers with pinned Bun 1.4.3 and uploads
their checksum manifest alongside the portable C. The Windows job verifies
and stages them, tests the actual compiled KiCad Worker, and includes both
executables, dependency licenses and WINDOWS-HELPERS.json in the app ZIP.
FreeCAD and the Backplane KiCad runtime are installed separately.
