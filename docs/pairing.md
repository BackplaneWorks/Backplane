# Connect devices to a hub

An unpaired web or desktop client starts at **Connect to your Backplane hub**.
A revoked device starts at **Re-pair this device**. On an already paired machine,
open **Settings → Devices → Add a device**. Copy or share its invitation link,
scan its QR code, or enter the eight-character code and the shown hub address.
Web clients can also choose a local QR image; images are decoded locally.

An invitation expires after ten minutes and works once, whether redeemed by
link, QR or code. Create a separate invitation for each device. Wrong, expired,
used and rate-limited codes have separate responses. A short code needs the hub
address: Backplane does not introduce a public code-resolver service. Both
devices need a route to the hub, usually its LAN or tailnet.

The HTTPS/HTTP link opens a web connection page and offers **Open in Backplane**
for installed apps. iOS/Android handle the existing `backplane://pair` scheme;
desktop packages register it too. This explicit web fallback works with
user-owned hub domains; universal/app links on arbitrary hub domains are not
claimed. A hosted HTTPS page does not bypass browser mixed-content rules.

The hub puts listener-confirmed tailnet origins ahead of a desktop's loopback
address. A listener bound to every interface also advertises its IPv4 routes.
When only loopback is available, web device management explains that tailnet
access is needed to connect another computer. Alternative addresses in the link
are hints, not a new tunneling service.

## Access and preservation

Invitation secrets are in fragments, removed from browser history before normal
app handling, and exchanged in POST authorization headers. They never contain
the hub's long-lived credential. Issued device tokens are random; the private
atomic `devices-v1.json` registry stores their hashes, names and activity times.
Invitations live only in memory and are invalid after a hub restart. Enrollment
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
link/code state, expiry, rate limiting, write failures and listener route hints.

`bun test/tools/pairing_ux_e2e.ts EMITTED_HUB_JS WEB_DIRECTORY` uses a private hub,
stand-in providers and a fixture-only clock preload. It covers the HTTP flow,
real QR decoding, actual browser controls, first-run and revocation screens,
HttpOnly cookies, mobile-width layout and credential-free hub logs. Optional
`ANDROID_SERIAL` and `ELECTRON_BINARY` target only dedicated test instances.
It never reads or copies the live home. All hubs and browsers it starts are
stopped by their captured handles/PIDs.
