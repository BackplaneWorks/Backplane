# T3CAD

T3CAD is an agent-driven electronics workspace, forked from [T3 Code](https://github.com/pingdotgg/t3code). It combines coding-agent conversations with KiCad PCB, schematic, Gerber, and 3D inspection across web, desktop, and mobile.

Open optional BOM, footprint, symbol, and EMerge / Analysis tabs inside the KiCad viewer. The BOM follows the project's saved KiCad settings. Antenna specifications can be prepared for the existing EMerge workflow; openEMS execution is not integrated yet. See the [KiCad viewer guide](docs/user/kicad.md).

## Run from source

Install the repository's Node.js and Vite+ prerequisites, then:

```sh
git clone https://github.com/i2cjak/T3CAD.git
cd T3CAD
vp i
vp run dev
```

Use the pairing URL printed by the development server. `vp run dev --share` makes an isolated development environment available over your Tailnet. BOM, footprint, symbol, and 3D previews require `kicad-cli` on the server; Gerber previews require Python 3.

The fork retains upstream package names and connection protocols. Upstream T3 Code downloads and `npx t3` do not contain these T3CAD additions. No T3CAD binary release is published yet.

## Documentation

Full docs live in [docs/](./docs). There's no docs site yet.

- [Install and first run](./docs/user/install.md)
- [Permission modes](./docs/user/permission-modes.md)
- [Keyboard shortcuts](./docs/user/keybindings.md)
- [Project settings](./docs/user/project-settings.md)
- [Remote access from a phone or another machine](./docs/user/remote-access.md)
- [Keeping app and server in sync](./docs/user/updating.md)
- [Source control integrations](./docs/user/source-control.md)
- Multiple accounts: [Codex](./docs/user/providers-codex.md) · [Claude](./docs/user/providers-claude.md)
- [Run T3CAD as a background service](./docs/user/background-service.md)

Building from source? Start at [docs/internals/overview.md](./docs/internals/overview.md).

## If you REALLY want to contribute still.... read this first

### Install `vp`

T3CAD uses Vite+ so you'll need to install the global `vp` command-line tool.

#### macOS / Linux

```bash
curl -fsSL https://vite.plus | bash
```

#### Windows

```bash
irm https://vite.plus/ps1 | iex
```

Checkout their getting started guide for more information: https://viteplus.dev/guide/

### Install dependencies

```bash
vp i
```

Read [CONTRIBUTING.md](./CONTRIBUTING.md) before reporting a bug or opening a PR.

Have a feature request? Start an [Ideas discussion](https://github.com/pingdotgg/t3code/discussions/categories/ideas).

Need support? Join the [Discord](https://discord.gg/jn4EGJjrvv).
