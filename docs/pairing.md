# Connect devices to a hub

An unpaired web or desktop client starts at **Connect to your Backplane hub**.
A revoked device starts at **Re-pair this device**. On an already paired machine,
open **Settings → Devices → Add a device** (Settings → Network has Add a
device too). Copy or share its invitation link, scan its QR code,
or enter the eight-character code and the shown hub address; the pairing
screen says where to find them. Web clients show Pairing link, QR code and
Short code as equal choices. The QR choice opens the camera full screen inside
the page (`src/web/pairing-scanner.js`: the rear camera, a Torch where the
camera has one, Close/Esc/Back; the browser's BarcodeDetector where it has
one, else the bundled jsQR) and pairs with the first invitation it reads, the
same way as a pasted link; it also has an image picker; images and frames are
decoded locally. A browser gives the camera only to a secure page (https://,
or localhost): over a plain http:// address (a phone at the hub's LAN or
tailnet address) the QR choice says so and offers the other ways in (the
phone's own camera app, which opens the link, or Short code) instead of a
button that cannot work.
Connection and device screens use the same light/dark theme as Settings.
Device names start with the advertised OS and browser (or Desktop) and remain
editable; no permission or network lookup is needed. A browser's enrolment answers its own device id
(never its credential), so a phone's Devices list marks and lists first This
device.

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

The HTTPS/HTTP link opens a web connection page and offers **Open in Backplane**
for installed apps. A `backplane://pair` link reaches the desktop app from
anywhere (a web page, a message), so the app pairs at once only with one of
the person's own hubs: its own service, the hub it is already paired with, or
one its own service knows (`POST /auth/known`: the hub's own addresses, the
owner's machines it found on the tailnet or in `BACKPLANE_PEERS`, and the
linked hubs it verified as the owner's). That hub then refuses a link it did
not issue or has revoked. For any other address the app asks first, naming
it, with Cancel the default. Links pasted, scanned or typed inside the app
pair as before. iOS/Android handle the existing `backplane://pair` scheme;
desktop packages register it too. This explicit web fallback works with
user-owned hub domains; universal/app links on arbitrary hub domains are not
claimed. A hosted HTTPS page does not bypass browser mixed-content rules.

Add a device asks how the other device reaches the hub: **Tailscale**, **Local
network** or **This computer** (only those that exist; Tailscale first, then the
local network, then this computer). Switching swaps the hub address, link, QR
code, Copy and Share in place: one invitation answers on every address, so its
short code works with whichever address is shown beside it. `POST
/auth/invitations` returns them as `addresses` (`{kind, url, link, qr,
listening}`, in that order: the tailnet, each local network IPv4, this computer;
`effects/pairing.js`'s `pairingPlan`); each link lists its own address first in
its routes, then the other remote addresses the hub answers on (never loopback,
which on another device would reach that device's own hub). The older fields
(`link`, `qr`, `routes`, `localOnly`) are the first remote address that answers,
else this computer, as before. A tailnet address is one a listener confirmed
(the hub's tailnet listener or Serve) or one a listener on every interface
covers. Local network addresses are the machine's IPv4 ones, without loopback,
link-local, the tailnet's 100.64/10 or a container's or VM's bridge (docker0,
br-*, veth*, virbr*, ...). This computer still says it works only on this
computer. Alternative addresses in the link are hints, not a new tunneling
service.

A hub listening on loopback only (the default) does not answer on the local
network. Choosing Local network then offers **Let the hub answer on the local
network**: the hub opens one more listener on that address and port (`POST
/auth/lan`, `X-Backplane-Lan: <address>`, `X-Backplane-Lan-State: on|off`,
authorized like `POST /auth/invitations`; server.bend's `Lan.*`). It is
untrusted exactly as the tailnet listener is for a device its owner does not
own: no loopback trust and no owner, so as there only the web page, `/hello`
and pairing itself answer without a paired credential, and the desktop Browser
channel stays loopback-only. It is
plain HTTP on that network: the traffic is not encrypted (the screen says so),
so prefer Tailscale on a network you do not trust. The choice is kept in
`<home>/lan-v1.json` and listened on again after a restart (an address the
machine no longer has is kept and tried at the next start). **Stop answering on
the local network**, beside the choice and under Devices → Local network,
closes that listener and its connections and forgets it. A hub started with
`--host 0.0.0.0` (or that address) already answers there and needs none.

## Access and preservation

Invitation secrets are in fragments, removed from browser history before normal
app handling, and exchanged in POST authorization headers. They never contain
the hub's long-lived credential. Issued device tokens are random; the private
atomic `devices-v1.json` registry stores their hashes, names and activity times.
An invitation still open when the hub restarts keeps working until it expires. Enrollment
uses a hub-wide ten-attempt/minute bucket. Registry read/write failures fail
closed and never rotate or rewrite the existing hub token file.

Browsers keep the resulting credential in an HttpOnly, SameSite=Strict cookie;
native clients keep their resulting session using the existing saved-link
storage. Failed native exchanges do not replace an existing pairing. Native
and desktop exchanges reject redirects. Revoking a new session rejects future
requests and closes its existing websocket connections, including admissions
queued during revocation. Device sessions do not open the private desktop
Browser channel, which retains its local main-process authentication boundary.

Existing shared-token pairings continue to work without re-pairing or token
rotation. Because that older credential is shared, those pairings cannot be
listed/revoked as separate devices retroactively; the device list explains this.
New invitations produce the individually managed sessions. Native machine
linking and its existing owner-authentication rules are unchanged.

## Validation

`node --test desktop/test/pair-browser.test.cjs desktop/test/pairing-links.test.cjs test/tools/pairing_devices_test.cjs`
checks the existing exchange, strict link parsing, persistence, single-use
link/code state, expiry, rate limiting, write failures, listener route hints,
the invitation records and their revocation, which origins count as the
owner's hubs, an invitation's addresses and their links, and local network
listening (kept, bound, reopened, turned off). `bun
test/tools/pairing_devices_refresh.ts` drives Add a device's address choice in
a browser with fixture responses.

`bun test/tools/pairing_ux_e2e.ts EMITTED_HUB_JS WEB_DIRECTORY` uses a private hub,
stand-in providers and a fixture-only clock preload. It covers the HTTP flow,
real QR decoding, actual browser controls, first-run and revocation screens,
HttpOnly cookies, mobile-width layout and credential-free hub logs. Optional
`ANDROID_SERIAL` and `ELECTRON_BINARY` target only dedicated test instances.
It never reads or copies the live home. All hubs and browsers it starts are
stopped by their captured handles/PIDs.
