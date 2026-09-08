# Backplane releases

`.github/workflows/backplane-release.yml` is the desktop publishing workflow
for this fork. The initial supported target is the Linux x64 AppImage. It
downloads a pinned stable Backplane KiCad runtime, verifies its SHA-256, stages
it under `resources/kicad`, builds the installer, verifies its bundled runtimes,
and creates a draft GitHub Release. Review the artifacts before publishing the draft.

Configure the KiCad repository variables from a release of `i2cjak/Backplane_KiCad`:

- `BACKPLANE_KICAD_RUNTIME_URL_LINUX_X64`
- `BACKPLANE_KICAD_RUNTIME_SHA256_LINUX_X64`

The Linux x64 Python pin is checked into
[`assets/runtime/python-linux-x64.json`](../../assets/runtime/python-linux-x64.json).
The workflow verifies the install-only archive checksum, then downloads the
matching full-build archive from that file solely to extract `PYTHON.json` and
all 19 files under `python/licenses/` into the packaged runtime's
`LICENSES/` directory. Keep both checksums and both URLs synchronized when
updating the pin.

The runtime archive must contain `bin/kicad-cli` and `manifest.json`. The
manifest must have non-empty `sourceRepository`, `sourceCommit`, `version`,
and `license` fields. The release build sets
`T3CAD_REQUIRE_BUNDLED_KICAD=1`, so a missing or unverified runtime fails the
build instead of falling back to a developer's system KiCad.

The packaged server receives the runtime directory through
`BACKPLANE_KICAD_ROOT`. A pinned Python-build-standalone archive is staged at
`resources/python` and passed as `BACKPLANE_PYTHON`, so Gerber rendering does
not depend on a user's Python installation. Developers can use
`BACKPLANE_KICAD_CLI`, `KICAD_CLI`, or `BACKPLANE_PYTHON` for local tools.

For a local Linux build, install the normal desktop prerequisites, then run:

```bash
T3CAD_KICAD_RUNTIME=/path/to/stable-kicad-runtime \
T3CAD_REQUIRE_BUNDLED_KICAD=1 \
T3CAD_PYTHON_RUNTIME=/path/to/python-runtime \
T3CAD_REQUIRE_BUNDLED_PYTHON=1 \
T3CODE_DESKTOP_UPDATE_REPOSITORY=i2cjak/Backplane \
./node_modules/.bin/vp run dist:desktop:artifact \
  --platform linux --target AppImage --arch x64 --build-version X.Y.Z
```

The runtime path above must contain `bin/kicad-cli` and the provenance
`manifest.json`. The resulting installer is written to `release/`.
