# Release signing key

`release.pub`: ed25519 public key (PEM, SPKI). `release.keyid`: its ID, the first 16 hex of SHA-256 over the raw 32-byte key; the workflow writes it to `manifest.json.keyid`.

Releases sign `manifest.json` with `openssl pkeyutl -sign -rawin`: `manifest.json.sig` is the raw 64-byte ed25519 signature over the exact manifest bytes. Verify in pure Rust with the 32-byte key (the last 32 bytes of the DER in `release.pub`).

The private key is the repo secret `BACKPLANE_RELEASE_KEY` (no password) and an offline copy at `~/.backplane/keys/release-signing.key`. Never commit it.
