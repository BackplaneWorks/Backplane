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
FreeCAD archive, select the launcher at the archive root, which initializes
its bundled environment. The GUI launcher must be beside it. Native Windows
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
