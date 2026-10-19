# Backplane Flatpak

This is an initial, locally distributable Flatpak package, not a Flathub
submission. It packages the normal native Linux build, web client and
helpers against the Freedesktop 25.08 runtime. The app runs on that runtime,
so its libc is independent of Ubuntu's host libc. Build on the architecture
you intend to package.

Install `flatpak` and `flatpak-builder`, then:

```sh
flatpak remote-add --user --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
flatpak install --user flathub org.freedesktop.Platform//25.08 org.freedesktop.Sdk//25.08
scripts/build.sh
scripts/package.sh v0.0.0-test linux-x64
python3 scripts/package-flatpak.py v0.0.0-test linux-x64
flatpak install --user dist/backplane-v0.0.0-test-linux-x64.flatpak
flatpak run io.github.i2cjak.Backplane
```

For arm64 use `linux-arm64`. `--manifest-only` writes the generated manifest
without invoking the builder. CI can package an already built artifact;
it does not need to compile Bend again inside the SDK. For a focused CI
package check, dispatch the workflow with `native_run` set to a previous
CI run containing the `backplane-linux-x64` artifact. The package test runs
on Ubuntu 22.04 and exports a board through the actual nested KiCad Flatpak.

The native Backplane process and its bundled helpers run inside the
sandbox. Host tools (`claude`, `codex`, `git`, `gh`, `tailscale`, `pdftoppm`,
`freecadcmd`, `FreeCADCmd`) run through `flatpak-spawn --host`, using the
host's login environment, libraries and sign-in state. Only tools found on
the host are advertised. The host must have Bash; the launcher includes
common local tool paths and nvm. Other provider tools and browser/CEF
integration need further qualification in this initial package.

Install KiCad separately with `flatpak install --user flathub org.kicad.KiCad`.
The packaged `kicad-cli` adapter starts that Flatpak through the host service
with the project/cache files visible in both environments. Stock KiCad 10
provides the CLI fallback; it cannot provide Backplane's fork-only headless
IPC editing. Select `/app/bin/kicad-cli` in KiCad settings if an existing
saved path overrides the environment default.

The package deliberately grants host-file and temporary-directory access,
network, X11 and access to `org.freedesktop.Flatpak`. Hardware projects,
exports and agent subprocesses need these permissions. Host command access
also lets agents run outside the sandbox: this package is **not** an agent
security boundary. It never modifies KiCad's persistent overrides or links
against its private Flatpak libraries.

The default Backplane home and its one-hub lock remain in use. Close a
native Backplane instance before opening the Flatpak on the same home,
or pass a different `--home`. The in-app updater is disabled; update by
installing a newly built Flatpak bundle.
