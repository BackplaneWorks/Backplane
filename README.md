# Backplane

Agentic hardware development. Claude Code, Codex, Grok, Muse Code and Google's Antigravity work on your KiCad boards and FreeCAD parts, and every change they make is drawn live beside the conversation, in a browser, on your phone, or in the desktop app.

![A thread beside the board in 3D in the browser, and the same change compared on a phone](docs/media/hero.png)

## Why

- **Agents that know KiCad.** Each one is told the exact `kicad-cli` and the project's canonical files, and gets the [KiStack](https://github.com/American-Embedded/KiStack) skills: schematics, symbols, footprints, layout, BOM, Gerbers, panelizing and product renders, kept up to date. With Backplane KiCad installed, they edit through a headless KiCad over its IPC API instead of rewriting files.
- **You see what the agent changed.** A board file is text no one can review by reading it. Here the board, schematic and 3D model redraw as the agent saves, every step is kept, and any two versions compare like a diff.
- **Mechanical in the same loop.** Agents model enclosures and brackets in FreeCAD from the board's real outline and parts, and the Mech tab shows them in 3D beside the board.
- **Your machine, your tools.** It runs locally with your KiCad, your FreeCAD, your files and your agent subscriptions: Claude Code, Codex, Grok, Muse Code or Google's Antigravity.

## Web and phone, the same work

The hub runs on your machine. The browser and the native Android and iOS apps reach it over your tailnet and show the same threads, live. The phone apps are not a reduced view: threads, the board, schematic, 3D model, Mech parts, files and the design history with Compare are all there, drawn natively, with a notification when a turn ends or an agent needs you. Several machines show as one list.

| | | | |
|---|---|---|---|
| ![](docs/media/phone-list.png) | ![](docs/media/phone-thread.png) | ![](docs/media/phone-3d.png) | ![](docs/media/phone-mech.png) |

## What it does

**Viewers that follow the files.** Board, Schematic (with its sheets), 3D (the board with its parts), Mech, Files, Diff and Browser, all next to the thread. They draw only finished saves and fade in only what changed. Click a pad, track, symbol or pin to see its net, reference, value and footprint, and mention it in chat.

![The power sheet of the schematic beside the thread](docs/media/web-schematic.png)

**Mechanical parts.** The Mech tab turns any STEP in 3D, with the project's other parts beside it, and draws standard views on request.

![The case, exploded, on the Mech tab](docs/media/web-mech.png)

**Design history.** Backplane snapshots the KiCad files after every tool call. Step through what the agent did, play it back, jump to the tool call behind a step, or compare any two versions (steps, turns, commits, branches): removed, changed and added items are coloured and the rest dims.

![Compare: the tracks the agent widened](docs/media/web-compare.png)

**Design review.** The Review tab lists the BOM and what changed in it, runs ERC and DRC against a checklist, keeps comments pinned to a spot on the board or schematic, and cuts tagged releases with their outputs.

![The Review tab's checks](docs/media/web-review.png)

**Threads per project.** Each thread is an agent session with its own model and provider. Tool calls fold into one line, and threads settle on their own when you stop touching them. Fork a thread, rename it, pin it, or hand it to another.

| The board | Datasheets and files |
|---|---|
| ![](docs/media/web-board.png) | ![](docs/media/web-files.png) |

**Ordering (pilot).** Turn it on in Settings and a finished design shows Ready to order with a price. Buy it starts a thread that gets the design reviewed for manufacturing, asks what's missing, exports the files and quotes them; Buy now opens Stripe. During the pilot no card is charged, and the exported files can go to any fabricator.

![Ready to order](docs/media/web-order.png)

**A native window.** The desktop app is the same client drawn by Bend itself, with its own board and 3D renderer.

![The native window](docs/media/window-3d.png)

**And more.** A Chrome the agents drive while you watch, embedded in the Linux window; agents delegating work to other models; API keys the agents can use without ever reading them; voice input; a terminal per thread. Bots are persistent agents with a face, memory, routines and webhooks, which talk in rooms, across your machines and other people's; one can be your uberbot, the main interface to everything. The interface speaks English and Japanese.

<img src="docs/media/cats.png" width="420" alt="Bots and their moods">

## How you use it

1. Add a project: any folder with a KiCad project in it (several, or one below the root, are fine).
2. Start a thread and ask: "the SSD rails look thin, check them and widen what falls short".
3. Watch the board change as the agent works. Scrub the history bar, open Compare, ask for changes.
4. Leave. Answer its questions or start the next thread from your phone.

## Install

```sh
curl -fsSL https://github.com/i2cjak/Backplane/releases/latest/download/install.sh | sh
backplane
```

Linux (x64, arm64) and macOS (Apple silicon). It installs into `~/.local/share/backplane` and keeps itself up to date (`BACKPLANE_NO_UPDATE=1` turns that off). On Linux it also adds Backplane to your applications menu (`BACKPLANE_NO_DESKTOP=1` skips that). Releases also carry an AppImage, a `.deb`, an AUR `PKGBUILD` and the Android APK. You need at least one agent CLI (`claude`, `codex`, `grok` or `muse`; Antigravity installs from Settings) and KiCad; FreeCAD for mechanical work. `backplane --headless` runs it without a window. To pair a phone, scan the QR code in Settings (Pairing link, QR code) with the app, or paste the pairing link.

Treat pairing links and QR codes as credentials. Browser pairing exchanges the fragment for an HttpOnly, SameSite=Strict cookie (Secure over HTTPS), then removes the fragment; existing browser tokens migrate out of local storage after a successful exchange. Native phone clients retain their pairing-link protocol. Remote clients need the pairing credential; a browser cannot inherit localhost access through a foreign origin or rebound hostname. Local programs running as your user remain trusted. Full Access runs agent tools with your user’s access; choose a more restrictive thread permission mode when wanted. Pairing controls access to the hub, and does not provide OS isolation for a Full Access agent.

On a Mac there is no native window yet: `backplane` runs the hub and you use it in the browser at http://127.0.0.1:3787 (and from your phone, as anywhere). Add `~/.local/bin` to your `PATH` if the installer says so.

To uninstall what install.sh put in place, quit Backplane, then:

```sh
rm -rf ~/.local/share/backplane
rm -f ~/.local/bin/backplane ~/.local/bin/backplane-serve
rm -f ~/.local/share/applications/backplane.desktop \
  ~/.local/share/icons/hicolor/scalable/apps/backplane.svg \
  ~/.local/share/icons/hicolor/256x256/apps/backplane.png
```

Your threads, settings and keys stay in `~/.backplane-bend` and the model cache in `~/.cache/backplane-bend`; remove them too to start over. A package install (`.deb`, AUR) uninstalls through its package manager.

Nightly builds of `main` are prereleases on the [Releases](https://github.com/i2cjak/Backplane/releases) page, each with its own installer: `curl -fsSL https://github.com/i2cjak/Backplane/releases/download/<tag>/install.sh | sh`.

## Build

Needs [Bend](https://bend-lang.com), clang 19+, X11 headers and bun. On macOS, Xcode's clang is enough and there are no X11 headers to find: `scripts/build.sh` builds the hub and web client only.

```sh
scripts/check.sh   # type-check everything, prove every law
scripts/test.sh    # tests
scripts/build.sh   # dist/: the app, the server and the web client
```

Backplane is written in Bend, and the rules that matter are laws proven in [`PROOF.bend`](PROOF.bend). [AGENTS.md](AGENTS.md) describes the layout; [mobile/README.md](mobile/README.md) the phone apps.

## License

MIT. See [LICENSE](LICENSE). `backplane-step2glb` embeds OpenCascade (LGPL-2.1); its notices ship in `licenses/`.
