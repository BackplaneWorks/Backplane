# Always-on uberbot, a short setup, one project list

Design for the Nov 11 push. Decisions are the person's (Nov 11), marked **D1–D5**. Background: docs/BOAT-HUB.md (boat-hub, home.move,
the ring's front door), docs/onboarding.md (today's setup), docs/multiple-provider-accounts.md, docs/provider-fallback.md.

## Decisions (Nov 11)

- **D1** Setup has a step to connect the user's own Boat account (a login, or an API key through `secret_request` / the masked box).
  With it Backplane makes Boats for always-on and for builds/offload. The user pays Boat on their own account; Backplane does not resell.
- **D2** The uberbot stays on the user's computer unless they opt in. Setup explains always-on with three clear options:
  **Stay on this computer** (default), **An always-on Boat**, **Another machine of yours** (a home server, a desktop that stays on; called out on its own).
- **D3** KiCad and FreeCAD installs stay in setup. The tour, the shortcut tables and the tips (about 10 pages) leave setup entirely.
  Shortcuts live in a help sheet (palette: "Keyboard shortcuts", and `?` / F1 outside a text box).
- **D4** The person's own Bub moves to boat-hub, and it must be seamless: Bub talks to and works with every bot and thread on every machine,
  launches on any machine, and every client follows it.
- **D5** Project identity is the normalized git remote (`mirror::normalize_origin`: ssh/https, user, port and `.git` are one).
  A project with no remote stays per-machine unless linked by hand.
- **D6** A new thread starts on the machine the person is using now. The composer has an optional machine picker to send it elsewhere
  (a big machine, a Boat for builds). Builds may prefer a big machine or a Boat through the picker and build offload, never through the default.
- **D7** The uberbot holds the registry: projects, which machines have each, threads and where they run, in a small database owned by the
  uberbot's home; every hub syncs with it. With a machine off, the uberbot still knows the project and can offer to run elsewhere
  (e.g. a Boat with a fresh clone of the git remote).
- **D8** Always-on setup shows the expected monthly cost of each Boat size and lets the user pick; it states expected RAM use, including
  about 0.7–1.2 GB for local dictation, and offers to host dictation (local STT) on the uberbot's machine, on by default.
- **D9** Phones only pair to the existing setup. No phone onboarding.
- **D10** Model sign-ins on the Boat are a separate sign-in to each provider once more. That is fine.

## 1. Setup: the new flow

Five steps, one screen each. Each step can be skipped, and setup resumes where it was. No step lists keys or tips.

| # | Step | What it holds | Done when |
|---|---|---|---|
| 1 | **Welcome** | Where data is kept (default folder, Choose another), and the anonymous-usage switch as one row. Import from 0.14 shows only when a 0.14 home exists. | home set |
| 2 | **Agents** | One list: Claude, Codex, Antigravity, Grok, and "More agents" folded (Muse, OpenCode, Pi, Oh My Pi). Each row: Install / Connect / Connected as <email>. | one agent signed in |
| 3 | **Design tools** | KiCad and FreeCAD, one row each: found (version) / Install / Locate it. | both found, or skipped |
| 4 | **Where your uberbot lives** | The three options (D2), explained in two lines: "Always-on keeps your uberbot, reminders, bots and phone alerts running while this computer sleeps. Hardware and code still run here." Picking one opens its sub-steps inline (below). | a choice made |
| 5 | **First project** | A folder (existing or new) and Create; then the first message box. Pair a phone is one optional row here ("Get the app", QR); the phone itself has no setup (D9). | a project exists |

Then **Done**: one line, "Press ? any time for keys", with Open the uberbot.

### Step 4's options

- **Stay on this computer** (default, preselected): nothing else. A note: "You can move it later in Settings, Machines."
- **An always-on Boat** (D1):
  1. *Connect your Boat account*: Sign in (Boat's browser/device login, run by the hub) or Paste an API key (masked box → vault `boat.api_key`).
     Shows the account, plan and plan hours left.
  2. *Make the Boat* (D8): a size list, each with its expected monthly cost from Boat's own prices ("small: 2 vCPU, 4 GB, about $13 a
     month on your Boat plan") and the expected RAM use against it (hub about 0.2–0.5 GB, bots' agents while running, local dictation
     0.7–1.2 GB); a switch "Hear dictation there" (on by default) that installs the voice runtime on the Boat and makes it the hub's STT;
     the smallest size that fits is preselected. Make it.
     The hub does everything `scripts/boat-hub.sh` does today, in Rust: create (`--no-auto-stop`, no TTL, no stored logins), install the
     current build and the guard under systemd with the health timer, bring the network up (below), and link it to this hub with no code
     (this hub made it, so it mints the pairing on the Boat and joins it itself). Progress as one row with steps; a failure says which and offers Try again / Delete it.
  3. *Sign it in to your agents* (section 3): one row per provider this computer has, each "Sign in there".
  4. *Move your uberbot there*: one press (`home.move`), with the move's progress shown in Bub's own chat (section 4).
- **Another machine of yours**: "Install Backplane there and open Settings, Machines, Make a code" (copyable install line), then Pair (address + code;
  already linked machines are listed to pick instead), then sub-steps 3 and 4 as for a Boat. Always-on is shown only when that machine says it is
  a server (guard under systemd, or the "stays on" switch there); otherwise it says "It runs while that machine is on."

"Both" (home on one, failover to another) is not offered in setup: the home moves by one press, and clients already fail over to the home
(hub.offer). A standby uberbot needs `uber.sync` (BOAT-HUB.md, option C) and is left for later.

### Network for a user's Boat

The Boat must be reachable by this hub and the phones. Order: (1) the user's tailnet when this machine is on Tailscale: setup asks for an
auth key through `secret_request` (`tailscale.boat`), the key goes to the Boat on stdin and is deleted; (2) otherwise Boat's own public HTTPS
endpoint for the box if Boat offers one (to be checked by the lane), with the hub's pairing auth in front. The ring/webhook front door then
uses that address (it is fixed for the Boat's life, so the ring URL never changes after setup).

### What leaves setup

The 10 tour pages (viewers, 3D, mech, browser, thread, bots, space, rooms, routines, palette, settings), Restarts and updates, the separate
Codex/Claude/Others pages, Another machine as its own step. Their words go to: the help sheet (keys), each view's empty state (one line), and
docs/onboarding.md. Phones have no onboarding (D9): the app's first screen pairs it (QR or link and code) and then opens the uberbot;
setup is done from a computer.

## 2. Hosting: a user's Boat

- Owned by the user's Boat account (D1). The hub keeps `alwayson.boat` = `{id, name, size, created, network, url}` and the key in the vault.
- Settings, Machines shows the always-on machine: status, build, cost this month and plan hours left (today's `update/boathub.rs` rows,
  generalized from the desktop's measurement to the hub's own Boat API calls), Resize, Restart, Delete (two presses, says what it removes).
- Keep-alive: systemd guard + health timer on the box (as today); this hub checks it every 5 minutes and alerts once if it stays down.
- Updates: the box follows the same channel as the hub that made it (releases for users; main builds for the person, as today).
- Builds/offload use the same Boat account (separate sandboxes, short-lived); nothing in this push beyond storing the account where the
  sandbox backend can read it.

## 3. Signing the always-on hub in

The second sign-in happens **on the always-on hub**, driven from the client the person is using. Each provider's own flow, run there over the
far link, with the link and the code shown here:

| Provider | How | Notes |
|---|---|---|
| Claude | sync token (520f05dcb): Make a sync token here; it reaches every linked machine | one browser approval; no second OAuth session on one login |
| Codex | `codex login --device-auth` on the remote hub; its URL and code are shown here | ChatGPT subscription; no API key |
| Antigravity | agy's own sign-in on the remote (device/URL flow if it has one, else its terminal, through the hub's terminal) | |
| Grok | `grok login` on the remote, same way | |
| Others | listed with "Sign in there" opening a terminal on the remote hub | |

One rule: account files are never copied between machines (two machines refreshing one OAuth session log one out); only sync tokens travel.
Setup's sub-step shows each row as Signed in / Sign in there / Not on that machine (with Install there).

## 4. The uberbot's home, moves and failover

- One home holds Bub, the bots, rooms, hooks, mail, vault, APNs and paired devices (`home.move`, net/homemove.rs). Other hubs show them as far
  bots (aae1beeca) and every client adds the home by itself (`hub.offer`, e81cc3b79).
- **Never lose sight of Bub during a move:** the client keeps Bub's chat open throughout. During the copy Bub's row says "Moving to boat-hub";
  a message sent then is held and delivered at the new home (not dropped, not an error); afterwards the same row, same thread, now marked
  boat-hub. A failed move rolls back and says so once, in that row.
- **From the home, Bub works everywhere:** `thread_launch {machine}` to any linked machine and project (a project on several machines picks the
  default machine, section 5), `thread_send/read/update/interrupt` on `<link>~<id>`, far bots and rooms both ways, held launches when a machine
  is away. Bub's tools that need a local thing (KiCad, files, browser) run in a thread on the machine that has it.
- **thread_update changes provider/model/account** in the Rust hub (today it renames only), locally and over the link, so failover and Bub
  can move a thread to another provider.
- The person's move (D4): after the end-to-end test passes, the desktop's Claude sync token is made, Codex/agy/Grok are signed in on
  boat-hub, Bub answers a test there, then `home.move` to boat-hub. The ring keeps `https://boat-hub.tail9e0cf2.ts.net/hook/h1794356011-590786`.

## 5. One project list

- A project's key is its normalized origin (D5); with no origin, `<machine>/<project id>` (per machine). A hand link (Link with… on a project's
  menu) sets `project.link.<id>` = the key of another, stored on both machines.
- The sidebar (hub `present/screen.rs`, so GPUI and phones alike) shows **one row per key**: the name (this machine's name for it, else the
  most recently active machine's), machine chips (`desktop · boat-hub`, an away machine dimmed), and the threads of every machine under it,
  each with its quiet machine marker. A project on linked machines only (none here) is one row too, not "Backplane on framework".
- The machine switcher at the top stays as a filter, not as a split.
- **Where new work runs (D6):** on the machine the person is using now (the client's own hub; on a phone, the hub it is on). The composer says
  "runs on <machine>" with an optional picker: the machines that have the project, plus "A Boat (fresh clone)" when the project has a remote
  and a Boat account is connected. If this machine does not have the project, the picker opens on the machine with the latest activity and
  offers "Clone it here".
- **The registry (D7):** the uberbot's home keeps a small registry: project key, name, remote, and per machine its project id, folder, last seen,
  and each thread's (machine, id, title, state, last activity). Every hub sends its rows to the home on change and on link (a far call, like
  sync accounts), and reads the merged list back; the sidebar is drawn from the registry plus the live mirrors, so a machine that is off still
  shows its projects and threads (dimmed, "desktop is off"). Starting work on a project whose machines are all away offers: wait for it (a held
  launch, today's far outbox), or run on a Boat with a fresh clone of the remote. Stored as events in the home's log (moves with `home.move`).
- data/1 for phones: `projects.list` returns merged rows (`key`, `machines`, `default`), and thread lists take a key.
- Migration: nothing is rewritten. Keys are computed from each machine's origin (machine_list already sends it) and the registry is filled by the
  first sync; old per-machine settings (pins, icons) carry to the merged row from the local project, then the newest.

## 6. Data migration

- Setup: a home that finished the old setup is marked done for the new one (no re-run). A half-done one resumes at the first new step not done.
- `onboard.checks` keeps its ids; removed step ids are ignored.
- The person's Boat: `bp-hub` is adopted as `alwayson.boat` (`id bx_479dqxby`, network tailnet), so Settings shows it like a user's.

## 8. The sidebar (accepted Nov 11, D11–D12)

- **D11 Thread visibility (T3 Code style):** a thread shows in the sidebar only while it is active: running (or monitoring work it owns),
  needs the person (an ask, approval or question), or stopped abnormally (failed, interrupted, cut by a restart or a limit). Finished, settled
  and idle threads are hidden there and stay reachable from the project page and Ctrl+K. Pinned threads always show.
- **D12 Structure, top to bottom:**
  1. Bub (the uberbot).
  2. **Needs you:** every open ask, approval and failure across all projects and machines; gone when empty.
  3. **Pinned** projects (thread pins extended to projects).
  4. **Active:** projects with a visible thread or activity in the last few days, newest first; each row a running count and dots for the
     machines that have it.
  5. **Groups** for the rest, auto-grouped by who the work is for (git owner, then folder: American Embedded / FazePro, Nordic, Backplane,
     i2cjak personal), named by Haiku like the project icons; the person renames groups and moves projects; collapsed by default, with counts.
  6. **Quiet:** nothing for 2 weeks; auto-collapsed, never deleted, searchable.
- **A project's page:** running, then needs you, then recent, with done threads behind "Done (n)".
- **Ctrl+K** jumps to any project or thread on any machine (registry-backed).
- Same on GPUI, iOS and Android: the hub presents it (present/screen.rs sections), the clients draw sections generically.
- Built on section 5's merged rows (lane I exposes them); lane J owns the sections.

## 7. Lanes

| Lane | Owns | Output |
|---|---|---|
| **E. Setup** | `present/onboard.rs`, `settings/onboard.rs`, GPUI `screens/onboard.rs`, `first.rs`, the phones' onboard views, help sheet | the five steps, help sheet, test/onboarding |
| **F. Always-on Boat** | new `net/alwayson/`, `update/boathub.rs`, Settings Machines rows, `present/onboard_alwayson.rs` (step 4's rows/actions, which E includes) | Boat account, create/link/keep-alive/cost/delete |
| **G. Sign in there** | `settings/accounts*`, remote sign-in flows over the far link, client panels | section 3 |
| **H. Bub everywhere** | `net/homemove.rs`, `agents/far.rs`, `agents/launch.rs`, thread_update provider change, move-in-chat | section 4 |
| **I. One project list** | `present/screen.rs` projects, `mirror.rs`, new `net/registry.rs`, data/1 projects, composer machine picker, GPUI sidebar, iOS/Android project lists | section 5 |
| **J. Sidebar** | sidebar sections in `present/screen.rs` (beside I's merged rows), project page, groups + Haiku naming, project pins, Ctrl+K, client section drawing | section 8 |
| **T. Test and polish** | test/e2e, screenshots | before/after, fresh-user run on a throwaway home and a real Boat |
