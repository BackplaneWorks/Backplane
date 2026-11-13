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
| Install | `curl -fsSL …/install.sh \| sh` as `person` (release), or `BACKPLANE_ARCHIVE=… sh install.sh` (commit); with no curl on the desktop, the `wget -qO- … \| sh` the docs give | `backplane-app` installed; the installer's words |
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

## Covered so far, and next

- [x] install from the release, and from a commit's build
- [x] desktop libraries against the distro's desktop manifest; libraries KiCad's runtime bundles
- [x] first start, setup to the KiCad step, KiCad fetched by itself, timings and sizes
- [x] a real-shaped project; Edit draws the PCB editor; Schematic; back to Board
- [x] an agent's `kicad_ipc` commit shows live and is saved; an agent's comment
- [x] DRC by the runtime's kicad-cli
- [x] export to the current stock KiCad (official packages): loads clean, same ERC/DRC, gerbers and netlist
- [ ] the person's own comment from KiCad (the comment window, lane C)
- [ ] the PCB canvas has a usable size in the pane (first runs: about 80 px wide in a 699 px pane, KiCad's Properties and Appearance panels take the rest)
- [ ] the DRC/ERC panel (lane E), BOM (lane D), floorplanner plan in the journey (the sample passes stock_compat), review, Gerber viewer
- [ ] footprint and symbol editors, 3D
- [ ] debian:12 and fedora:41 (the manifest step is Ubuntu's; others report every library)
- [ ] Windows (bp-win) and macOS (the Mac mini under `mac-heavy`)
