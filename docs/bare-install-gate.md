# The bare-install gate

A person's first hour with Backplane on a machine that has nothing: no Backplane, no KiCad, no caches. The gate installs Backplane as the docs
say, lets setup fetch KiCad, opens a real-shaped project, edits it in the embedded KiCad with an agent beside, saves, checks, and exports it to
a release KiCad. It runs on a fresh Boat, from our machines, never on GitHub runners.

```sh
scripts/bare-gate.sh                          # the published release (latest), on ubuntu:24.04
scripts/bare-gate.sh --commit origin/main     # a build of that commit, made on a build box, packed like a release
scripts/bare-gate.sh --release v0.15.2 --image ubuntu:24.04 --keep
```

It prints the checks, a timing table and the paper cuts ranked by how early and how hard a person meets them, writes everything to
`$BP_TMP/bare-gate-<time>/` (screenshots in `shots/`, logs, `checks.tsv`, `timings.tsv`, `findings.tsv`), deletes every box it made (only those)
and exits non-zero on any FAIL.

## What "bare" means

- **The clean machine** is a container from a stock distro image (`--image`, default `ubuntu:24.04`; `debian:12`, `fedora:41` follow) on a
  fresh Boat made with `--no-env` (none of the account's secrets). Nothing of ours is baked in: not the `bp-build-base` snapshot, no home, no
  tokens, no `.git`. The person is a plain user `person` with an empty home.
- **What a person's desktop already has** is taken from the distro's published desktop manifest (Ubuntu:
  `releases.ubuntu.com/<v>/ubuntu-<v>-desktop-amd64.manifest`). Every library Backplane's programs or the KiCad runtime link is looked up
  (`ldd`, then `apt-file` in a separate helper container); a package on the desktop manifest is installed as the desktop the person has, and
  one that is not is a paper cut (ship it, or document it as a prerequisite). A bundled library older than a system library needs is one too.
  The runtime set a desktop has without linking it (Vulkan driver, GL, fonts, a session bus, python3, xdg-utils) goes in the same way.
- **The harness** (Xvfb, i3, xdotool, ImageMagick, Bun for the hub watcher) stays on the Boat host, outside the clean machine; the container
  shares only the host's network and X socket. The person's home is a host folder so the harness can read the hub's token and logs.
- **Agents** are stand-ins only (`test/bare/coedit_standin.py` as `claude`): no model runs.
- **Projects** are the fork's demos as the installed runtime ships them (`pic_programmer`, `ecc83`, `complex_hierarchy`, `video`) and the
  repo's fixtures. The person's boards never go to a Boat.

## The journey (test/bare/journey.sh, journey_project.sh)

| Step | How | Oracle |
|---|---|---|
| Install | the README's first install line as `person` (curl, else wget: the stock desktop has wget only), on the release's installer; a commit's build installs with `BACKPLANE_ARCHIVE=… sh install.sh`, and the README's line is checked to fetch install.sh with what the desktop has | `backplane-app` in `~/.local/bin` |
| Desktop libraries | `ldd` on every ELF of the build | classified against the desktop manifest |
| First start | `backplane-app` as from the menu; Use this folder (xdotool on the rectangle the client logs) | hub answers `/hello`; time to the first window and to the hub |
| Setup to KiCad | Next (Enter) until the step with KiCad | `onboard.step`; KiCad's install starts by itself (else a paper cut, and Install is pressed) |
| KiCad fetched | the hub's installer | `<home>/kicad/<v>/bin/kicad-cli` runs in the box; time from first start; download and installed size; the runtime's libraries as above |
| Project | `project.new {path, orAdd}` and `thread.create`, as setup's Create and Start send them, on `~/Documents/pic_programmer` | `ProjectCreated`, `ThreadCreated` in the log |
| Edit | the client's `--script`: `thread:<id>`, `rail:Board`, `ui:Edit` | the client's KiCad report (state ready, painted, PCB frame shown, in the window's tree); screenshot |
| Schematic | `rail:Schematic`, then back to Board | report: SCH frame shown |
| Agent co-edit | a turn: `kicad_ipc` OpenDocument, CreateItems (a track), SaveDocument | the hub's answers (tracks n → n+1); the pane's pixels change; the file on disk has the track |
| Comment | `review_comment add` on the board | the hub's answer; `review_comments` |
| DRC | the runtime's `kicad-cli pcb drc --format json` on the saved board | the report |
| Stock KiCad | the current release from its official packages (Ubuntu: `ppa:kicad/kicad-10.0-releases`), then `test/bare/stock_compat.py` | below |

The three oracles of docs/kicad-embed-e2e.md: the window system and pixels (screenshots, the pane's report with its X tree, before/after
diffs), the live IPC (the hub's `kicad_ipc` answers in the agent's turn), the files and `kicad-cli` (the saved board, DRC, the stock checks).

The KiCad a fresh install gets must be the latest release of the repository setup downloads from (`install.rs` `FROM`), or the check
fails. A deliberate hold is declared in `test/bare/kicad-hold` (`<version> <why>`): while the install is exactly that version the check
reads HOLD with a paper cut; any other version fails, and the hold still being there once the install is the latest fails too. The commit
that bumps the pin deletes it.

Every X call the harness makes (xdotool, screenshots, xwininfo) and every hub or KiCad IPC call is bounded: a timeout is a FAIL naming
the step (each phase prints `STEP <seconds> <name>`), with the box's processes in `logs/xstuck.txt`. Rerun 3 lost 46 minutes to one
unbounded X call in the second session.

The stand-in stays alive across turns and reloads its plan configuration each turn: the journey supplies the plan after the co-edit and
comment. Run `python3 test/bare/coedit_standin_test.py` to check that sequence without a model or KiCad. Fab is opened by the client's
script before its `fab` mark, so Review's rectangle is captured before the harness reads it.

## Stock KiCad compatibility (test/bare/stock_compat.py)

The fork saves natively; a person with a release KiCad gets the fork's export, `kicad-cli project downgrade --target 10.0`. For each project:
the fork saves it natively (`pcb/sch upgrade --force`; `--as-is` for the one the journey already saved after the co-edit and comment), runs its
own ERC and DRC, exports **without** `--force` (a refusal is a finding; it then retries with `--force` so the rest still runs), and the release
`kicad-cli` runs ERC, DRC, gerbers and the netlist on the export. Pass: no repair, version, unknown-token or similar line on its output; ERC and
DRC identical as counts by (kind, type, severity), exclusions counted apart; no `*.kicad_plan` in the export, and a project's own plan keeps
every value through the fork's save. Each CLI gets a private HOME seeded with its own install's library tables, as a first start would.
A difference is attributed: when the fork's own checks on the export differ from its checks on the native files, the export changed the data
(always a FAIL); otherwise it is the two engines (master and 10.0) on the same file, and passes only when listed with a reason in
`test/bare/stock_engine_known.json`. The fork has the same gate (`scripts/backplane-stock-export-regression.py` in Backplane_KiCad).

Locally, on COPIES of real boards, with a stock `kicad-cli` installed here (never on a Boat):

```sh
python3 -I test/bare/stock_compat.py --fork-cli ~/.backplane-gpui-fresh/home/kicad/10.99.0-bp.11/bin/kicad-cli --stock-cli /usr/bin/kicad-cli \
  --out $BP_TMP/compat COPY_OF_PROJECT...
```

## DRC correctness probe (test/bare/drc_probe.py)

`drc-zone-fill-short`, lane P's probe: on a copy of the complex_hierarchy demo a foreign-net track (Net-(R313-Pad1), 0.05 x 0.1 mm, B.Cu)
inside the GND fill at (100.5, 100.9), where the fill's triangulation leaves copper uncovered. DRC must report it with actual 0 under the
demo's rules and at all under 0.1 mm rules; a control track at (120, 90) checks the probe. Runtimes before 10.99.0-bp.12.1 without the
capability `drc-zone-guard` miss it: that is a **KNOWN-RED** line by name ("DRC zone-fill triangulation, fixed in bp.12.1"), listed apart
from passes and fails; from bp.12.1 on a miss is a FAIL. Stock KiCad's DRC on the fork's downgrade of the same boards is recorded (NOTE).
First results (Nov 13): bp.10, bp.11 and bp.12 report 0.1174 mm (demo rules) and nothing (0.1 mm rules); stock 10.0.7 reports actual
0.0000 mm on both.

## Commit mode

`--commit SHA` makes a build box from `bp-build-base` (the build snapshot; never the gate box), copies `git archive SHA` to it and runs
`test/bare/build-archive.sh`: the hub, window, guard and voice built at that commit (profile `fast`), the CEF runtime the window was built
against and the commit's assets, over the newest release's archive for the parts other toolchains build (`backplane-browser`,
`backplane-step2glb`, KiStack, licenses; `OVERLAY` in the archive says so). It writes `install.sh` from `deploy/install.sh.in` as release.yml
does; the gate box installs that archive through it. Builds from this desktop do not run on 24.04 (glibc 2.43); the build box is Ubuntu 24.04.

## When it runs

- **Before a release**: `scripts/bare-gate.sh --commit <the release commit>`; a FAIL holds the release.
- **After a release**: `scripts/bare-gate.sh` (the published one, as a person gets it).
- **Nightly** from this machine: `scripts/bare-gate-nightly.sh install <lead thread id>` (the person runs it once: a user systemd timer at
  03:30; agents never start timers on the person's machine). Each night it runs `--commit origin/main` from a worktree of its own
  (`.backplane/worktrees/bare-nightly`), skips when under 20 GB are free, writes `$BP_TMP/bare-nightly/<date>/` and `~/.cache/bare-nightly.last`,
  and only on a FAIL posts the failed checks and paper cuts to that thread through this machine's hub, as a paired client does
  (`BACKPLANE_NIGHTLY_HOME`, default `~/.backplane-gpui-fresh/home`). `status` and `uninstall` too.
Boats cost while they run: the gate deletes its gate box and its build box on every exit (`--keep` leaves the gate box, named in the output).

## Windows and macOS (prepared, not run yet)

The same journey and oracles; what differs is the clean machine, the driver and the install path. Neither runs until the lead says so
(bp-win stays deallocated when not in use; the Mac mini only under the lock `mac-heavy`).

**Windows (bp-win).** The clean machine: Windows Sandbox inside bp-win (a disposable stock Windows of the host's build: no Git, no Python,
no VC++ redistributable beyond what Windows ships, nothing of ours); it needs bp-win's size to allow nested virtualization (Dv5 does) and the
optional feature `Containers-DisposableClientVM`. A `.wsb` file maps one folder read-only (the archive, the harness's PowerShell agent and
the fixtures) and one read-write (results), and its logon command starts the agent, which polls the results folder for jobs as
`test/windows/rig/sessagent.ps1` does in bp-win's own session, so `sess.sh` drives it unchanged. Steps: the release zip (or a commit's, made
by release.yml's Windows packaging run on bp-win) extracted as a person does; `backplane-gpui.exe`; its "Install Backplane to
%LOCALAPPDATA%" answered Install; setup to Design tools; the WINDOWS package fetched; then `win_embed_e2e.py`'s window/editor checks against
that install (HWND tree, named-pipe IPC, screenshots) and the same stand-in agent (Python is not on stock Windows: the stand-in is shipped as
the gate's own embeddable Python in the mapped folder, never installed system-wide). Libraries: the PE imports of every `.exe`/`.dll` of the
build and the runtime against the Sandbox's System32 (`pefile` on the host side), as the desktop manifest is used on Linux. Stock KiCad:
the official `kicad-10.0.x-x86_64.exe /S` in the Sandbox. Needed in scripts: `scripts/bare-gate-win.sh` (start bp-win, write the `.wsb`, run,
pull results, deallocate), `test/windows/bare/agent.ps1`, a PE-import classifier. Product questions the first run answers:
- Git for Windows is a stated prerequisite (the hub's `sh`), and the KiCad install script runs under `sh` (`install.rs`): on a bare
  Windows, setup's automatic KiCad fetch cannot start until Git is installed, and setup does not install or explain it. Likely rank 1.
- The exe is unsigned: SmartScreen's "Windows protected your PC" is the first thing a person sees (documented, not fixed).
- The WINDOWS KiCad package is bp.10: no `routing-plan` or `coedit`, as on Linux.

**macOS (Mac mini, under `mac-heavy`).** The clean machine: a VM from Cirrus Labs' vanilla macOS image (`tart clone
ghcr.io/cirruslabs/macos-sequoia-vanilla bare-gate`: stock macOS with only the admin user; about 25 GB on the mini, deleted after),
run headless with VNC for screenshots; nothing of ours or Homebrew in it. Steps: the documented `curl … install.sh | sh` (macOS has curl);
`backplane-app`; setup to Design tools; the MAC package fetched; the journey through the client's `--script` (no Xvfb: the VM's real
window server; screenshots with `screencapture` in the VM; clicks with the client's own script steps, since there is no xdotool). Libraries:
`otool -L` of the build and the runtime against the VM's `/usr/lib` and `/System`. Stock KiCad: the official `.dmg` copied to `/Applications`.
Needed in scripts: `scripts/bare-gate-mac.sh` (lockf on the mini as `mac-build.sh` does, tart clone/run/delete, ssh into the VM). Product
gaps already known:
- The MAC KiCad package is bp.6 (`install.rs`: bp.8 and later have no macOS build): no `--embedded`, so Edit cannot work on a fresh Mac
  at all; this is the first thing the Mac run will show. Rank 1, fork/release lane.
- Not notarized: a downloaded `Backplane.app` is stopped by Gatekeeper (documented); the `curl | sh` path sets no quarantine and is not.

## Covered so far, and next

- [x] install from the release, and from a commit's build
- [x] desktop libraries against the distro's desktop manifest; libraries KiCad's runtime bundles
- [x] first start, setup to the KiCad step, KiCad fetched by itself, timings and sizes
- [x] a real-shaped project; Edit draws the PCB editor; Schematic; back to Board
- [x] a box of KiCad's own the pane does not show (`test/bare/kicad_boxes.py`: a visible top-level window of KiCad's pid that is not one of
  its frames): a FAIL when the pane did not draw (first Edit, or after reopening), a rank-2 paper cut when it did. From bp-win, no GPU: KiCad's
  "Could not use OpenGL, falling back to software rendering" came before any frame and the pane stayed on "Starting KiCad..." (ready, 0 frames, no dialogs)
- [x] an agent's `kicad_ipc` commit shows live and is saved; an agent's comment
- [x] DRC by the runtime's kicad-cli
- [x] export to the current stock KiCad (official packages): loads clean, same ERC/DRC, gerbers and netlist
- [x] DRC correctness: a short inside a zone fill (known-red before bp.12.1)
- [ ] the PCB canvas has a usable size in the pane (first runs: about 80 px wide in a 699 px pane, KiCad's Properties and Appearance panels take the rest)
- [x] BOM panel (Alt+Shift+B) with the board's lines (hub's mgmt.watch data)
- [x] Checks panel, and a click on a DRC row selects its items in KiCad (GetSelection over KiCad's socket)
- [x] Fab panel, Review writes `.backplane/design-review.json`
- [x] Ctrl+; in KiCad opens the comment window; a note posts to the thread
- [x] Gerbers tab draws the `mfg/` set exported with the runtime's kicad-cli
- [x] plan tools through the stand-in agent: propose, route refused, the person approves (`plan.approve`), route goes through
- [ ] footprint and symbol editors, 3D
- [ ] debian:12 and fedora:41 (the manifest step is Ubuntu's; others report every library)
- [ ] Windows (bp-win) and macOS (the Mac mini under `mac-heavy`)
