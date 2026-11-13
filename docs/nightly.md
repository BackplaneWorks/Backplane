# Nightly builds

A build of main each night, for people who choose **Settings, Update channel, nightly**. Stable takes releases; beta takes those and the betas; nightly takes
all three and the nightlies (`native/host/src/update/rel.rs`: `channel`, `rank`, `wants`).

## What a nightly is

- Tag `vX.Y.Z-nightly.YYYYMMDD.N`, Z one past the newest release (`v0.15.4-nightly.20261114.1` after 0.15.3): it sorts after that release's betas and before the
  release itself, so a nightly hub moves on to the release when it comes. Every updater since 0.14 accepts the shape (`tag_ok`).
- Built, signed and packaged by `release.yml` exactly as a release (called by `nightly.yml` with `nightly: true`), its manifest saying `"channel": "nightly"`,
  telemetry baked in (the server sorts the version as nightly).
- Published as a prerelease in **its own repo** (`vars.NIGHTLY_REPO`, default `BackplaneWorks/backplane-nightly`), never in this repo's releases:
  0.15.0 to 0.15.3 call any prerelease beta, so a nightly in this repo's list would be offered to their beta hubs, refused by them (its manifest says
  nightly) and would hold up their betas. A hub on the nightly channel reads both lists (`update/run.rs` `NIGHTLY_FEED`; `BACKPLANE_UPDATE_NIGHTLY_FEED`
  replaces it, `-` reads none); when the nightly list cannot be read it still gets releases and betas.
- Only from 0.15.4: an older hub set to nightly reads it as beta (that was 0.14's name for it) and never reads the nightly repo.
- Nothing is built when main has not moved since the last nightly; the newest 14 are kept.

## Where it builds

`release.yml`'s build jobs take a runner per target from repo variables, for nightlies only (a tag release always builds on GitHub's images):

| variable | what | unset |
| --- | --- | --- |
| `NIGHTLY_LINUX_RUNNER` | `boat` (Boat CI app: `boat ci connect --org`) | ubuntu-22.04 |
| `NIGHTLY_MAC_RUNNER` | the Mac mini's self-hosted runner label | macos-latest |
| `NIGHTLY_WIN_RUNNER` | a self-hosted Windows runner label (bp-win) | windows-latest |

Signing and publishing stay on GitHub (the key is a repo secret). linux-arm64 and Android stay on GitHub.

## The checks every night

`scripts/bare-gate-nightly.sh run THREAD` (a user timer at 03:30 the person installs; docs/bare-install-gate.md):

1. the bare-install gate on a clean Boat, on the tip of main;
2. when a new nightly is published: `scripts/nightly-update-gate.sh` on a clean Boat box: the nightly before it installed by its own `install.sh`, a project
   and a thread in its home, the channel set to nightly; the hub's own updater takes the new one (a loopback copy of both feeds, the real signed files), the
   guard proves it; then `/hello` names the new version and commit and the project, thread and channel are still there (`test/update/nightly-update.sh`).

A failure of either is a message to THREAD. By hand: `scripts/nightly-update-gate.sh --from v0.15.4 --to v0.15.5-nightly.20261120.1`, or
`--from run:ID --to run:ID` with two `release.yml` dry runs.

## Turning it on (the person's)

1. Create the public repo `BackplaneWorks/backplane-nightly` with one commit (a README).
2. Add the repo secret `BACKPLANE_NIGHTLY_TOKEN`: a fine-grained token with contents read and write on that repo only.
3. Optional, faster builds: install the Boat CI app on the org and set `NIGHTLY_LINUX_RUNNER=boat`; register the Mac mini as a self-hosted runner (only the
   nightly and release workflows use it: never pull requests) and set `NIGHTLY_MAC_RUNNER`.
4. Uncomment the `schedule:` in `nightly.yml` (`cron: '0 9 * * *'`, 01:00 Pacific).
5. Install the nightly timer: `scripts/bare-gate-nightly.sh install THREAD`.
