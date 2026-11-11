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

## State (Nov 10, prep done)

`bp-hub` is on the tailnet as `boat-hub` (100.103.2.108); `bp-hub.service` is enabled and runs an empty hub (no lease, no link, nothing of
Bub's); `https://boat-hub.tail9e0cf2.ts.net/healthz` answers `ok` from the desktop over the tailnet (`tailscale serve` 443 -> 127.0.0.1:3797).

## The ring's URL

`bots/ring.rs` derives a hook's URL from the hub's own `tailscale serve` address, so each hub shows its own (the Boat's has no port:
`https://boat-hub.tail9e0cf2.ts.net/hook/<id>`). The ring is owned by one hub. At the cutover the hook record and its token move to the Boat
(the Authorization header is unchanged), the desktop revokes its hook and gets the setting `ring.home` (`{"url","machine"}`), and its Ring page
(desktop and phones) then shows the owner's URL instead of Set up. The person edits the URL in the Pebble app once and sends the test event.

## The cutover as run (Nov 10)

The Rust hub does not sync an uberbot between machines (`uber.sync` answers 501: `net/rpc.rs`), so the lease plan below could not run.
Bub moved whole instead, as the one owner (`scripts/boat-hub-carry.py`): everything of the desktop's log outside its projects (94k lines:
Bub's conversation, its run, coordination and routine threads, the five other bots, 13 rooms, memories, routines and slots, mail, triage, the
push book), ids kept; machine, peer, uber, account, session, worktree, lock and device lines left out. Order: the desktop log copied, at once
the uberbot demoted and every bot and room deleted on the desktop (RPCs; their history stays in its log), then the Boat hub stopped, the carry
appended, the bots' folders, mail, message images, ring key, vault and APNs config copied, the hub started (`push.apns.key` set to the box path).
The ring was set up on the Boat (`ring.setup`: Pebble, hook `h1794356130-94959`); the desktop never had one. Parakeet v2 is installed there.
Rollback: `scripts/boat-hub-cutover.sh rollback` (the Boat hub stopped; with the desktop hub stopped, its own last BotSet/RoomSet/UberSet lines
appended again; then the app started).

## Rolled back (Nov 10, 19:34)

Within the hour the person found Bub gone from the app: the desktop app and the phone talk to the desktop hub, and neither shows another
machine's bots (`present/screen.rs` `bots` drops `<link>~<id>` rows; data/1 `bots.list` reads only this hub's bots). So the move was rolled back:
the Boat hub stopped and disabled, then, with the desktop hub stopped by a detached job (a turn is a child of the hub), the desktop's own last
BotSet/RoomSet/UberSet lines appended again with `at` now (each bot's thread taken from its last BotSet that named one: Bub's last edit had
`thread: ""`), the ring lane's new #ring-requests kept with Bub added, and the hub started as `fresh.sh` starts it. Nothing was lost: the Boat
hub never ran a turn (no Claude sign-in there). Its home, with the carry and a ring hook of its own, is kept and stopped.

Before moving Bub again, one of (recommended first):
- **B. The clients pair with the Boat hub too** (no code: the desktop app's hub picker and the phones' Settings both have Add a hub). Bub,
  the bots and rooms are on boat-hub; the desktop's projects and threads show there as another machine's (`mark`); hardware work stays on
  the desktop hub in the same app. Plus the Boat signed in to Claude. About an hour with the person, then the same cutover.
- **A. One hub's clients show and drive another machine's bots and rooms** (`<link>~<id>`): the sidebar and the phones' bot list (screen.rs
  `bots`, bots/api.rs `directory`), a far bot's chat, space, memory and routines, room read and post over the link. Hub, GPUI, iOS, Android
  and e2e: about 3 to 5 days.
- **C. Port `uber.sync`** (`net/rpc.rs` answers 501; the ops, fold, derive and lease laws are in `bots/uber.rs`): the exchange, offer and join,
  heartbeat, takeover, handoff and routine gating, with an e2e of two hubs. About 2 to 3 days, and it covers Bub only: the other bots,
  rooms, hooks and Google still need one owner, which brings back A or B.

## The cutover as first planned (not run)

Run as `scripts/boat-hub-cutover.sh <step>` in this order: `backup`, `link`, `link-join CODE`, `approve`, `join`, `home`, `carry`, `retire`, `check`
(`rollback` undoes it). The carry is `scripts/boat-hub-carry.py`: from a copy of the desktop log, every live bot but the uberbot (with its thread,
messages, memories, routines, settings), every live room with its posts and every live hook, ids kept, the desktop uberbot's id replaced by the
Boat replica's; appended to the Boat log with its hub stopped. Dry run Nov 10: 230 lines (5 bots, 13 rooms) on a copy of the Boat home; a client
of that copy sees all of them. RPCs go through `scripts/hub-rpc.mjs` (the hub's /ws, CBOR). The Boat signs in to Claude with its own session
(Settings, Agents on the Boat hub): the desktop's account files are not copied, since two machines refreshing one OAuth session would log one
out. APNs is set on both hubs: each pushes for its own threads, and Bub's pushes come only from the lease holder. Google's refresh token goes to
the Boat with the vault; the desktop's copy is left unused rather than signed out (sign-out revokes it at Google).

0. Backup: stop nothing; copy the desktop home (`~/.backplane-gpui-fresh/home`) to `~/.backplane-gpui-fresh/backup-<date>/` with
   `~/.config/backplane/apns*`, and `boat snapshot bx_479dqxby bp-hub-before`. Rollback = the steps below in reverse (each is one setting or
   one service).
1. `boat-hub.sh tailnet`, then `boat-hub.sh start`: the box hub starts on its empty home as `boat-hub.<tailnet>.ts.net`.
2. Link the machines: on the box `BACKPLANE_HOME=~/bp/home ~/bp/backplane pair --machine`, on the desktop
   `backplane pair --join https://boat-hub.tail9e0cf2.ts.net CODE`, then `pair --requests` / `--approve ID` on the box after the check codes match. The desktop offers the uberbot; the box joins it
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
