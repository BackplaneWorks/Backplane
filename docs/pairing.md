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
device, and whether it was revoked. **Settings → Devices → Pairing links** lists
them, newest first, and each one not yet used, or used by a device that still
has access, has its own Revoke. Revoking an unused link stops it from pairing
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

The hub puts listener-confirmed tailnet origins ahead of a desktop's loopback
address. A listener bound to every interface also advertises its IPv4 routes.
A link whose every route is loopback is marked `localOnly`. Alternative addresses
in the link are hints, not a new tunneling service.

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

`cd native/host && scripts/cargo-q test --offline pair_` and
`cd native/gpui && scripts/cargo-q test --offline pairing` check the registry, the
routes, link parsing, refusals and the revoke paths.

`test/e2e/pairing/proto.py` drives a running temp-home hub over HTTP and its
data/1 socket from any machine: link and short code, used, wrong, revoked and
expired codes, rate limiting, revoke, re-pair and restart.
`test/e2e/pairing/settings_revoke.py` sends the action a Settings button sends and
checks the device's socket closes. Both are protocol results, not client results;
`docs/e2e/MATRIX.md` holds what has been run where.
