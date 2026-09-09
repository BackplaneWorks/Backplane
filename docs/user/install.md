# Install Backplane

Download the Linux x64 AppImage from [Backplane Releases](https://github.com/i2cjak/Backplane/releases).
The desktop app includes its Chromium browser, server, Python, the matching Backplane KiCad runtime, and the official KiCad symbols, footprints, 3D models, and project templates.
You do not need to install Chrome, Node.js, Python, or KiCad separately.

Make the downloaded AppImage executable in your file manager, then open it.
If your system cannot mount AppImages, launch it from a terminal with
`--appimage-extract-and-run`.

Berkeley Mono is used when installed locally. Backplane includes no Berkeley Mono
font files and falls back to your system monospace font.

## Open the bundled KiCad editor

To use the full KiCad GUI, extract the AppImage into its own directory:

```sh
mkdir -p ~/Applications/backplane-kicad
cd ~/Applications/backplane-kicad
/path/to/Backplane.AppImage --appimage-extract
./squashfs-root/resources/kicad/bin/kicad
```

This starts KiCad's schematic and PCB editor. It is separate from the Backplane
viewer.

## First run

Open **Settings → Providers**, enable your coding provider, and follow its
installation and sign-in instructions. Provider accounts and any required
provider CLI are separate from the Backplane installer.

Open your project directory and start a thread. PCB, schematic, Gerber, and 3D
files appear in the project's KiCad views.

## Updates

Download the newer AppImage from the same releases page, close Backplane, and
replace the previous AppImage. Your saved threads and settings remain in your
user data directory.

The initial release supports Linux x64. Native Windows and macOS installers and
Backplane mobile store releases are not available yet.

For source builds, see the [development instructions](../../README.md#develop).
