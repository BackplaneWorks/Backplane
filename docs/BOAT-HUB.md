# The always-on hub on Boat

A small hub that keeps running when the desktop sleeps or is off: the uberbot (Bub) and its routines and reminders, the bots, phone push,
Google, webhooks (the ring) and phone access. The desktop stays the machine for hardware and code: KiCad, local files, builds.
Script: `scripts/boat-hub.sh` (run on the desktop; its header lists the commands).

## The box

- Sandbox `bp-hub` (Nov 10: `bx_479dqxby`), type **small** (2 vCPU, 4 GB, half rate), made with `--no-auto-stop` and `--no-env`: no TTL,
  no idle stop, no archive date (`boat info`: `ttlSeconds` and `archiveAfter` null), and none of the Boat account's stored logins on it.
  Small is enough: the dry run below folds the desktop's whole log (584k events) in 4.4 s at about 205 MB, with 2.7 GB of the 4 GB free.
- Layout under `/home/user/bp` is the guard's builds folder (docs/deploy.md): `builds/<id>/` (hub, guard, voice, assets, `COMMIT`, `EXPECT`),
  `CURRENT`, `backplane` (the guard, copied out of the build), `home/` (the box hub's own home), `hub.log`.
- `bp-hub.service` (system unit, user `user`): `backplane hub run --home ~/bp/home --port 3797 --host 127.0.0.1 --builds ~/bp -- --tailnet-only`
  with `BACKPLANE_HTTPS_PORT=443` (the box is its own tailnet node), `BACKPLANE_NO_UPDATE=1` (it follows main builds, below, not releases),
  `Restart=always`, enabled at boot. Checked Nov 10: a killed hub is restarted by the guard, a killed guard by systemd.
- `bp-hub-health.timer`: every 2 minutes `/healthz` on loopback; three misses in a row restart the service (`~/bp/health.log`).
- Tailnet: `tailscale` is installed; `scripts/boat-hub.sh tailnet` (through `secret_run` with `files {TS_KEY_FILE: tailscale-boat-hub}`)
  brings it up as `boat-hub` with `--operator=user` (so the hub can `tailscale serve`). The key reaches the box on stdin and is deleted after use.

## Updates

Each main build the desktop publishes is pushed by `main-build.sh` (`boat-hub.sh update --quiet`, in the background, log
`~/.backplane-gpui-dev/boat-hub-update.log`, off with `BP_BOAT_HUB_UPDATE=0`): copied to `builds/<id>/` and handed to the box's guard
(`backplane hub update <id>`): probation, identity check against `EXPECT`, and one failure goes back to the build before (checked Nov 10).
Three builds are kept. While the desktop is off nothing new is built, so nothing is missed.

## Cost

`boat-hub.sh cost --save` (run by `update` and `status`) measures from the desktop, which has the Boat login, and writes
`~/.config/backplane/boat-hub/cost.json` here and on the box. Each hub reads it every 5 minutes (`update/boathub.rs`, info `boathub.cost`) and
Settings, Network shows "Always-on hub: bp-hub (small)" with the rate, the month around the clock, hours so far and plan hours left
(`present::settings::boat_hub_rows`). It warns when the plan's hours left are under twice what the box needs to the end of the period (or a tenth
of the plan), says "run out" under what it needs, and says when the measurement is over a day old. Nov 10: $0.018/h, $12.96 a month, 2,418 of
2,778 plan hours left until Dec 3; the box needs about 280 of them to Dec 3.

## What runs where

| | Boat hub | desktop hub |
|---|---|---|
| Bub's conversation, unpinned routines, reminders | holds the uberbot lease | replica (memory, space, routines converge) |
| other bots, rooms, webhooks, Google, push to phones | yes | stopped there after the cutover |
| hardware and code threads, KiCad, builds | no | yes |
| phones | paired here | can stay paired too, for desktop threads |

One owner for each thing: the uberbot's conversation and routines go by the lease (`docs/uber.md`: one holder per term, a routine slot runs
once); everything else is moved, not copied, and switched off at the old place in the same step.

Dispatch to the desktop: Bub uses `thread_launch {machine}` / `thread_send` over the link. When the desktop is away, a send and (new, Nov 10) a
launch are held in the Boat hub's far outbox (`net/farsend.rs`, `<home>/far-sends.json`, kept across restarts) and go the moment it is back;
a held launch carries an id (`_launch`) so it starts once however often it is posted (`agents/launch.rs`, setting `far.launched.<id>`).

## Dry run (done, Nov 10)

`boat-hub.sh dry-run`: a copy of the desktop home (without secrets, providers, voice, browser profile, checkpoints, devices) on the box, run with
`BACKPLANE_COPY=1`, no tailnet, stand-in agent folder only, port 3899; `/healthz` ok at 5 s, folded at 4.4 s, 205 MB; then stopped and deleted.

## The cutover (only after the person says yes)

0. Backup: stop nothing; copy the desktop home (`~/.backplane-gpui-fresh/home`) to `~/.backplane-gpui-fresh/backup-<date>/` with
   `~/.config/backplane/apns*`, and `boat snapshot bx_479dqxby bp-hub-before`. Rollback = the steps below in reverse (each is one setting or
   one service).
1. `boat-hub.sh tailnet`, then `boat-hub.sh start`: the box hub starts on its empty home as `boat-hub.<tailnet>.ts.net`.
2. Link the machines (Settings, Machines: an invite from the box, joined from the desktop). The desktop offers the uberbot; the box joins it
   (`bots.uber_join`): its replica gets Bub's memory, space, page, persona and routines by sync.
3. On the box, "Make this the home" (`bots.uber_home`): the box takes the lease one term on; the desktop steps down at the next exchange and
   stops firing Bub's routines; Bub's last 20 entries go ahead of its next message there. Reminders follow (`Ub.fires`).
4. Copy to the box, then switch off on the desktop: the provider accounts Bub runs on (`providers/`, the Claude CLI installed there), the vault
   (`secrets/vault.json`, `vault.key`: Google tokens `hub.google.*` and the secrets), APNs (`~/.config/backplane/apns.env`, `apns/`, the
   `push.apns.*` settings), the webhook tokens and the ring's URL (now `https://boat-hub.<tailnet>.ts.net/...`), the other bots and rooms.
   On the desktop: Google signed out of the hub there (the box holds the refresh token), APNs config set aside, webhooks revoked.
5. Phones: Add a hub with the box's pairing link (Settings, Devices on the box); push then comes from there. The desktop pairing can stay.
6. Check: a reminder fires once, a push arrives, the ring's webhook lands on the box, Bub launches a desktop thread, and, with the desktop
   asleep, a held launch starts when it wakes.

Rollback: "Make this the home" on the desktop (lease back), `boat-hub.sh stop`, put back APNs/vault/webhooks from the backup.
