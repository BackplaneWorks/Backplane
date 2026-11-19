# Release builds on our machines

0.16.0 on: release.yml's macOS and Windows jobs build on our own machines, through self-hosted runners that GitHub dispatches. GitHub's
machines only build Linux (x64, arm64) and Android, which are quick and run in parallel, and run `publish` (ubuntu-latest), which signs manifest.json. The
release key (`BACKPLANE_RELEASE_KEY`, `BACKPLANE_RELEASE_KEY_ID`), the Android keystore and the telemetry key (`BACKPLANE_TELEMETRY_KID`/`KEY`, baked into the
hub on a tag push) are repo secrets only. A self-hosted job gets them from GitHub just as a hosted one does, so a hub built on the mini or on bp-win is the
same hub that GitHub would build.

## Routing (repo variables)

| variable | jobs | value | unset |
| --- | --- | --- | --- |
| `RELEASE_MAC_RUNNER` | build and smoke darwin-arm64 | `bp-mac` (the Mac mini, `~/bp-runner`) | macos-latest |
| `RELEASE_WIN_RUNNER` | build and smoke windows-x64 | `bp-win-nightly` (bp-win, `C:\actions-runner`) | windows-latest |

A `RELEASE_*` variable applies to every run while it is set: tags, dry runs and nightlies. It comes ahead of the `NIGHTLY_*` ones (docs/nightly.md), which
apply only to nightlies. linux-x64 (ubuntu-22.04, so the archive keeps glibc 2.35), linux-arm64 (ubuntu-22.04-arm; we have no ARM machine) and android
(ubuntu-latest) always run on GitHub's images. **The variables are set only while a release uses them.** Set them before a dry run or a tag push, and delete
them once its jobs are done:

    gh variable set RELEASE_MAC_RUNNER -b bp-mac -R BackplaneWorks/Backplane
    gh variable set RELEASE_WIN_RUNNER -b bp-win-nightly -R BackplaneWorks/Backplane
    gh variable delete RELEASE_MAC_RUNNER -R BackplaneWorks/Backplane; gh variable delete RELEASE_WIN_RUNNER -R BackplaneWorks/Backplane

If a variable is set and its runner is offline, the job waits in the queue (up to 24 hours) and never falls back to GitHub. Start the machine and its runner first.

## Every self-hosted runner refuses all but the release and the nightly

This repo is public. A fork's pull request runs its own copy of the workflows, so it can name any runner's label. The org is on GitHub Free, so runner
groups can't be limited to workflows. Each runner therefore has a job-started hook (`ACTIONS_RUNNER_HOOK_JOB_STARTED` in its `.env`). The hook lives on the
machine, so no workflow or pull request can change it. A non-zero exit fails the job before any of its steps runs. It allows only these jobs from
`BackplaneWorks/Backplane`, judged by `GITHUB_WORKFLOW_REF` and `GITHUB_EVENT_NAME`:

| workflow ref | event |
| --- | --- |
| `.github/workflows/nightly.yml@refs/heads/main` | schedule, workflow_dispatch (it calls release.yml; the ref stays nightly.yml's) |
| `.github/workflows/release.yml@refs/tags/v…` | push (a tag release) |
| `.github/workflows/release.yml@refs/heads/main` | workflow_dispatch (a dry run) |
| `.github/workflows/release.yml@refs/heads/release/<name>` | workflow_dispatch (a release-branch dry run; dispatch needs write access, the same trust as main) |

It refuses everything else: pull_request and pull_request_target (whatever their branch), other workflows, other branches, a ref with anything but
`[A-Za-z0-9._/@-]` in it. The rules live in `scripts/runner-guard.sh` (macOS: copied to `~/bp-runner/runner-guard.sh`) and in `scripts/runner-guard.ps1`
(bp-win: `C:\actions-runner\runner-guard.ps1`). Change both together and copy them back to the machines. Check one by hand with
`env -i GITHUB_REPOSITORY=BackplaneWorks/Backplane GITHUB_EVENT_NAME=pull_request GITHUB_WORKFLOW_REF=BackplaneWorks/Backplane/.github/workflows/release.yml@refs/pull/1/merge sh scripts/runner-guard.sh`
(this must exit 1).

## The machines

**bp-win** (Azure VM, 4 vCPU, 16 GB). Runner `bp-win` (label `bp-win-nightly`) in `C:\actions-runner`, started at logon by the scheduled task
`GitHubActionsRunner`. The VM is deallocated every day at 07:00 UTC; start it with `az vm start -g BP-WIN-RG -n bp-win` and give it a minute. Its Windows
Update active hours are 05:00 to 23:00 UTC: outside them an update can restart it mid-build, as one nightly found. The job then fails, and its run is started
again. It has no Chocolatey; the build takes CMake and Ninja from Visual Studio, and Git's `usr\bin` goes ahead of System32 (release.yml).

**The Mac mini** (hs-mac-mini, macOS 26). Runner `bp-mac-mini` (label `bp-mac`) in `~/bp-runner`. It is **not** a service: start it with `./run.sh` only while
holding the `mac-heavy` lock, and stop it once the release's darwin jobs are done. Its `.env` gives it a home of its own (`HOME`, `RUSTUP_HOME`,
`CARGO_HOME` inside the folder), `BACKPLANE_KICAD_NO_SYSTEM=1`, its own `KICAD_CONFIG_HOME` and `KICAD_DOCUMENTS_HOME`, and the `HOMEBREW_NO_*` variables, so
a build never reads or changes the person's settings. A build needs 15 to 20 GB free; check `df -h ~` first. The macOS floor check in the build (every
Mach-O at macOS 13.0 or lower) keeps a build on macOS 26 installable on older Macs.
