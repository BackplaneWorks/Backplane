# Setting up Backplane

This guide follows the setup Backplane runs the first time you open it, on Linux and macOS. Windows comes later. Every step can be skipped, and setup resumes where you left it.

## Install

### Linux

```sh
curl -fsSL https://github.com/BackplaneWorks/Backplane/releases/latest/download/install.sh | sh
backplane
```

The installer supports x64 and arm64. It puts Backplane in `~/.local/share/backplane`, links `backplane` into `~/.local/bin` and adds Backplane to your applications menu. If your shell cannot find `backplane`, add `~/.local/bin` to your `PATH`.

### macOS

Backplane runs on Apple silicon. Run the same install command, or unpack the release archive and move `Backplane.app` to Applications.

macOS builds are not notarized yet. If macOS refuses to open the app, try to open it, then choose Open Anyway in System Settings, Privacy and Security. This is needed once.

## First launch

The first window asks where Backplane keeps your data. That is one folder on this computer, with your threads, settings and keys. A background program, the hub, runs your agents from it.

| System | Default home |
| --- | --- |
| Linux | `~/.backplane/hub` |
| macOS | `~/Library/Application Support/Backplane/hub` |

Press Use this folder to take the default, or Choose a folder for another. A folder that already has a hub's data opens as it is. The hub takes the first free port from 3800 to 3899 and keeps running when you close the window, so agents finish their work. Later launches go straight to the same home.

To change it, open Settings, Setup, go to Where your data is kept and press Choose another folder. The old home stays as it is.

Connect to a hub on another computer connects this window to a hub that is already running. On that machine, open Settings, Devices, Add a device. Paste the pairing link here, or type the address and the short code.

### Moving from 0.14

Backplane 0.14 keeps its data in `~/.backplane-bend`. This version never changes that folder and refuses to use it as a home. Stop 0.14, then copy your threads into a new home:

```sh
scripts/migrate-from-bend.sh ~/.backplane-bend ~/.backplane/hub
```

The destination must be empty or missing, so run this before the first launch.

## What setup does

Setup opens after the hub starts. The list on the left shows each step. A filled square is done, a dash is skipped and an empty square is still to do.

| Control | What it does |
| --- | --- |
| Next or Skip step | Moves on. Skip step marks an unfinished step as skipped. Enter does the same. |
| Back | Returns to the step before. |
| A step in the list | Goes straight to it. |
| Finish later | Closes setup and keeps your place. Esc does the same. Setup returns at the next launch, and Finish setup in the sidebar brings it back sooner. |
| End setup | Stops setup for good. Settings, Setup still runs it. |

Setup checks this machine each time you reach a step, so anything you install in another window shows as done when you press Check again. It only reads: it runs `--version`, `claude auth status` and `codex login status`, and it never installs or signs in by itself.

Setup does not open by itself on a home that already has projects and never ran it. Settings, Setup runs it there.

## Sign in to agents

Backplane runs command-line agents on the machine with the hub. You need one to start.

### Claude

Install Claude Code:

```sh
curl -fsSL https://claude.ai/install.sh | bash
```

Then add an account in the Claude step. Name it, press Connect, open the link, sign in and paste the code Claude shows. If Claude Code is signed in already, the step finds that and shows as done. If a sign-in fails or the code expires, press Connect again. Use for new threads picks the account that new threads start on, and threads you already have keep theirs. Backplane never copies your keys and never runs a paid request to check a sign-in.

You can add several Claude accounts, for example Work and Personal. Each one signs in separately and stays on this machine. When an account reaches its usage limit, Backplane moves the thread to another. Each thread keeps its own account, and the chip beside the model picker changes it. Settings, Accounts adds more later.

### Codex

```sh
npm install -g @openai/codex
```

Add an account in the Codex step and press Connect. Backplane shows a link and a code to enter there.

### Other agents

| Agent | Install | Sign in |
| --- | --- | --- |
| Grok Build | `curl -fsSL https://x.ai/cli/install.sh \| bash` | `grok login` |
| Muse Code | [Guide](https://dev.meta.ai/docs/muse-code/auth) | `muse login` |
| OpenCode | [opencode.ai](https://opencode.ai) | `opencode auth login` |
| Pi | [pi.dev](https://pi.dev) | Run `pi`, then `/login` |
| Oh My Pi | [GitHub](https://github.com/can1357/oh-my-pi) | Run `omp`, then `/login` |

These agents sign in within a terminal, and Backplane cannot check that sign-in. An agent that is not signed in says so in its first reply. This build cannot install Antigravity yet.

## KiCad and FreeCAD

Stock KiCad 9 or later is enough to plot and export. Agents edit a board in a running KiCad only with Backplane KiCad, our build with an editing API; the KiCad step says whether the one it found is that build and links to the downloads. With stock KiCad, agents edit the files.

Agents use KiCad for boards and schematics and FreeCAD for mechanical parts. The viewers need neither, but agents cannot change designs without them. Setup runs each tool and asks for its version, so a broken install shows as broken with its error.

| System | KiCad | FreeCAD |
| --- | --- | --- |
| Debian, Ubuntu | `sudo apt install kicad` | `sudo apt install freecad` |
| Fedora | `sudo dnf install kicad` | `sudo dnf install freecad` |
| Arch | `sudo pacman -S kicad` | `sudo pacman -S freecad` |
| macOS | `brew install --cask kicad` | `brew install --cask freecad` |

If a tool is somewhere else, give the full path to `kicad-cli` or `freecadcmd` under Locate it, then press Use this path.

## Pair another machine

Each computer runs its own hub. Linked hubs show each other's projects and threads in one sidebar, and work stays on the machine that owns it.

Hubs never link by themselves, even on one Tailscale account. The step shows the Settings, Machines page. Press Make a code on one computer. On the other, enter the first computer's address and the code under Pair a machine. The first computer then shows a request with a check code, which must match, and you press Approve. Over ssh, `backplane pair --machine` does the same. A code works once, for ten minutes, and both computers need a network route to each other.

To see another hub's threads in this window without linking, use the hub list at the top of the sidebar, or Settings, Hubs.

## Pair a phone

Install the app first: iOS through TestFlight, Android from the signed file on the release page. Then press Show the QR code in the phone step, or open Settings, Devices, Add a device.

Scan the QR code with the app, or type the short code and the hub's address. Each invitation works once, for ten minutes. The phone needs a route to the hub, usually Tailscale on the same tailnet. Treat the QR code and the link as passwords. Settings, Devices lists paired devices and revokes them.

## Restarts and updates

The hub keeps running when you close the window, so agents finish their work. A hub installed from a release checks for a new build every hour on its channel, stable or beta, and installs it once no turn is running. If the new build fails its health check, the hub goes back to the one it had. A hub from a system package is updated by that package instead. Setup says which kind this hub is. Settings, Network and updates shows the version and the update channel. With Continue after a restart on in Settings, Threads, turns that a restart cuts short go on by themselves.

## Your first project and thread

A project is a folder, usually with a KiCad project in it. Give a folder that exists, or a new path, and press Create. In a git repository, each thread works in its own worktree.

A thread is one conversation with an agent. Type what you want and press Enter. The picker beside the box sets the agent, model and effort.

## Tour

**Boards and schematics.** The rail beside a thread opens its project. Backplane draws boards and schematics itself, so they open without KiCad running, and the view updates when an agent changes a file. Keys work while the keyboard is outside the message box.

| Key | View |
| --- | --- |
| B | Board |
| S | Schematic |
| 3 | 3D model |
| V | Schematic and board side by side |
| M | Mechanical parts |
| F | Files |
| D | Diff of the thread's changes |
| R | Review |
| W | Browser |

**3D view.** Press 3 for the board in 3D with its parts. Drag to turn it, and use the wheel to zoom at the pointer. A right drag, a middle drag or Shift with a drag moves it.

**Mechanical parts.** Press M for the Mech view. It lists every STEP file in the project's `mech` folder and draws the one you pick in 3D with the same controls. Agents write each part as a FreeCAD script beside its STEP, so they need FreeCAD to change one.

**Browser.** Agents have a web browser on the hub's machine for datasheets, part pages and web tools. Press W to watch it. A bot's browser is also its Browser tab. This build shows the page the agent has open, and you cannot click in it yet.

**Threads.** Settle takes a thread out of the active list and Snooze hides it until later. Fork starts a new thread from this one.

**Bots.** A bot is an agent that stays between chats, with a name, a cat and a memory. Mention one in any message: `@scout look at the BOM`.

**A bot's space.** Each bot has a Space tab, a page the bot writes with its `space_set` tool. It can hold a status, a table, buttons and fields to fill in. Ask the bot for one, for example "make me a space that tracks the open BOM questions".

**Rooms.** A room is a shared chat for you and your bots. Each bot reads the room and answers there.

**Routines.** A routine runs a bot on a schedule. Open a bot, then its Routines tab. Backplane reads times like `tomorrow 9am`, `monday 09:00` and `in 20m`.

**Palette and shortcuts.** Press Ctrl+K, or Cmd+K on macOS, to jump to a thread or run a command. Settings, This device lists every shortcut.

**Settings.** Settings holds the choices for the whole hub. This device holds the choices for one window.

**Uberbot.** The uberbot is the bot you talk to first. Its Status tab shows what runs and what waits for you.

## Run setup again

Open Settings, Setup. It lists each step as done, skipped or not done, with Go beside each, and Run setup again starts over. It detects what is done each time.

## For developers

The steps are in `native/host/src/present/onboard.rs`, for every client. To add a system, add an entry to `PLATFORMS` and a default home in `native/gpui/src/launch.rs`, then add its install commands to each `Agent`. The tests check the words of every entry. Windows is not in the list yet because there is no Windows machine to test on. `settings/onboard.rs` runs the checks and keeps them in the setting `onboard.checks`. The GPUI client draws setup in `native/gpui/src/screens/onboard.rs`, and the first window is `screens/first.rs`. Phones use `onboard.view` and `onboard.act` over data/1.

All text follows the [unslop skill](https://github.com/cursor/plugins/blob/main/pstack/skills/unslop/SKILL.md). The test `every_word_follows_unslop` checks each string on each system and client.

`test/onboarding/run.sh` runs the first launch from a blank home in a private X server, with stand-in agents and real clicks.
