# Browser pairing proposal for 0.16.4

Settings › Devices controls browser access. It starts off. When enabled, show the hub’s tailnet HTTPS address. Opening it displays a pairing page. The browser and the native Devices page display the same six digits. The person compares them and approves on the desktop or a paired phone. Devices lists the browser by name so the person can revoke it later.

## Trust and transport

Protect chats, files, viewer data, vault contents and device credentials from other tailnet members, hostile websites and stolen sessions. Trust the operating system, the hub, tailscaled and the approving native device. A compromised paired browser can act within its granted permissions until revocation. Revocation ends future access. A recipient can retain content already delivered.

Use an explicit tailnet-only HTTPS Serve mapping to a dedicated loopback browser listener. Verify the configured MagicDNS authority including its port. Keep this listener separate from native loopback trust and route registration. Refuse public Funnel exposure and fail closed when the ingress configuration cannot be verified. Local processes can connect to a loopback backend, so every protected operation still requires pairing. Treat proxy identity as a rate-limit hint only when its provenance is verified. Cookies are shared across ports on a hostname. Enable persistent browser sessions only on a hostname whose HTTPS services the person trusts.

## Pair and authenticate

1. A same-origin POST to /auth/web/request creates a two-minute in-memory request and returns the comparison code. Issue an independent random polling secret in a short-lived Secure, HttpOnly, SameSite=Strict, host-only cookie. Keep its digest server-side. Bound pending requests, body sizes, and request and poll rates per verified peer and hub-wide.
2. Deliver the pending name and comparison code only to authorized approvers through devices.page. Escape the untrusted name. Approval and denial consume the pending decision atomically. Restart, expiry and disabling browser access discard pending requests.
3. POST /auth/web/poll with the polling cookie redeems approval once. Persist the digest of a fresh credential and browser kind in devices-v1.json before sending Set-Cookie for __Host-bp_web with Secure, HttpOnly, SameSite=Strict and Path=/. Clear the pending cookie. Keep both credentials out of URLs, bodies, traces, client storage and frames.
4. In auth.rs, resolve the cookie to a live browser device for each protected HTTP request and websocket upgrade. Scope that credential to browser routes and approved data/1 commands. Enforce exact configured HTTPS Origin on POST and websocket upgrade, rejecting missing or ambiguous headers. Enforce the configured Host on every route. Restrict browser actions that can mint credentials, reveal secrets or administer ingress at the hub.
5. Enforce idle and absolute expiry server-side across restarts and on open sockets. Count intentional client activity for idle expiry. Revoke by device ID, invalidate pending writes and close all its sockets within one second. Disabling access terminates active sockets and blocks further browser requests immediately.

## Data and rendering

Serve only the pairing page, its necessary static resources and minimal health information before authentication. Use the phone wire projection for masked chat and viewer data. Pass /files/get, /bots/<thread>/screen.jpg and /message-image through the shared verifier and preserve their resource-specific path and snapshot checks. Send private, no-store on sensitive responses. Serve project HTML or SVG downloads as attachments or through an isolated origin. Keep uploaded content outside script and worker asset paths. Restrict script, style, worker and connection sources by CSP, forbid framing and object embeds, and set nosniff and no-referrer.

Implementation touches net/listener.rs for ingress, wire/mod.rs for HTTP and upgrade dispatch, auth.rs for the verifier, net/pairing.rs for registry durability, net/devicespage.rs for approvals and files/imgroutes.rs for image authorization. net/pairflow.rs provides the human approval pattern. net/owner.rs permits an absent Origin for native clients and issues broader cookies, so the browser boundary needs its own checks.

The release decision includes session lifetime and whether the person enables a dedicated Serve mapping or supplies one. Validate these controls with synthetic sessions on an isolated test hub before enabling the feature.
