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
links stay listed. Hub-to-hub invites (`bp1:`, made in Bots to link a
machine, and the ones hubs give each other when they pair by themselves) are
recorded and listed here too, as machine invites: they expire after ten
minutes and work once like a device invitation, and revoking a used one
unlinks the machine it linked. A short code needs the hub
address: Backplane does not introduce a public code-resolver service. Both
devices need a route to the hub, usually its LAN or tailnet.

A pairing link is `http(s)://<hub>/pair#invite=<secret>&routes=<addresses>`. The
scheme link `backplane://pair?url=<that link>` reaches an installed app from
anywhere (a message, another app). The desktop client never pairs from a link
that came from outside its window: it names the hub and asks, with Cancel first.
Packages register the scheme: `deploy/backplane.desktop` on Linux, the bundle on
macOS, the manifests on the phones. Short codes need the hub address: there is no
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

## Access and preservation

Invitation secrets are in fragments and exchanged in POST authorization headers.
They never contain the hub's long-lived credential. Issued device tokens are
random; the private atomic `devices-v1.json` registry stores their hashes, names
and activity times. An invitation still open when the hub restarts keeps working
until it expires. Enrollment uses a hub-wide ten-attempt/minute bucket. Registry
read/write failures fail closed and never rotate or rewrite the existing hub token
file.

Native clients keep their credential privately: the phones in their own storage,
the desktop client in `device-token` and `pairing.json` beside its local settings,
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
