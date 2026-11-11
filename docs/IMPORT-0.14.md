# Bringing a 0.14 home across

Backplane 0.14 (the Bend hub and its Electron app, tags `v0.14.0`, `v0.14.1`, `main-bend-final`) kept everything in one folder. 0.15 keeps its data in a new home and copies from the old one when the person says so. Code: `native/host/src/import014/` (`plan.rs` decides from the logs, `files.rs` copies, `mod.rs` runs it). Words: `present/onboard.rs` (`import_rows`, `import_note`).

## Where 0.14 kept its data

| System | 0.14 home | 0.14 program | 0.15 home |
| --- | --- | --- | --- |
| Linux | `~/.backplane-bend` | `~/.local/share/backplane` | `~/.backplane/hub` |
| macOS | `~/.backplane-bend` (`/Users/<you>`) | the app | `~/Library/Application Support/Backplane/hub` |
| Windows | `%USERPROFILE%\.backplane-bend` (the desktop app), or `<cygwin root>\home\<user>\.backplane-bend` (the hub-only launcher `Run-Backplane.ps1`, under Cygwin; the root is `BACKPLANE_CYGWIN_ROOT`, `%LOCALAPPDATA%\Backplane\cygwin64` or `C:\cygwin64`) | `%LOCALAPPDATA%\Backplane` | `%APPDATA%\Backplane\hub` |

0.14's desktop app put its home at `os.homedir()/.backplane-bend` on every system (`desktop/main.cjs`). The 0.14 home is never a 0.15 home (`launch::home_problem`). Bots kept their working folders in `<home>/bots/<id>`. 0.15 keeps them in `~/.backplane/bots/<id>`.

What is in a 0.14 home:

| Path | What | 0.15 |
| --- | --- | --- |
| `events.jsonl` | the log: projects, threads and their history, bots, memory, rooms, routines, settings, tasks, push registrations | the same format (`store.rs`), filtered by `plan.rs` |
| `devices-v1.json` | paired phones and desktops (token hashes) | added to this hub's registry |
| `providers/accounts-v1/` | Claude and Codex account profiles: their stores, logins and agent session history | copied; logins only with keys |
| `secrets/vault.json` + key | the vault (keyring, keychain, or `vault.key`) | with keys: opened with 0.14's own key, sealed again here |
| `secrets/openai.key`, `secrets/hooks/` | the voice key, webhook secrets | with keys |
| `attachments/`, `message-images/` | files and images in messages | copied |
| `bots/<id>/` | bots' working folders | copied, unless over 2 GB |
| `store.db*`, `live-checkpoints/`, `*.cache`, `browser-profile/`, `cef-profile/`, `electron-profile/` | 0.14's own indexes, caches and browser profiles | not used: 0.15 rebuilds its own |
| `mail/`, `work/`, `far/`, `deploy/`, `updates/`, `plot/`, `shots/`, `exports/`, `voice/`, `kicad/` | the mailroom's mail, background work logs, linked-machine mirrors, 0.14 updates, plots, models, KiCad builds | not copied; a KiCad left under `~/.backplane-bend/kicad` is never used (a stale fork there must not win over the hub's own install): install Backplane KiCad from Settings |
| `secrets/peers/`, `private-pairing/` | links to other machines | not copied: link again |
| `secrets/google.json` | the Google connection | not in 0.15 |

## What comes across

- **Projects.** The ones not removed, with their folders as they were. A project folder, and its git origin, stays where it is. Removed projects and their threads stay behind.
- **Threads, with all their history.** The person's threads in those projects and each bot's own chat. They keep their messages, activity, settle and archive marks, worktrees, models, todos, agent session ids and subthread links. They arrive idle: a past conversation that the next message continues. When the account profile came across too, the agent's session continues.
- **Tasks between imported threads.** A task still running in 0.14 arrives as interrupted.
- **Bots.** With their memory and their Space. Routines arrive switched off, so none fires twice while 0.14 still runs. Webhooks come only with keys. The uberbot comes when this home has none.
- **Rooms.** With their posts.
- **Settings that still mean something.** Agent and model choices, theme and sidebar, notifications, the voice device, thread summaries, effort and speed, task board and rules, project icons and fab settings. This home's own settings win.
- **Paired devices and push registrations.** Each live 0.14 device's token hash joins this hub's registry, so a phone paired with 0.14 is let in without pairing again. Revoked devices and dropped push tokens stay behind.
- **Account profiles.** The stores and session history always come. Without keys, the logins (`.credentials.json`, `auth.json`) stay behind and the profile asks for a sign-in. On macOS, Claude Code keeps its login in the keychain, so the person signs in again either way.
- **With keys:** the vault's secrets, the voice key, webhook secrets and the logins.

## What does not, and why

- **The mailroom's run threads and mail** (`run…`, `mailcoord-…`, MailPosted, RunSet, ConvSet, DecisionLogged, `mail.*`). They were bot mail's working state. A copy would start delivering again. On the person's machine these are 12,644 of 13,736 threads.
- **Turn and queue state, asks, wakes, locks, usage meters, liveness, alerts, allowances** (TurnChanged, PendingChanged, TurnQueued, Queue…, AskOpened/Closed, WakeSet/Dropped, LockChanged, UsageSet, TurnUsage, WorkSet, ThreadWoke, BotWoke, Routine fired/slot/plan/done). They describe 0.14's running processes, and importing them would resume or wake things.
- **Links to other machines** (PeerSet, `secrets/peers`). Link them again in Settings, Devices.
- **0.14's self-deploys** (DeployAsked/DeployBuilt), its `machine.id`, time zone check and onboarding marks.
- **Deleted threads, removed projects, deleted bots, rooms and routines.**
- **Composer drafts** (`drafts/`) and `CtxNoted` lines: 0.15 keeps drafts per client, and `CtxNoted` has no meaning here.
- **The Google connection:** not part of 0.15.
- **A bot folder over 2 GB:** it stays where it is, and the note names its path. Its threads and memory still come.

## Safety

- The old home is only read. No file in it is changed, moved or deleted. If 0.14 is still running, the import reads its log up to the last whole line, and a later import brings the rest.
- Nothing is written to the new home until the person presses Import. Counting reads only.
- **Re-runs are safe.** A line that comes across is the 0.14 line itself, cleaned as the store reads it. The few rebuilt lines (a routine switched off, a running task interrupted, a setting's last value) come out the same on every run. A line already in the new log is never written again. So a second import adds nothing, a cut-off import finishes where it stopped, and an import after 0.14 kept working adds only what is new. Files are never copied over a file already there. Profiles, devices and vault names already present are skipped.
- An id this home already uses for something else (a clash) stays behind, with a note.
- **Quiet writes.** The lines go in through `Hub::import_lines`: one append and fsync per chunk, folded into the model and the presenter. No transform or observer sees them, so the import triggers no phone push, no alert, no feed to a linked machine and no policy reaction. The policies hold their batches during the import and fold the log again afterwards, as at a start (`statehost::refold_begin`/`refold_end`).

## How it is started

- **Setup:** the step "Bring your 0.14 projects across?" after "Where your data is kept". It appears only on a desktop that has 0.14 data. Count reads the old log, then Import, or Import with keys.
- **Settings, Import from 0.14:** the same rows, at any time.
- **RPC:** `import.v014` with `{op: "scan" | "run", secrets: bool}`. It answers at once, and progress and the result go to the hub info key `import.v014`, as JSON text. The last result is also the setting `import.v014.done`.
- **HTTP**, with the home's token: `GET /import/0.14` returns what would come (or the last result), and `GET /import/0.14/run[?secrets=1]` imports and answers the result.
- **Where it looks:** `BACKPLANE_IMPORT_FROM`, else each place in the table above that holds a 0.14 home, the one whose log was written last. A home under the temp folder (tests) looks only at the variable. A copy of a real log (`BACKPLANE_COPY=1`) imports nothing.
- **The 0.14 to 0.15 update path:** 0.14.1's updater installs 0.15 and its shim starts the 0.15 guard on the new default home with `BACKPLANE_IMPORT_FROM` set to the home 0.14 ran with. The person then sees the setup step, as on a fresh install.

## Progress

A long import shows itself the whole time (`import014/progress.rs`). The steps are reading the 0.14 log (by bytes), copying files, writing the lines (events, projects n/N, threads n/N, the thread now) and finishing. The estimate is the current step's measured rate over its last ten seconds, and none until a second of work. A ticker publishes the state every second, even in the middle of a step:
- the info key `import.v014` (the desktop: an overlay with the bar, counts and the project now, which cannot be closed while it runs, then the summary until Continue);
- data/1 `migrating` frames and `hello.migrating`, with the same card for the phones;
- `migrating` on `/healthz` and `/hello`: numbers only, no names, since they need no credential.

`/healthz` stays `ok` and holds a `Folding` while it runs, so the guard keeps the build on probation and proves it only after the import (remote-op's `plan::holds_proof`). The import never makes the hub answer `hung`. The hub serves throughout: the reading is off the runtime threads, and a client's write waits for at most one 2,000-line chunk.

A failure says what failed, that the 0.14 folder is untouched, and offers Retry, which picks up where it stopped. When the import ends, one push notice tells the phones. After 0.14's Update installs 0.15, the shim sets `BACKPLANE_IMPORT_AUTO=1` and the import begins by itself on every start until it has finished, without keys.

## Tests

- `cargo test import014`: a long import, from a home written by the generator's large mode (`make-v014-home.py OUT --large P T L`) and slowed by `throttle_ms`, publishes progress at least every 2 s, answers a write and a read within 1 s meanwhile, holds a fold on `/healthz` and ends with the right summary; an import after an update runs by itself once. The fixture home `test/fixtures/v014-home` (written by `test/fixtures/make-v014-home.py`, made-up content in 0.14.1's shapes), copied to a temp folder. The tests cover:
  - what comes across and what stays;
  - no observer sees an imported line;
  - files and profiles are copied, and logins only with keys;
  - a second run adds nothing;
  - a run after 0.14 wrote more adds only that;
  - the HTTP routes;
  - finding the home on each system's layout;
  - the old home is byte for byte unchanged.
- `settings::vault::tests::another_homes_vault_is_read_with_its_own_key`, and `present::onboard` for the step and its words.
- On a copy of a real 0.14 home, never the home and never committed: `BP_IMPORT_REAL=<read-only copy> cargo test real_home -- --ignored --nocapture`.
