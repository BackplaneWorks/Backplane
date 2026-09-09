# Backplane

Backplane is a workspace for agentic hardware development. It combines coding-agent conversations with KiCad PCB, schematic, Gerber, and 3D views. It is based on [T3 Code](https://github.com/pingdotgg/t3code), with a companion [KiCad fork](https://github.com/i2cjak/Backplane_KiCad).

## Install

Download an installer for your system from [Backplane Releases](https://github.com/i2cjak/Backplane/releases). Each release lists its supported platforms. Installers include the desktop app, Chromium browser, server, Python, and the matching Backplane KiCad runtime.

Open Backplane and configure the coding provider you use. Provider accounts and credentials are your own. Berkeley Mono is used when installed on your system; the font is not redistributed with Backplane.

See [install and first run](docs/user/install.md), [KiCad views](docs/user/kicad.md), and [remote access](docs/user/remote-access.md).

## Develop

Use the Node.js version in `package.json` and [Vite+](https://viteplus.dev/guide/).

```sh
git clone https://github.com/i2cjak/Backplane.git
cd Backplane
vp install
vp run dev --home-dir .t3
```

Open the pairing URL printed by the development server. Source builds use your local KiCad installation unless a Backplane runtime is configured. Viewer-tab changes are tracked separately from the app's release and appearance work.

[Release operations](docs/operations/release.md) describes runtime staging and publishing. Original licenses and upstream attribution remain in this repository and the public KiCad fork.
