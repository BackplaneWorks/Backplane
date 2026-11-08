# Installation

## Linux

```sh
curl -fsSL https://github.com/BackplaneWorks/Backplane/releases/latest/download/install.sh | sh
backplane-app
```

x64 and arm64. It lays the release out for the guard (docs/deploy.md): the build in `~/.local/share/backplane/builds/<tag>/`, `CURRENT` naming the one that runs, and the guard at `~/.local/share/backplane/backplane`, linked as `backplane` in `~/.local/bin` beside `backplane-app` (the desktop app). It adds Backplane to your applications menu (`BACKPLANE_NO_DESKTOP=1` skips that). The desktop app starts its home's hub under the guard, which keeps it up to date from signed releases, takes each new build on probation and goes back when one fails (`backplane hub status`, `restart`, `rollback`). `BACKPLANE_NO_DESKTOP_APP=1` installs the hub alone and starts the guard at once (home `BACKPLANE_HOME`, default `~/.backplane/hub`; port `BACKPLANE_PORT`, default the first free from 3800); `BACKPLANE_START=1` starts it with the desktop app too. Run the installer again while the guard runs and it hands the new build to the guard, on probation. Add `~/.local/bin` to your `PATH` if asked.

## macOS (Apple silicon)

Run the install.sh one-liner above: the same layout, with the default home in `~/Library/Application Support/Backplane/hub`. `BACKPLANE_NO_DESKTOP_APP=1` installs only the hub.

**macOS builds are not notarized yet** (notarization will follow once an Apple Developer ID exists). If macOS refuses to open the downloaded app, once only: open it from Finder with right-click (or Control-click) → Open → Open; or on macOS 15+, try to open it, then System Settings → Privacy & Security → "Open Anyway"; or run `xattr -dr com.apple.quarantine /Applications/Backplane.app`. The installer checks the release checksum and signatures, and clears quarantine only on the payload it downloaded.

## Windows (x64, experimental)

Download `backplane-<tag>-windows-x64-native.zip`, extract the **whole** archive into a folder, and run `backplane-gpui.exe`. The exe is not signed yet, so Windows SmartScreen warns the first time (see below). The first run asks **Install Backplane to %LOCALAPPDATA%\Backplane? (needed for updates)**: Install copies the release to `%LOCALAPPDATA%\Backplane\builds\<tag>`, puts the guard (`backplane.exe`, which updates, restarts and rolls back the hub) at `%LOCALAPPDATA%\Backplane`, adds **Backplane** to the Start menu and starts from there; the folder you extracted can then be deleted. Not now runs it from the extracted folder, without updates, and asks again next time. Git for Windows is needed (the hub's shell commands use its `sh`); install your agent CLIs, KiCad and FreeCAD separately (Settings → CAD finds them). Closing the window leaves the hub running.

To remove Backplane: quit it and stop the hub (`%LOCALAPPDATA%\Backplane\backplane.exe hub stop`), then delete the folder `%LOCALAPPDATA%\Backplane` and the Start-menu entry `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Backplane.lnk`. Your projects, threads and settings stay in the hub's home, `%APPDATA%\Backplane\hub` (and the window's settings in `%APPDATA%\backplane-gpui`); delete those too to remove everything.

## From 0.14

On Linux and macOS, press Update in 0.14 as usual: 0.15 installs beside it, starts on the same port and offers to import 0.14's projects, threads, bots and paired phones (docs/deploy.md, "From 0.14"). Link your other machines again in Settings, Devices. If 0.15 fails to start, 0.14 comes back by itself. On Windows, 0.14 cannot update itself: quit it, download `backplane-<tag>-windows-x64-native.zip` from the release, extract it and run `backplane-gpui.exe` (Install, as above); its setup offers the same import.

## What each platform's archive carries

Every archive holds the hub, the desktop client, the guard, `backplane-voice` (dictation), `backplane-browser` (the Browser tab's helper), `backplane-step2glb` (STEP to GLB without FreeCAD) and `assets/` with the generated icons and cat rigs; `release.yml` fails a build that misses any. On Linux the client also has the embedded Chromium (feature `cef`) with its runtime beside it (`libcef.so`, the `.pak` files, `icudtl.dat`, `v8_context_snapshot.bin`, `locales/`). On macOS and Windows the client is built without it: the Browser tab shows the page as the hub's helper draws it (a frame stream), not an embedded browser.

## Phones

- **iOS:** install through TestFlight with an invitation.
- **Android:** download the signed APK from the release and install it.

To pair a phone or another browser, open **Settings → Devices → Add a device** and use the pairing link, the QR code or the short code. The same menu lists paired devices and revokes them.

## First run

You need at least one agent CLI (`claude`, `codex`, `grok` or `muse`; Antigravity installs from Settings), KiCad for board work and FreeCAD for mechanical work. The first run asks about anonymous usage counts in a clear popup. They are on by default, nothing is sent before you answer, and Settings turns them off at any time. See [telemetry](telemetry.md).

Treat pairing links and QR codes as credentials. Browser pairing exchanges the fragment for an HttpOnly, SameSite=Strict cookie (Secure over HTTPS), then removes the fragment; existing browser tokens migrate out of local storage after a successful exchange. Native phone clients retain their pairing-link protocol. Every client needs the pairing credential, on this machine too: the desktop app reads the hub's token from its home (`<home>/token`, readable only by your user), and a browser on this machine pairs like any other (a pairing link from Settings, or `backplane` opening it on an install without the desktop app). Full Access runs agent tools with your user’s access; choose a more restrictive thread permission mode when wanted. Pairing controls access to the hub, and does not provide OS isolation for a Full Access agent.

The hub preserves the existing `<home>/token` file byte for byte, including a symlink to a regular file. As before, surrounding whitespace is ignored for pairing. Empty, unreadable or non-regular token files stop startup with an error instead of generating a replacement. Restore the original token from backup to retain paired clients. On Unix the opened file is restricted to mode 0600; Windows access control comes from the home folder's ACL; POSIX mode bits do not establish a private Windows ACL.

A missing token is created exclusively at mode 0600, without requiring hard links. An interrupted creation keeps `token.backplane-creating` and refuses startup. Preserve both files and recover the original credential before resolving that marker; do not remove an existing token to silence the error. The normal home lock serializes startup. Filesystems must support exclusive creation and file flushing; Unix directory flushing is checked too. Windows power-loss durability, native ACL behavior and network filesystem guarantees require platform qualification.

To uninstall what install.sh put in place, stop the guard (`backplane hub stop`), then:

For a custom installation, substitute your `BACKPLANE_PREFIX`,
`BACKPLANE_BIN` and `XDG_DATA_HOME` paths in the commands below.

```sh
rm -rf ~/.local/share/backplane
rm -f ~/.local/bin/backplane ~/.local/bin/backplane-app
rm -f ~/.local/share/applications/backplane.desktop \
  ~/.local/share/icons/hicolor/scalable/apps/backplane-desktop.svg
```

Your threads, settings and keys stay in the home (`~/.backplane/hub`, or `~/Library/Application Support/Backplane/hub` on macOS); remove it too to start over. A package install (`.deb`, AUR) uninstalls through its package manager.

Nightly builds of `main` are prereleases on the [Releases](https://github.com/BackplaneWorks/Backplane/releases) page, each with its own installer: `curl -fsSL https://github.com/BackplaneWorks/Backplane/releases/download/<tag>/install.sh | sh`.
