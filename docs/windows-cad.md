# Windows CAD tools

What the Rust hub does for KiCad and FreeCAD on Windows, and what is proven (see `docs/windows.md` for the port as a whole).

- KiCad's API: the hub speaks KiCad's IPC itself (pure-Rust nng and protobuf, `native/host/src/kicad/`). nng's `ipc://<path>` is the named pipe `\\.\pipe\<path>` on Windows (`plat::ipc`), and a server's socket is found by listing the pipe namespace (`kicad/kipc.rs`). Unit-tested on the Windows runner against a stand-in server; not yet run against a real KiCad.
- Set `BACKPLANE_KICAD` (or Settings → CAD) to the Backplane KiCad runtime root or its `bin\kicad-cli.exe`; a stock KiCad does not provide all of Backplane's IPC capabilities.
- The hub's scripts around kicad-cli run in Git for Windows' sh (`docs/windows.md`). KiCad 10 export (`files/export10.rs`) and the release scripts (`files/rv.rs`) still take POSIX paths: their tests are skipped on Windows (`scripts/ci-hub-windows-filter.sh`), so treat export and release as not working there yet.
- FreeCAD: not run on Windows.

The 0.14 text that was here described the Bun hub, a portable Cygwin shell and `scripts/build-windows-helpers.sh`; none of those exist in this tree.
