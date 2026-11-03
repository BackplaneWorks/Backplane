# Pairing UX research

Checked 2026-11-02 against the upstream documentation below. These describe current upstream behavior, not assumptions based on Backplane's former T3Code ancestry.

## T3 Code

The host exposes Settings → Connections → Network access and creates a fresh pairing link; `t3 pair` also prints a QR code for a running host. The recipient scans it or pastes it into Add environment. Each device gets its own lasting session after exchanging a one-time token. Hosts can revoke unused invitations and individual sessions. T3 Connect is a separate account-based connection option. Sources: [remote access](https://github.com/pingdotgg/t3code/blob/main/docs/user/remote-access.md), [welcome wizard](https://github.com/pingdotgg/t3code/blob/main/docs/user/welcome-wizard.md).

Its architecture separates machine identity from addresses. LAN/tailnet addresses are reachability hints, checked before credentials are sent. Hosted pairing URLs keep the secret in the fragment, exchange it directly with the environment and remove it from browser history. An HTTPS UI cannot make an HTTP backend reachable through mixed-content restrictions. Source: [remote architecture](https://github.com/pingdotgg/t3code/blob/main/docs/internals/remote.md). The docs do not establish that every native platform implements automatic installed-app fallback; do not claim that capability from the link format alone.

## VS Code Remote Tunnels

`code tunnel` starts the host server and supplies a machine-specific `vscode.dev/tunnel/...` URL. The first browser connection authenticates to the account; both ends use the same GitHub or Microsoft account. Remote Explorer also lists machines and supports unregistering them. Outbound Azure tunnel connections avoid requiring inbound firewall changes. Source: [official Remote Tunnels guide](https://code.visualstudio.com/docs/remote/tunnels).

## Syncthing

Each device has a generated public-key-derived device ID, available under Actions → Show ID. Add Remote Device asks for the other device's ID and an optional name; both ends must authorize each other. This makes the trust ceremony explicit, but an ID is public identification, not a secret invitation or an expiring code. Source: [official Getting Started guide](https://docs.syncthing.net/intro/getting-started.html#configuring).

## Backplane design decisions

Use the proposed dedicated connection screen and Settings → Devices invitation ceremony, borrowing T3 Code's separation of disposable invitations and durable device sessions. A QR code contains exactly the same invitation link as Copy/Share. Keep invitation secrets in URL fragments and erase them before any network or diagnostic work. A short code requires a known hub address or discovery: eight characters cannot independently encode arbitrary LAN/tailnet routes without a rendezvous service. Ask for the hub address alongside a code when it is not already known; do not invent a cloud resolver.

Use ten-minute, single-use invitations and eight-character random codes with hub-wide attempt throttling. Store individual device credentials separately from invitations and the legacy hub credential; revocation must invalidate access without rotating the hub token or discarding other clients. Distinguish authentication failure from unreachable/offline status. Preserve saved pairings on failed exchanges.

An HTTPS link can serve the web pairing page and offer an explicit Open in Backplane action. Automatic app dispatch additionally needs working domain associations on mobile and protocol registration on desktop; a custom scheme alone has no browser fallback. Do not promise universal links without those deployment files and platform entitlements.

## Existing implementation correction

PR [#754](https://github.com/i2cjak/Backplane/pull/754) preserves the existing hub token on read/permission/interrupted-create errors; it does **not** implement expiring or single-use invites. Current `src/core/auth.bend` compares a shared token and `src/web/auth.js` exchanges it for a cookie. Preserve #754's initialization behavior while adding a separate invitation/session layer; do not describe the existing token as disposable.
