# Nightly builds

A build of main each night, for people who choose **Settings, Network and updates, Update channel: nightly**. Stable takes releases; beta takes those and the
betas; nightly takes all three and the nightlies (`native/host/src/update/rel.rs`: `channel`, `rank`, `wants`). This is alpha: nightly is for people who
choose it.

## What a nightly is

- Tag `vX.Y.Z-nightly.YYYYMMDD.N`. X.Y.Z is one past the newest `vX.Y.Z` tag or `release/X.Y.Z` branch (a release being cut counts), so during 0.15.3's cut
  the nightlies are `v0.15.4-nightly.*`. A nightly sorts after its version's betas and before the release itself: a nightly hub moves on to the release
  when it comes. Every updater since 0.14 accepts the shape (`tag_ok`).
- Built, signed and packaged by `release.yml` exactly as a release (called by `nightly.yml` with `nightly: true`), telemetry baked in (the server sorts
  the version as nightly), its manifest saying `"channel": "nightly"`.
- Published as a **prerelease of BackplaneWorks/Backplane**, so it is on the releases page (marked Pre-release) and in the one feed every hub reads.
  GitHub's "latest release" (what the README's install line takes) skips prereleases, so a fresh install is still the newest stable.
- Its signed files are named `nightly-manifest.json`, `.sig`, `.keyid`, not `manifest.json*` (`update/run.rs` `NIGHTLY_SIGNED`), and 0.15.4 on read them
  as the manifest. Why: 0.15.0 to 0.15.3 call any prerelease beta. Shown a nightly with the usual names, their beta hubs would offer it, refuse its
  manifest ("the manifest's channel does not match its tag") every hour and never reach the next beta. Without those names they count it as half published
  and pass over it, so they keep getting betas and releases. Stable hubs never look at prereleases.
- Only 0.15.4 on can be on the nightly channel: 0.15.0 to 0.15.3 read a stored "nightly" as beta (0.14's name for it). Someone on 0.15.3 who wants nightlies
  installs a nightly once (its `install.sh`, from the release page), then sets the channel.
- Nothing is built when main has not moved since the last nightly; the newest 14 nightlies are kept, older ones deleted with their tags.
- The tag is made by the workflow's own token, which starts no workflow, so a nightly tag never starts a tag release.

## When and where it builds

`nightly.yml` runs at 09:00 UTC (01:00 Pacific), or by hand from the Actions tab (`force` builds even when main has not moved).

`release.yml`'s build jobs take a runner per target from repo variables, for nightlies only; a tag release always builds on GitHub's images:

| variable | what | unset |
| --- | --- | --- |
| `NIGHTLY_LINUX_RUNNER` | `boat` (the Boat CI app) | ubuntu-22.04 |
| `NIGHTLY_MAC_RUNNER` | a self-hosted runner label | macos-latest |
| `NIGHTLY_WIN_RUNNER` | a self-hosted runner label | windows-latest |

None is set yet. The wall time is Windows's (12 to 25 minutes on GitHub), longer than Linux (7 to 16) and macOS (13 to 18), so a faster Linux or macOS machine
does not finish a nightly sooner while Windows builds on GitHub; bp-win (4 vCPU, 16 GB) is the size of GitHub's Windows runner. Linux moves to Boat once the
Boat CI app is on the org and a Boat build is shown to run on Ubuntu 22.04's glibc (releases are built on 22.04 for that reason).

A self-hosted runner on this public repo must refuse everything but the nightly: a fork's pull request runs its own copy of the workflows. The org is on
GitHub Free, so runner groups cannot be limited to workflows; the runner itself refuses any job whose `GITHUB_WORKFLOW_REF` is not
`BackplaneWorks/Backplane/.github/workflows/nightly.yml@refs/heads/main` from a schedule or workflow_dispatch (a job-started hook, which a pull request
cannot change).

## The checks every night

`scripts/bare-gate-nightly.sh run THREAD`, a user timer at 03:30 (`scripts/bare-gate-nightly.sh install THREAD`; docs/bare-install-gate.md):

1. the bare-install gate on a clean Boat, on the tip of main;
2. once per new nightly: `scripts/nightly-update-gate.sh` on a clean Boat box. The nightly before it is installed by its own `install.sh`, a project and a
   thread are in its home and the channel is nightly; the hub's own updater takes the new one from a loopback copy of the feed (the real signed files, under
   the names they are published with), the guard proves it; then `/hello` names the new version and commit, and the project, thread and channel are still
   there (`test/update/nightly-update.sh`).

A failure of either is a message to THREAD. By hand: `scripts/nightly-update-gate.sh` (the newest nightly from the one before it), or
`--from v0.15.4 --to v0.15.5-nightly.20261120.1`, or `--from run:ID --to run:ID` with two `release.yml` dry runs.
