# Installation

```sh
curl -fsSL https://github.com/BackplaneWorks/Backplane/releases/latest/download/install.sh | sh
backplane
```

Linux (x64, arm64) and macOS (Apple silicon). It installs into `~/.local/share/backplane` and keeps itself up to date (`BACKPLANE_NO_UPDATE=1` turns that off). On Linux it also adds Backplane to your applications menu (`BACKPLANE_NO_DESKTOP=1` skips that). Releases also carry an AppImage, a `.deb`, an AUR `PKGBUILD` and the Android APK. You need at least one agent CLI (`claude`, `codex`, `grok` or `muse`; Antigravity installs from Settings) and KiCad; FreeCAD for mechanical work. `backplane --headless` runs it without a window. To pair a phone, scan the QR code in Settings (Pairing link, QR code) with the app, or paste the pairing link.

Treat pairing links and QR codes as credentials. Browser pairing exchanges the fragment for an HttpOnly, SameSite=Strict cookie (Secure over HTTPS), then removes the fragment; existing browser tokens migrate out of local storage after a successful exchange. Native phone clients retain their pairing-link protocol. Remote clients need the pairing credential; a browser cannot inherit localhost access through a foreign origin or rebound hostname. Local programs running as your user remain trusted. Full Access runs agent tools with your user’s access; choose a more restrictive thread permission mode when wanted. Pairing controls access to the hub, and does not provide OS isolation for a Full Access agent.

On Apple Silicon, the installer includes the macOS desktop app. Add `~/.local/bin` to your `PATH` if requested, then run `backplane`. `BACKPLANE_NO_DESKTOP_APP=1` installs only the hub. The installer checks the release checksum and code signatures before replacing files and clears quarantine only on that downloaded payload. Portable macOS apps are ad-hoc sealed, not notarized; a browser-extracted app may still require an explicit Gatekeeper override.

The hub preserves the existing `<home>/token` file byte for byte, including a symlink to a regular file. As before, surrounding whitespace is ignored for pairing. Empty, unreadable or non-regular token files stop startup with an error instead of generating a replacement. Restore the original token from backup to retain paired clients. On Unix the opened file is restricted to mode 0600; Windows access control comes from the home folder's ACL; POSIX mode bits do not establish a private Windows ACL.

A missing token is created exclusively at mode 0600, without requiring hard links. An interrupted creation keeps `token.backplane-creating` and refuses startup. Preserve both files and recover the original credential before resolving that marker; do not remove an existing token to silence the error. The normal home lock serializes startup. Filesystems must support exclusive creation and file flushing; Unix directory flushing is checked too. Windows power-loss durability, native ACL behavior and network filesystem guarantees require platform qualification.

To uninstall what install.sh put in place, quit Backplane, then:

For a custom installation, substitute your `BACKPLANE_PREFIX`,
`BACKPLANE_BIN` and `XDG_DATA_HOME` paths in the commands below.

```sh
rm -rf ~/.local/share/backplane
rm -f ~/.local/bin/backplane ~/.local/bin/backplane-serve
rm -f ~/.local/share/applications/backplane.desktop \
  ~/.local/share/icons/hicolor/scalable/apps/backplane-desktop.svg \
  ~/.local/share/icons/hicolor/256x256/apps/backplane-desktop.png
```

Your threads, settings and keys stay in `~/.backplane-bend` and the model cache in `~/.cache/backplane-bend`; remove them too to start over. A package install (`.deb`, AUR) uninstalls through its package manager.

Nightly builds of `main` are prereleases on the [Releases](https://github.com/i2cjak/Backplane/releases) page, each with its own installer: `curl -fsSL https://github.com/BackplaneWorks/Backplane/releases/download/<tag>/install.sh | sh`.
