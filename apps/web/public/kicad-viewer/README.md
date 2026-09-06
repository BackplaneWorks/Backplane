# KiCad viewer vendor bundle

This directory contains the browser viewer artifacts used by KiCad Prism,
vendored from the local checkout of [KiCAD-Prism](https://github.com/krishna-swaroop/KiCAD-Prism)
at commit `50a4812c7c05aebcc8dda28c64c9382c9fce719d`. The ECAD bundle manifest separately records its own upstream and adapter commits.

The `ecad-viewer.js` and `parser.worker.js` artifacts provide the upstream
native KiCad schematic and PCB viewers. `3d-viewer.js`, Three.js, and its
add-ons provide the upstream 3D viewer loaded by the ECAD element. These files
are read-only browser assets: the viewer receives snapshots of file contents
and never opens a KiCad source file for writing or locking.

KiCAD-Prism is Apache-2.0 licensed; its bundled third-party notices remain in
the generated artifacts. Keep this attribution with the vendored files when
updating them from upstream.

The Gerber camera hook at `src/kicad/vendor/fabrication-viewport.ts` and server renderer at `apps/server/src/kicad/vendor/prismGerber.ts` come from that same Prism commit. The renderer is embedded verbatim for server packaging. `runtime.html`, `runtime.js`, `model-appearance.js`, and `schematic-sizing.js` are k3eda adapters, not upstream viewer code. The sizing adapter supplies high-density canvas backing and reserves space for the schematic properties panel without modifying the vendor bundle.

The model appearance adapter adds a resin clearcoat to exported soldermask and tunes Prism's existing lighting. Color and mask opacity come from KiCad's GLB export, not a fixed viewer palette.
