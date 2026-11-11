# Connect devices to a hub

> Paths under `src/` below (`src/core`, `src/server`, `src/web`, `src/mobile`, `src/app`) name the Bend JS tree, removed from this checkout (tag `main-bend-final`). Where the current Rust code for each is, is unverified; look under `native/host/src/` and `native/gpui/`.

An unpaired desktop client starts at **Connect to your Backplane hub**; a revoked
one starts at **Re-pair this device**. On an already paired machine, open
**Settings → Devices → Add a device** (Settings → Network has Add a device too).
It makes one invitation with three forms: a link, a QR code and an
eight-character code to enter with the shown hub address.

The desktop client takes the link, or the address and code. It does not scan: a
QR code is for the phones, which scan it with their own camera. The phones take
the link or the QR code. Device names start with the OS and are shown in
Settings → Devices.

An invitation expires after ten minutes and works once, whether redeemed by
link, QR or code. Create a separate invitation for each device. Wrong, expired,
used, revoked and rate-limited codes have separate responses.

The hub records every invitation it issues (hashes only, in `devices-v1.json`
beside the devices): when it was issued, whether it was used, when and by which
device, and whether it was revoked. **Settings → Devices** lists each one not
yet used, or used by a device that still has access, with its own Revoke (the
finished ones stay in the registry but are not listed, nor are revoked devices). Revoking an unused link stops it from pairing
anything; revoking a used one also revokes the device it paired and closes that
device's connections. A hub pairs only with a link it issued, recorded and has
not revoked; one it cannot record is never issued. The newest hundred finished
links stay listed. Hub-to-hub invites (`bp1:`, made by `bots.invite`; no page makes one now, and the
Machines page's address field still takes one, and the machine codes below) are
recorded and listed here too, as machine invites: they expire after ten
minutes and work once like a device invitation, and revoking a used one
unlinks the machine it linked. A short code needs the hub
address: Backplane does not introduce a public code-resolver service. Both
devices need a route to the hub, usually its LAN or tailnet.

A pairing link is `http(s)://<hub>/pair#invite=<secret>&routes=<addresses>`. The
scheme link `backplane://pair?url=<that link>` reaches the desktop client from
anywhere (a message, another app); `deploy/backplane.desktop` registers it on
Linux and the bundle `scripts/mac-run.sh` makes on macOS. The phone builds keep
their own schemes: `bpgpui://` on Android, `backplane-gpui://` on iOS.
The desktop client takes all three, and never pairs from a link that came from
outside its window: it names the hub and asks, with Cancel first.
Short codes need the hub address: there is no
public code resolver. Both devices need a route to the hub, usually its LAN or
tailnet. The hub answers `POST /auth/known` (is this origin one of the owner's
hubs), but no client asks it yet.

Add a device asks how the other device reaches the hub: **Tailscale**, **Local
network** or **This computer** (only those that exist; Tailscale first, then the
local network, then this computer). Switching swaps the hub address, link, QR
code and Copy in place: one invitation answers on every address, so its short
code works with whichever address is shown beside it. `POST /auth/invitations`
returns them as `addresses` (`{kind, url, link, qr, listening}`, in that order:
the tailnet, each local network IPv4, this computer; `native/host/src/net/plan.rs`);
each link lists its own address first in its routes, then the other remote
addresses the hub answers on (never loopback, which on another device would
reach that device's own hub). The older fields (`link`, `qr`, `routes`,
`localOnly`) are the first remote address that answers, else this computer. A
tailnet address is one a listener confirmed (the hub's tailnet listener or
Serve), one a listener on every interface covers, or the one the request came
through. Local network addresses are the machine's IPv4 ones without loopback,
link-local, the tailnet's 100.64/10 or a container's or VM's bridge (docker0,
br-*, veth*, virbr*, ...). A link whose every route is loopback is marked
`localOnly`, and This computer says it works only on this computer. Alternative
addresses in the link are hints, not a new tunneling service. The page shows the
link, its QR code (dark on white with a quiet zone, which phone scanners read in
either theme), the code beside the hub address and the time it expires; Copy link
and Copy code put them on the clipboard. The phones draw the same rows
(`devices.page`, docs/PHONE-DATA.md).

A hub listening on loopback only (the default) does not answer on the local
network. Choosing Local network then offers **Let the hub answer on the local
network**: the hub opens one more listener on that address and port (`POST
/auth/lan`, `X-Backplane-Lan: <address>`, `X-Backplane-Lan-State: on|off`,
authorized like `POST /auth/invitations`; `native/host/src/net/lan.rs`). It is
untrusted as the tailnet listener is for a device its owner does not own: no
loopback trust and no owner, so only the page itself, `/hello` and pairing answer
without a paired credential. It is plain HTTP on that network: the traffic is not
encrypted (the page says so), so prefer Tailscale on a network you do not trust.
The choice is kept in `<home>/lan-v1.json` and listened on again after a restart
(an address the machine no longer has stays kept and is tried at the next start).
**Stop answering here** under Settings → Devices closes that listener and its
connections and forgets it. A hub started with `--host 0.0.0.0` (or that address)
already answers there and needs none; a hub on a copied log never opens one.
`GET /auth/devices` lists the kept addresses as `lan` (`{ip, url, listening}`).

A hub on its tailnet address (`--host <100.x>`) or LAN address serves plain HTTP
there. `--tailscale-https` or `--tailnet-only` also puts it behind
`tailscale serve` and names that HTTPS address first; set `BACKPLANE_HTTPS_PORT`
to a free port, because a hub never takes a port already mapped to something else.
Every client reaches such a hub over TLS, checked against the system's roots.

A phone keeps every hub it pairs with (Settings → Hubs: Use, Forget, Add a hub; the thread list's top bar names the hub in use, and tapping it lists the others and Add a hub);
the first is the one in use, and switching drops the phone's saved copy of the last
one's threads. Pairing a hub again replaces only that hub's credential. A phone
tries an invitation at each address it names, in order, and keeps the one that
answered; a refusal is final (every guess counts against the hub's limit). A link
whose every address is the hub's own computer (`localOnly`: the hub runs on
127.0.0.1 without Tailscale) is refused on the phone with that reason, and
Settings → Devices and Setup show a warning beside it; Setup does the same for a
machine invite that names 127.0.0.1. `test/mobile/hubs_cases.json` holds both
phones to the same list and address rules (`HubListTest`,
`scripts/mobile/ios-hub-list-test.sh`, `test/mobile/hub_list_ref.py`).

`test/e2e/pairing/link.py` links two running temp-home hubs: first by a machine code and an
approval (a wrong code, a denied request, a request that waits and links nothing until approved, both sides recording
`approved`), then the older invite paste (`bots.invite`, `bots.join`: unpaired; the Machines page takes a `bp1:` invite in its address field), and checks both logs hold the
link and that it survives a restart with `--restart-b`.

## Pairing two machines

Being signed into the same Tailscale account is not trust, and hubs never link by themselves. A link exists only after a code
and an approval:

1. Machine A makes a machine code (Settings → Machines → Make a code, or `backplane pair --machine`): eight characters of the
   device-code alphabet, recorded as a machine invite (hash only), ten minutes, once.
2. Machine B enters A's address and the code (Settings → Machines → Pair a machine, or a found machine's Pair, or
   `backplane pair --join ADDRESS CODE`; a `bp1:` invite pasted into the address field still works, as an unpaired link).
   B shows a six-digit check code and asks `POST /bots/pair/request`. A wrong code is refused; ten tries a minute hub-wide, right
   or wrong, then everything waits. A code that works is spent, even if the request is denied.
3. The request waits on A (Settings → Machines → Requests, or `backplane pair --requests`) showing B's name, address and the
   same check code. The person approves (the secret is then kept on both sides and the link made) or denies; two minutes without
   an answer is a refusal. B polls `POST /bots/pair/poll` and receives its invite once.
4. Both hubs record the link as made by an approved pairing (`peer.own.<id>` = `approved`). That, or the person pressing Mine
   on an existing link (`owner`), is what `PeerTable::verified` means: thread sharing, `/far/rpc` forwarding and owner mail rest
   on it. Links made before this (set `pair` or `tailnet`) stay trusted, so nothing already linked breaks; nothing writes those
   values any more. A pasted invite links but shows as Unpaired until marked Mine or paired again with a code.

A hub never announces a loopback address (127.x, localhost, [::1]) to a hub that is not on loopback: B refuses to ask, and A
refuses a request or a link that names one. Start the hub with Tailscale on, or set `BACKPLANE_URL`.

Found on the network lists every tailnet machine that answers `GET /hello`, with `same_user` false for another account's. It
is a suggestion with a code field, never a link.

`backplane pair` (the launcher; it finds the running hub through `--home`, `$BACKPLANE_HOME` or the `--default` home, and its
port in `<home>/hub.lock`, `--port` overrides) talks to the hub on loopback with the hub's own token, which it never prints:

    backplane pair                       link, short code and QR for a new device
    backplane pair --machine             a machine code
    backplane pair --join ADDR CODE      ask the hub at ADDR; prints the check code and waits for the approval
    backplane pair --requests            requests waiting here, and attempts made from here
    backplane pair --approve ID | --deny ID

Hub routes: `POST /bots/pair/request` and `/bots/pair/poll` (admitted by the code), `POST /bots/pair/local` (the hub's
token; what the CLI calls), `POST /bots/link` (signed, as before). Socket methods: `machines.page`, `machines.page.act`
(`PHONE-DATA.md`), `machines.code`, `machines.pair {address, code}` → `{attempt, check}`, `machines.attempt {attempt}`,
`machines.requests`, `machines.approve {request}`, `machines.deny {request}`.

## Access and preservation

Invitation secrets are in fragments and exchanged in POST authorization headers.
They never contain the hub's long-lived credential. Issued device tokens are
random; the private atomic `devices-v1.json` registry stores their hashes, names
and activity times. An invitation still open when the hub restarts keeps working
until it expires. Enrollment uses a hub-wide ten-attempt/minute bucket. Registry
read/write failures fail closed and never rotate or rewrite the existing hub token
file.

Native clients keep their credential privately: the phones in their own storage,
the desktop client in `hubs/<host>_<port>.token` (one owner-only file per paired hub) with the list in `hubs.json` beside its local settings (the first hub is the one in use; an older `pairing.json` + `device-token` is migrated on first start),
never in a hub's `token` file. A failed exchange does not replace an existing
pairing, and exchanges reject redirects. Revoking a device, from Settings or
`POST /auth/revoke`, rejects its future requests and closes its open connections,
including admissions queued during the revocation.

Existing shared-token pairings continue to work without re-pairing or token
rotation. Because that older credential is shared, those pairings cannot be
listed/revoked as separate devices retroactively; the device list explains this.
New invitations produce the individually managed sessions. Native machine
linking and its existing owner-authentication rules are unchanged.

## Validation

`cd native/host && scripts/cargo-q test --offline pair_`, `net::plan`, `net::lan`,
`net::devicespage` and `present::settings::tests::devices` and
`cd native/gpui && scripts/cargo-q test --offline pairing` check the registry, the
routes, link parsing, refusals and the revoke paths.

`test/e2e/pairing/proto.py` drives a running temp-home hub over HTTP and its
data/1 socket from any machine: link and short code, used, wrong, revoked and
expired codes, rate limiting, revoke, re-pair and restart.
`test/e2e/pairing/settings_revoke.py` sends the action a Settings button sends and
checks the device's socket closes. Both are protocol results, not client results;
`docs/e2e/MATRIX.md` holds what has been run where.

## Your machines: one list, threads anywhere

Once two machines are paired, the window's sidebar is one list over both (`present/screen.rs` `recent_unified`; the phones' data/1 lists
follow the same rules, `docs/PHONE-DATA.md`):

- One repository on both machines is one project (`mirror::unify`, by the normalized origin remote); every other project of a linked machine
  has its own row saying "on <machine>". Each thread of another machine carries its machine's mark (short on the row, the full name in
  the header, dimmer with "away" when that machine did not answer the last minute's ask).
- Order: each machine keeps its own order, and the machines' projects and Active rows stand together by recency (live first, then the
  last move: `sidebar::merge_ordered`, `Plan::place`, `Plan::active_keyed`).
- The switcher over the list (`shown.machines`): All, this machine by its name, each linked machine. A press is `side-machine` with
  `""`, `"@here"` or the machine's name (`@machine` in the session); a machine no longer linked falls back to All. One machine's
  projects are shown as that machine has them (not joined to this one's).
- New thread on a chosen machine: a project that is also on linked machines has `newOn` (this machine first, then each linked one, an
  away one saying so); the window's + and the project's menu offer them. A thread made there shows once that machine has pushed it.
- A thread of another machine is opened, read, sent to, stopped, settled, archived, renamed and answered here; each request goes to its
  machine over `/far/rpc` (`net/rpc.rs`, `far::METHODS`) and the mirror brings the result back. Its viewers are drawn there: Board,
  Schematic, 3D, Split and Mech through `net/farview.rs` (frames forwarded), Files through `files.dir`/`files.get`
  (`net/farfiles.rs`: the listing comes over, a file opened is shown from a copy under `<home>/far-files/`), Diff through `git.diff`,
  and the terminal (`term.open/input/resize`) is a shell on that machine in the thread's folder, its output forwarded.
- Links: `backplane://thread/<id>` opens a thread of this machine; `backplane://thread/<link>~<id>` or `<machine>~<id>` (the machine's
  short or full name) opens one of a linked machine this hub mirrors; this machine's own name before `~` is this machine
  (`mirror::thread_ref`). The window takes them from a message, from the system's link handler and on its command line (`link-open`);
  the phones ask `link.resolve`.

`test/e2e/pairing/unified.py --hold DIR` keeps two linked temp hubs up (`BP_NAME_A`/`BP_NAME_B` name them) for a window to drive.

## Home hub: an offered hub, added with no taps, used when the hub in use is away

When the person's home (Bub, the bots, push, webhooks) moves to an always-on hub, the hub in use offers it to every client paired with it.
The offer is info `hub.offer`, a JSON string (`""` for none) with no secret: `{"id", "name", "url", "why": "home", "credential"?: "same"}`.
GPUI gets it in the info snapshot and `{"t":"info"}` frames. data/1 gets it as hello `hubOffer` (feature `hubOffer`) and `{"t":"info","info":{"hub.offer":…}}` frames.
What a client does is `HubList.plan` / `hubs::plan`, held to `test/mobile/hubs_cases.json` (`offer`):

- none: no offer, one already answered, or one for the hub in use.
- known: the hub is already on the list. No take; it becomes the home when `why` is `home`.
- same (`credential: "same"`, and the client holds a device token for the hub in use): the hub adopted this hub's paired devices, so the
  client keeps the offered url with that same token, once `GET /client/capabilities` there takes it. The phones are this case.
- take (no device token of its own: the desktop window on its own hub's token): `hub.offer.take {id}` → `{link}`, a single-use pairing link,
  redeemed as a scanned code is (`/auth/enroll`).

The hub goes second on the list (`HubList.added` / `hubs::second`): the hub in use stays in use. A failed try is tried again twice, with a longer
wait each time. Then the client calls `hub.offer.done {id, ok}` on the hub in use. Nothing is shown to the person.

Failover (`HubList.onHome` / `hubs::on_home`, the table's `failover`): the client remembers the added hub as the home (phones: the stored key
`home`; GPUI: `home` in `hubs.json`). When the hub in use has not answered for 20 s and the home answers `/hello`, the client connects to the home.
The list does not change. While on the home, the client asks the chosen hub's `/hello` every 10 s and goes back as soon as it answers.
This holds at a start with the chosen hub asleep too. The hub lists show which hub is connected now, and mark the home.

A push may carry `hub` (the sending hub's url) and `link`. A tap on one from another hub on the list connects to that hub first, then opens the
link: `account:<id>` is that account's sign-in. `scripts/mobile/ios-offer-journey.sh` and `scripts/mobile/android-offer-journey.sh` run the
whole thing against two stand-in hubs (`test/mobile/offer_stub.py`).
`scripts/mobile/android-homemove-journey.sh HUB_BIN APK` runs the same against two real temp hubs after a home move (`test/e2e/homemove/hold_phone.py`).
