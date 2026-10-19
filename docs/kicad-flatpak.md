# KiCad 10 installed with Flatpak

Backplane can use the stock KiCad 10 CLI from `org.kicad.KiCad` for exports,
ERC/DRC and its file-based fallback. Run it through Flatpak's launcher; do
not point Backplane at an executable inside `/var/lib/flatpak` or link
KiCad's Flatpak libraries into Backplane.

Install the adapter from this checkout:

```sh
install -D -m 755 scripts/kicad-flatpak.sh "$HOME/.local/lib/backplane-flatpak/bin/kicad-cli"
"$HOME/.local/lib/backplane-flatpak/bin/kicad-cli" version
```

In Backplane's KiCad settings, select that **absolute executable path**.
Alternatively, start Backplane with:

```sh
BACKPLANE_KICAD_CLI="$HOME/.local/lib/backplane-flatpak/bin/kicad-cli" backplane
```

The setting takes precedence over the environment. Every CLI caller uses
the same adapter, including the runtime health probe, viewers, checks and
release exports. Arguments are forwarded separately, preserving spaces and
shell characters. The adapter returns KiCad's exit status and diagnostics.
Set `BACKPLANE_KICAD_FLATPAK_REF=org.kicad.KiCad//stable` to select a branch.

The command grants KiCad access to host project/model files and `/tmp` and
`/var/tmp` for Backplane's generated inputs and outputs, for that invocation.
It does not change `flatpak override` settings. Flatpak's reserved system
paths still belong to KiCad's runtime; keep projects in home, removable
storage or a normal data directory. KiCad's own Flatpak configuration and
library extensions remain in use. The Flatpak must already be installed.

Inside a Backplane Flatpak, the adapter uses `flatpak-spawn --host` to ask
the host Flatpak service to launch KiCad's separate sandbox. That requires
`--talk-name=org.freedesktop.Flatpak` on Backplane's sandbox. It is not a
library link or a direct invocation of KiCad's sandboxed executable.

Stock KiCad 10 does **not** provide Backplane's patched headless
`api-server`. Backplane will correctly report the CLI fallback rather than
claiming IPC support; the Backplane KiCad fork is still required for IPC
editing, project downgrade and its other fork-only commands.

References: [Flatpak command reference](https://docs.flatpak.org/en/latest/flatpak-command-reference.html),
[KiCad CLI](https://docs.kicad.org/10.0/en/cli/cli.html).
