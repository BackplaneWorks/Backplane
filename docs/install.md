# Installation

## Linux

```sh
{ curl -fsSL https://github.com/BackplaneWorks/Backplane/releases/latest/download/install.sh 2>/dev/null || wget -qO- https://github.com/BackplaneWorks/Backplane/releases/latest/download/install.sh; } | sh
backplane-app
```

The line takes curl where there is one and wget where there is not (a stock Ubuntu desktop has wget and no curl; macOS has curl and no wget); the installer downloads with whichever it finds, and so does the KiCad install.

x64 and arm64. It lays the release out for the guard (docs/deploy.md): the build in `~/.local/share/backplane/builds/<tag>/`, `CURRENT` naming the one that runs, and the guard at `~/.local/share/backplane/backplane`, linked as `backplane` in `~/.local/bin` beside `backplane-app` (the desktop app). It adds Backplane to your applications menu, on macOS as Backplane.app in `~/Applications` (`BACKPLANE_NO_DESKTOP=1` skips that). The desktop app starts its home's hub under the guard, which keeps it up to date from signed releases, takes each new build on probation and goes back when one fails (`backplane hub status`, `restart`, `rollback`). `BACKPLANE_NO_DESKTOP_APP=1` installs the hub alone and starts the guard at once (home `BACKPLANE_HOME`, default `~/.backplane/hub`; port `BACKPLANE_PORT`, default the first free from 3800); `BACKPLANE_START=1` starts it with the desktop app too. Run the installer again while the guard runs and it hands the new build to the guard, on probation. Add `~/.local/bin` to your `PATH` if asked.

## macOS (Apple silicon)

Backplane runs on Macs with Apple silicon (M1 or later) and macOS 13 or later; Intel Macs aren't supported, and the installer says so on one. The Backplane KiCad it installs needs macOS 15 or later; on an older macOS, Settings → CAD says why.

Run the install.sh one-liner above: the same layout, with the default home in `~/Library/Application Support/Backplane/hub`, and **Backplane.app in `~/Applications`** (your own Applications folder: no administrator password), so Backplane is in Launchpad, Spotlight and the Dock. Open it from there (or `open -a Backplane`). The app holds no build of its own: its program (`deploy/mac/launcher.c`) replaces itself with the desktop client of the build the guard runs (`~/.local/share/backplane/CURRENT`), so updates never touch the app, and the client is the app's own process. The hub it starts runs as its child. `BACKPLANE_NO_DESKTOP=1` leaves the app out (`backplane-app` in `~/.local/bin` still runs the client from a terminal); `BACKPLANE_NO_DESKTOP_APP=1` installs only the hub. With a `BACKPLANE_PREFIX` of your own, the installer writes it into the app and signs the app again (ad hoc).

**Documents, Desktop and Downloads.** The first time Backplane opens one of them (a project you add there, or the folder picker going in), macOS asks whether *Backplane* may; the hub, the guard and the agents run under that answer. Allow it and the project opens. If you said Don't Allow, Backplane says so in the picker or the Files tab ("macOS hasn't given Backplane access to Documents: allow it in System Settings → Privacy & Security → Files and Folders"): turn Backplane on there, under Documents Folder, and try again. Started from a terminal (`backplane-app`), macOS asks for the terminal instead.

**macOS builds are not notarized yet** (no Apple Developer ID; the app and its helpers are signed ad hoc). install.sh downloads with curl, which marks nothing as quarantined, so what it installs opens. If you unpack an archive you downloaded with a browser by hand and macOS refuses to open it, once only: right-click (or Control-click) → Open → Open; or on macOS 15+, try to open it, then System Settings → Privacy & Security → "Open Anyway"; or run `xattr -dr com.apple.quarantine ~/Applications/Backplane.app ~/.local/share/backplane`. The installer checks the archive's sha256 before it installs anything.

## Windows (x64, experimental)

Download `backplane-<tag>-windows-x64-native.zip`, extract the **whole** archive into a folder, and run `backplane-gpui.exe`. The exe is not signed yet, so Windows SmartScreen warns the first time (see below). The first run asks **Install Backplane to %LOCALAPPDATA%\Backplane? (needed for updates)**: Install copies the release to `%LOCALAPPDATA%\Backplane\builds\<tag>`, puts the guard (`backplane.exe`, which updates, restarts and rolls back the hub) at `%LOCALAPPDATA%\Backplane`, adds **Backplane** to the Start menu and starts from there; the folder you extracted can then be deleted. Not now runs it from the extracted folder, without updates, and asks again next time. Git for Windows is needed (the hub's shell commands use its `sh`); install your agent CLIs, KiCad and FreeCAD separately (Settings → CAD finds them). Closing the window leaves the hub running.

To remove Backplane: quit it and stop the hub (`%LOCALAPPDATA%\Backplane\backplane.exe hub stop`), then delete the folder `%LOCALAPPDATA%\Backplane` and the Start-menu entry `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Backplane.lnk`. Your projects, threads and settings stay in the hub's home, `%APPDATA%\Backplane\hub` (and the window's settings in `%APPDATA%\backplane-gpui`); delete those too to remove everything.

## From 0.14

On Linux and macOS, press Update in 0.14 as usual: 0.15 installs beside it, starts on the same port and offers to import 0.14's projects, threads, bots and paired phones (docs/deploy.md, "From 0.14"). Link your other machines again in Settings, Devices. If 0.15 fails to start, 0.14 comes back by itself. On Windows, 0.14 cannot update itself: quit it, download `backplane-<tag>-windows-x64-native.zip` from the release, extract it and run `backplane-gpui.exe` (Install, as above); its setup offers the same import.

## What each platform's archive carries

Every archive holds the hub, the desktop client, the guard, `backplane-voice` (dictation), `backplane-browser` (the Browser tab's helper), `backplane-step2glb` (STEP to GLB with OpenCascade, natively: no FreeCAD needed) and `assets/` with the generated icons and cat rigs; `release.yml` fails a build that misses any. The client also has the embedded Chromium (feature `cef`) with its runtime beside it: on Linux `libcef.so`, the `.pak` files, `icudtl.dat`, `v8_context_snapshot.bin` and `locales/`; on Windows the same with `libcef.dll` and `chrome_elf.dll`; on macOS `Chromium Embedded Framework.framework` and the `Backplane Helper*.app` helper apps, beside the client. The macOS archive also holds `Backplane.app` (`Contents/MacOS/Backplane`, the launcher; `Contents/Info.plist`, bundle id `works.backplane.gpui`; `Contents/Resources/Backplane.icns`; ad hoc signed), which install.sh copies to `~/Applications`; `release.yml` checks it is there and signed, and that every Mach-O in the archive asks for macOS 13.0 at most. The Browser tab is that Chromium inside the window on every platform.

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
rm -rf ~/Applications/Backplane.app   # macOS
```

Your threads, settings and keys stay in the home (`~/.backplane/hub`, or `~/Library/Application Support/Backplane/hub` on macOS); remove it too to start over. A package install (`.deb`, AUR) uninstalls through its package manager.

Nightly builds of `main` are prereleases on the [Releases](https://github.com/BackplaneWorks/Backplane/releases) page, each with its own installer: `{ curl -fsSL https://github.com/BackplaneWorks/Backplane/releases/download/<tag>/install.sh 2>/dev/null || wget -qO- https://github.com/BackplaneWorks/Backplane/releases/download/<tag>/install.sh; } | sh`.
