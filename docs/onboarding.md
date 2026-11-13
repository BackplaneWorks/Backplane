# Setting up Backplane

This guide follows the setup Backplane runs the first time you open it on a computer, on Linux, macOS and Windows. It has five steps. Every step can be skipped, and setup resumes where you left it. Phones have no setup: they pair with a computer's hub and open your uberbot.

## Install

### Linux

```sh
{ curl -fsSL https://github.com/BackplaneWorks/Backplane/releases/latest/download/install.sh 2>/dev/null || wget -qO- https://github.com/BackplaneWorks/Backplane/releases/latest/download/install.sh; } | sh
backplane
```

It takes curl, or wget where there is no curl (a stock Ubuntu desktop has only wget).

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

To change it, open Settings, Setup, go to Welcome and press Choose another. The old home stays as it is.

Connect to a hub on another computer connects this window to a hub that is already running. On that machine, open Settings, Devices, Add a device. Paste the pairing link here, or type the address and the short code.

### Moving from 0.14

Backplane 0.14 keeps its data in `~/.backplane-bend`. This version never changes that folder and refuses to use it as a home. Stop 0.14, then copy your threads into a new home:

```sh
scripts/migrate-from-bend.sh ~/.backplane-bend ~/.backplane/hub
```

The destination must be empty or missing, so run this before the first launch.

## What setup does

Setup opens after the hub starts. The list on the left shows the five steps and Done. A filled square is done, a dash is skipped and an empty square is still to do.

| Control | What it does |
| --- | --- |
| Next or Skip step | Moves on. Skip step marks an unfinished step as skipped. Enter does the same. |
| Back | Returns to the step before. |
| A step in the list | Goes straight to it. |
| Finish later | Closes setup and keeps your place. Esc does the same. Setup returns at the next launch, and Finish setup in the sidebar brings it back sooner. |
| End setup | Stops setup for good. Settings, Setup still runs it. |

Setup checks this machine each time you reach a step, so anything you install in another window shows as done when you press Check again. It only reads: it runs `--version`, `claude auth status` and `codex login status`, and it never installs or signs in unless you press a button.

Setup does not open by itself on a home that already has projects and never ran it. Settings, Setup runs it there. A home that finished the setup of an earlier version stays finished. One that stopped halfway opens at the first step of this setup that is not done.

## 1. Welcome

Where your data is kept, with Choose another, and the anonymous-usage switch. This build sends nothing either way; with On, a later build may send daily counts with a random id, never your conversations, files, names or keys. Settings, Privacy changes it.

When this computer has Backplane 0.14 data, Welcome also offers to copy it: Count reads the 0.14 log and changes nothing, Import copies projects, threads, bots, rooms and settings, and Import with keys also copies the vault and your Claude and Codex sign-ins. The 0.14 folder is only read.

## 2. Agents

Backplane runs command-line agents on the machine with the hub. You need one. The step is one list: Claude Code, Codex, Antigravity and Grok Build, with More agents folded under them (Muse Code, OpenCode, Pi, Oh My Pi). Each row says Install, Connect, or who it is connected as.

### Claude

Press Install. It runs Claude's own installer, with no terminal: `install.sh` on Linux and macOS, `install.ps1` through PowerShell on Windows. By hand, that is `curl -fsSL https://claude.ai/install.sh | bash`, or `irm https://claude.ai/install.ps1 | iex`.

Then press Connect. The Accounts panel opens under the list: name the account, press Connect, open the link, sign in and paste the code Claude shows. If Claude Code is signed in already, the row says Connected as your email. You can add several accounts, for example Work and Personal; when one reaches its usage limit, Backplane moves the thread to another. Settings, Accounts adds more later. Backplane never copies your keys and never runs a paid request to check a sign-in.

### Codex

Press Install. Codex needs no Node: Backplane downloads Codex's standalone release for your system, checks it against the checksum GitHub lists for it and places it in Backplane's own tools folder (`~/.local/share/backplane/tools/bin`, or `%LOCALAPPDATA%\Backplane\tools\bin` on Windows), which agents search. npm is used only where no standalone build exists and Node is already installed. Press Connect: Backplane shows a link and a code to enter there.

### The others

| Agent | Install | Sign in |
| --- | --- | --- |
| Antigravity | [antigravity.google](https://antigravity.google) | Run `agy` once and sign in |
| Grok Build | `curl -fsSL https://x.ai/cli/install.sh \| bash` | `grok login` |
| Muse Code | [Guide](https://dev.meta.ai/docs/muse-code/auth) | `muse login` |
| OpenCode | [opencode.ai](https://opencode.ai) | `opencode auth login` |
| Pi | [pi.dev](https://pi.dev) | Run `pi`, then `/login` |
| Oh My Pi | [GitHub](https://github.com/can1357/oh-my-pi) | Run `omp`, then `/login` |

These agents sign in within a terminal, and Backplane cannot check that sign-in; the row shows the command with Copy. An agent that is not signed in says so in its first reply.

## 3. Design tools

One row for KiCad and one for FreeCAD: found with its version, Install, or Locate it.

Setup installs Backplane KiCad by itself when no KiCad is found and a package exists for the computer: Linux x64, macOS on Apple silicon and Windows x64. It checks the download against the pinned checksum and the release's list, unpacks it in a staging folder and moves it into `<home>/kicad/<version>` (`%LOCALAPPDATA%\Backplane\kicad\<version>` on Windows), with no administrator password. The row shows the progress and ends with the version; a failure shows its reason and Try again. Where no package exists (Linux ARM64, Intel Macs) the row says so and offers the stock download. With stock KiCad found, one press installs Backplane KiCad beside it. Releases: <https://github.com/i2cjak/Backplane_KiCad/releases>.

Stock KiCad 9 or later is enough to plot and export. Agents edit a board in a running KiCad only with Backplane KiCad, our build with an editing API. With stock KiCad, agents edit the files. FreeCAD installs with one press through winget on Windows when it is there, and opens its download page otherwise.

| System | KiCad | FreeCAD |
| --- | --- | --- |
| Debian, Ubuntu | `sudo apt install kicad` | `sudo apt install freecad` |
| Fedora | `sudo dnf install kicad` | `sudo dnf install freecad` |
| Arch | `sudo pacman -S kicad` | `sudo pacman -S freecad` |
| macOS | `brew install --cask kicad` | `brew install --cask freecad` |

If a tool is somewhere else, give the full path to `kicad-cli` or `freecadcmd` under Locate KiCad or Locate FreeCAD, then press Use this path.

## 4. Where your uberbot lives

Always-on keeps your uberbot, reminders, bots and phone alerts running while this computer sleeps. Hardware and code still run here. Three options:

- **Stay on this computer**, chosen unless you pick another. You can move it later in Settings, Machines.
- **An always-on Boat**: a small server on your own Boat account. Setup connects the account, shows each size with its monthly cost, makes the Boat, signs it in to your agents and moves your uberbot there. You pay Boat directly.
- **Another machine of yours**: a home server or a desktop that stays on. Install Backplane there, pair it, sign it in and move your uberbot there.

docs/ALWAYS-ON.md has the design. In this build the last two are being finished; the step says so.

## 5. First project

A project is a folder, usually with a KiCad project in it. Give a folder that exists, or a new path, and press Create. In a git repository, each thread works in its own worktree. Then type a first message and press Start: it opens a thread with an agent in that project.

Pair a phone is one optional row here. Install the app first (Android from the signed file on the release page, iPhone through TestFlight), then press Show the code and scan it with the app, or type the short code and the hub's address. A code works once, for ten minutes, and the phone needs a route to the hub, usually Tailscale on the same tailnet. Treat the code and the link as passwords. The phone then opens your uberbot. Settings, Devices lists paired devices and revokes them.

## Done

Done says one thing: press `?` any time for keys. Open the uberbot ends setup and opens its chat; Backplane makes an uberbot named Bub if this home has none. Steps still open are listed with Go.

## Keyboard shortcuts

Press `?` when no text box has the keyboard, or F1 anywhere, or pick Keyboard shortcuts in the palette (Ctrl+K, Cmd+K on macOS). The sheet lists every key by where it works, with this device's choices. Settings, This device changes them.

## Linking another computer

Each computer runs its own hub. Linked hubs show each other's projects and threads in one sidebar. Hubs never link by themselves: open Settings, Machines, make a code on one computer and enter it with that computer's address on the other, then approve the request. Over ssh, `backplane pair --machine` does the same.

## Updates

A hub installed from a release checks for a new build every hour on its channel and installs it once no turn is running. If the new build fails its health check, the hub goes back to the one it had. A hub from a system package is updated by that package instead. Settings, Network and updates shows the version and the channel.

## Run setup again

Open Settings, Setup. It lists each step as done, skipped or not done, with Go beside each, and Run setup again starts over. It detects what is done each time. On a phone, Settings, Setup only says that setup runs on a computer.

## For developers

The steps are in `native/host/src/present/onboard.rs`; step 4's rows and presses are in `present/onboard_alwayson.rs`. To add a system, add an entry to `PLATFORMS` and a default home in `native/gpui/src/launch.rs`, then add its install commands to each `Agent`. The tests check the words of every entry. Windows is not in the list yet because there is no Windows machine to test on. `settings/onboard.rs` runs the checks and keeps them in the setting `onboard.checks`. The GPUI client draws setup in `native/gpui/src/screens/onboard.rs`, and the first window is `screens/first.rs`. Phones draw no setup: `onboard.view` answers no step, and `onboard.act` only carries out the 0.14 import card's buttons. The keys sheet is `native/gpui/src/screens/keysheet.rs`, from `keys::TABLE`.

All text follows the [unslop skill](https://github.com/cursor/plugins/blob/main/pstack/skills/unslop/SKILL.md). The test `every_word_follows_unslop` checks each string on each system.

`test/onboarding/run.sh` runs the first launch from a blank home in a private X server, with stand-in agents and real keys, and walks the five steps.
