# Updating and restarting a hub

A hub moves to a new build in one way only: its updater (`native/host/src/update/`) finds and stages a signed release, and the guard (`native/guard`, the `backplane` program) swaps to it, checks it, and goes back when it fails. The Bend hub's `deploy_main` / `deploy_restart` path (build origin/main in place, the `deploy.ship` and `deploy.wait` settings) is gone: old logs' `DeployAsked` and `DeployBuilt` events still fold, and set nothing (`state/fam.rs`).

Dev machines do not use this: `scripts/dev-build.sh` builds `native/host` and `native/gpui` and `scripts/dev-cut.sh` publishes them; `scripts/fresh.sh` sets `BACKPLANE_NO_UPDATE=1`.

## Installing: the guard from the first start

A release archive (`backplane-<tag>-<target>.tar.gz`) holds `backplane-hub`, `backplane-gpui`, `backplane` (the guard), `assets/` and `COMMIT`; `release.yml` builds the guard for every target and fails the job when it is missing. `install.sh` (`deploy/install.sh.in`, filled in by `release.yml` with the version and each archive's sha256, published beside the archives) checks the archive and lays it out as the guard expects, under `BACKPLANE_PREFIX` (`~/.local/share/backplane`):

- `builds/<tag>/`: the unpacked release, made by one rename;
- `CURRENT`: the build that runs;
- `backplane`: the guard, copied out of the build, so an update never replaces the program running it; linked as `~/.local/bin/backplane`;
- `~/.local/bin/backplane-app`: the desktop app of `CURRENT`.

Nothing starts the hub directly. The desktop app finds this layout around itself (`launch::guarded_install`) and starts its home's hub with `backplane hub run --home <home> --port <port> --builds <prefix> --default`; a build tree with no such layout still starts `backplane-hub` beside it. With no desktop app (`BACKPLANE_NO_DESKTOP_APP=1`), or with `BACKPLANE_START=1`, the installer starts the guard itself for `BACKPLANE_HOME` on `BACKPLANE_PORT`. Run again while that home's guard runs, the installer asks it `backplane hub update <tag>`: the new build on probation.

## Who updates itself

Only a hub the guard runs from a builds folder (`BACKPLANE_BUILDS`: `builds/<tag>/`, `CURRENT`, `BAD`, `.pins/`). Otherwise the updater says so and does nothing:

| state | when |
|---|---|
| `package` | no `BACKPLANE_BUILDS`: a package install or a plain folder; its package manager updates it |
| `off` | `BACKPLANE_NO_UPDATE=1`, or a copy of a real log (`copy::on`) |
| `idle` / `checking` / `downloading` / `waiting` / `installing` / `failed` | a managed hub |

The feed is GitHub Releases (`run::FEED`); only the environment (`BACKPLANE_UPDATE_FEED`) can change it, never a setting, because any paired client can write settings. The channel is the setting `update.channel`: `stable` or `beta` (0.14's stored `nightly` reads as `beta`).

## What triggers an update

- The hourly check (`Hourly`, `svc::due`), on every managed hub.
- `update.check` from any client: desktop Settings > Network's Check now, either phone's Settings > Updates (`update.check` over data/1), or a linked machine over the link. It looks now and installs what it finds.
- `app.update`: the same, as an explicit wish to install.

Every client gets the same answer, `update.status` → `{current, version, channel, state, note, checked, line, canCheck}`, where `line` is the sentence to show (`present::settings::update_line`). The clients draw it and decide nothing. Phones set the channel with `update.channel {channel}` (docs/PHONE-DATA.md).

**A running turn always holds an install.** When something newer is found while any thread's turn is `running`, the state is `waiting` ("installs when running turns finish") and the install goes ahead at the next pass with no turn running. Whoever asked, from wherever, nothing cuts a turn.

## Finding and staging (`update/run.rs`, `update/rel.rs`)

In this order, and each step must pass before the next one starts:

1. Fetch the release list. A release counts only when it carries all three signed files (`manifest.json`, `.sig`, `.keyid`), which the release workflow uploads last. Drafts and tags in the guard's `BAD` are skipped, so a build that failed here is never offered again; the next higher tag is.
2. `rel::offer`: the newest tag on the channel that is newer than this hub's version. Tags have exactly the shape `vX.Y.Z`, `vX.Y.Z-beta.N` or `vX.Y.Z-nightly.YYYYMMDD.N`, because a tag becomes a folder name.
3. **Signature.** `manifest.json`'s bytes are checked against `manifest.json.sig` (ed25519), under the key `manifest.json.keyid` names, before they are parsed. One key is trusted: `native/host/src/update/release.pub` (PEM SPKI; the raw key is the DER's last 32 bytes) under the ID in `release.keyid` (`6c899f869be656d3`, the first 16 hex of SHA-256 over the raw key), compiled in. The signature is the raw 64 bytes over the exact manifest bytes. A key ID that does not match the key means the key is not trusted at all. There is no second key and no way to add one in a default or release build; the private key is the repo secret `BACKPLANE_RELEASE_KEY` (`update/README.md`).
4. The manifest names this machine's asset (`linux-x64`, `linux-arm64`, `darwin-arm64`, `windows-x64`) with its size and sha256 (2 GiB at most).
5. Fetch the archive into scratch beside `builds/`, check its size and sha256, then list it before writing anything (`run::members_ok`, `tar -tzf` and `tar -tvzf`, which must agree line for line). Only plain files and folders get through: no absolute path, no `..`, no symlink, no hard link, no device. Unpack with `--no-same-owner --no-same-permissions`, then check the tree again: there must be a runnable `backplane-hub`, and no link of either kind (a file with more than one link counts as a hard link).
6. Write `EXPECT` = `{"version","commit"}` from the verified manifest. This is written by the updater after the tree check, so the archive cannot choose what its build is held to.
7. One rename makes `builds/<tag>`, whole or not at all. Scratch is removed however the steps end.
8. Write `update/pending` (`<tag>`) and the guard request `guard/request` = `{action:"update", build:<tag>, from:"local"}` (beside it, then renamed). A request already waiting, perhaps a restart the person asked for, is refused, never replaced. While the update is pending and the guard has neither moved to it nor marked it bad, a restart of the old build reads it as "not yet" (`run::Outcome::Waiting`): the state stays `installing`, and the pending line stays.

The request is `from:"local"` because it is the hub's own. A phone or another machine asks the hub (`app.update`, `update.check`), never the guard's `hub.control update` directly: the guard refuses a remote switch to a build that has not yet run here with the control route, since going there could lock the asker out.

## Probation and the health check (guard)

The guard runs the new build on probation, with the build it left kept as the way back.

- It probes `GET /healthz`. A hub that is `starting` or still `folding` history is not yet proven; one that stops answering for `misses` probes in a row, or stalls during its start, is killed and counts as failed.
- **Identity.** On the first well probe the guard compares the hub's `version` and `commit` with `builds/<tag>/EXPECT`. They are the same values `/hello` reports (`{"backplane": version, "commit": ...}` in CBOR); `/healthz` carries them in JSON only to a caller with a credential the hub accepts (the guard sends the home token); being on loopback is not enough, because `tailscale serve` arrives on loopback. Commits match as a prefix of at least 7 characters (the manifest has the full SHA, the build the short one), and `unknown` never matches. A mismatch is a failure: "answered as v/c, expected v/c". It usually means the old hub still holds the port, or the archive held the wrong binary. With no `EXPECT` there is no comparison. The commit is compiled into the hub (`option_env!("BACKPLANE_COMMIT")`, set by `release.yml` and `scripts/dev-build.sh`); the runtime environment is only a fallback for other builds and the guard does not set it, so the comparison proves something.
- **Proven**: the identity matches, then the hub stays well for 60 s after its folds reach 0. Then the guard adds the build to `guard/good`, publishes it as `CURRENT` and pins it with up to two predecessors (`.pins/`), so pruning keeps them.

The guard takes one request at a time: a second one gets 409 unless the first is over 30 s old. The hub's own requests go through `guard::control(home, action, build, "local")`, which skips the remote limits.

## When and how it rolls back

- **During probation, automatically.** The first failure goes straight back to the previous build, with no retries (`plan::next_on`, `probation`). A failure is a crash or panic, a stalled start, missed health checks, or an identity mismatch. The guard writes the tag to `BAD` with the reason, sets checkpoints aside (never deletes them), starts the previous build, and records an incident.
- After the restart, the hub works out how its update ended (`run::outcome`): `Updated to <tag>.` (it then prunes old builds), or `failed` with "<tag> failed its health check, so this build stayed: <why>". Every client shows that line.
- **After probation**, a crash follows the normal ladder: the same build again 3 times within 10 minutes, then the newest good build, then safe mode, then rescue. Exit 75 (port or home lock held, disk full) and exit 78 (refused: a copy gate, a bad flag) are the machine's fault and never mark a build bad.
- **By hand**: `backplane hub rollback [--now]` at the machine, or `POST /hub/control {action:"rollback"}` from a phone or a linked machine with the hub's credential. Either marks the running build bad and goes to the newest good build that is not bad. From a remote client the target must have the control route, so the asker can still reach the hub afterwards. Remote requests are limited to 3 in 5 minutes, and a remote client can never stop the hub. `backplane hub use BUILD` takes a bad mark off and runs that build.
- **Rescue**: when nothing starts, the guard itself answers on the hub's port. `/healthz` returns 503 `down`, and `/hub/status` and `/hub/control` take the same credentials as the hub, so a phone can still restart or roll back.

## The test key (feature `e2e-trust`)

The update E2E (`test/update/`) needs releases it can sign, and it never signs one with the real key. A hub built with `cargo build --features e2e-trust` also trusts one public key (PEM) from the file `BACKPLANE_E2E_TRUST_KEY` names, and only for releases below `0.1.0` (`rel::E2E_CEILING`), so a leaked test release would offer nothing. The E2E makes a fresh keypair in its private home for each run and deletes it at the end; no private key is committed. Default and release builds have none of this code: the feature's code carries the string `backplane-e2e-trust-enabled`, and `release.yml` fails a build whose `backplane-hub` contains it. A unit test shows a default build refuses a manifest signed by such a key.

## Checking an update from far away

The client that asked keeps polling `/hello` (never `/healthz`, which tells a caller without a credential nothing of the build). The update is done when it answers with the new version and commit. When it still answers with the old pair after the guard's boot and stable time, and `update.status` says `failed` with the tag, the update was rolled back. `/hub/status` (paired) shows the guard's view: build, commit, good, bad, last request and recent incidents.

Tests: `update::` (`cargo test`, no network), the guard's `plan` and `run` tests, and `test/guard/faults.sh` on a private home and port (`native/guard`). End to end: `test/update/feed.sh BIN_DIR` (a real guard and an `e2e-trust` hub against a signed feed on loopback: good, panic, die, mismatch, older, wrongkey, ceiling, healthz, and with PHONE the Android app's Check now, Restart hub and Roll back), and `test/update/clients.sh HUB CLIENT...` (a hub held on this desktop, backplane or the Mac, driven from the GPUI client on another machine or the Pixel), and `test/update/install-e2e.sh FIRST NEXT BROKEN` (three release dry runs: install.sh lays one out and starts the guard, the next updates it from a loopback feed, a broken one is rolled back).
