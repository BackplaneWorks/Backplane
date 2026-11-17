# KiCad pins without a Backplane release

A hub installs and offers the Backplane KiCad its pin names. The pins compiled into the hub (`native/host/src/kicad/install.rs`: `LINUX`, `MAC`, `WINDOWS`) are the floor. A newer pin published in the signed pin feed replaces one at run time, with no Backplane release. 0.16.0 waited 8 h for bp.13 and then shipped without it (docs/release-retro-0.16.0.md); with the feed, bp.13 would have reached 0.16.0 hubs the hour it was published.

## Publishing a pin (the KiCad lead)

Once the KiCad release `backplane-kicad-v<version>` is published on BackplaneWorks/Backplane_KiCad with its asset and `SHA256SUMS`, run one command per platform:

```sh
gh workflow run kicad-pins.yml -R BackplaneWorks/Backplane --ref main -f platform=linux-x86_64 -f version=10.99.0-bp.13 -f min_hub=0.16.1
```

`platform` is `linux-x86_64`, `darwin-arm64` or `windows-x86_64`. `min_hub` is the oldest Backplane that may take the pin. Raise it when the runtime needs something a hub only has from some version on. `days` (default 120) is how long the document stays valid.

The workflow (`.github/workflows/kicad-pins.yml`) runs only from main. Its signing job runs in the GitHub environment `kicad-pins`, which waits for a required reviewer (i2cjak) to approve the run (Actions, the run, Review deployments), accepts deployments from main only, and does not let admins bypass it. A first job checks that the environment still exists with a reviewer and that main-only policy, and nothing is signed otherwise: GitHub would create a missing environment with no protection. After approval the workflow:

1. downloads the published `kicad-pins.json` and verifies it with the release key in the tree;
2. checks the runtime the way `scripts/kicad-pin.py` does. It downloads the asset and checks that its sha256 equals the release's `SHA256SUMS` line. It checks that the manifest names this platform and version, and that Windows and macOS have `global-tables-seeded` and a library pin, and the Mac its `minOs`. It records the manifest's capabilities and the Mac's `minOs`;
3. writes the next document: the current pins with this platform's replaced, the sequence one higher, and a new expiry;
4. signs it with `BACKPLANE_RELEASE_KEY`, verifies the signature against `release.pub`, and uploads it as `kicad-pins.json` on the release `kicad-pins` of this repo (a prerelease, so it is never the latest release).

Hubs pick it up within 6 hours, at their next start, or the next time an editor opens at Edit (at most every 15 minutes). The update row above the pane and Settings, CAD then offer it like a compiled pin.

Per release, the KiCad lead:
- publishes the KiCad release as now;
- runs the workflow once per platform that gets the new runtime;
- before the document expires, runs it with `platform=none` to sign the same pins under a new sequence and expiry. The run summary shows the expiry date. A document that expires drops hubs back to their compiled pins. Nothing breaks, but nothing newer is offered;
- bumps the compiled pins (`scripts/kicad-pin.py <tag>`) in a normal change, so a new install that is offline at first start still gets a recent runtime. This no longer blocks a Backplane release.

## Revoking a bad pin (rolling back)

A bad pin is revoked with a higher sequence that points back. Run the workflow again for the platform with the earlier version (the next sequence, pointing at the old pin), or with an empty `version` to drop the platform, so hubs go back to their compiled pin:

```sh
gh workflow run kicad-pins.yml -R BackplaneWorks/Backplane --ref main -f platform=linux-x86_64 -f version=10.99.0-bp.13   # back to bp.13
gh workflow run kicad-pins.yml -R BackplaneWorks/Backplane --ref main -f platform=linux-x86_64 -f version=                 # the compiled pin
```

This stops the bad pin from being offered or installed. It does not uninstall a runtime a hub already installed: the hub runs the newest runtime in its folder, and the offer never goes down a version. To replace a bad runtime that is already installed, publish a fixed one with a higher number (bp.14.1).

## What the hub checks

`native/host/src/kicad/pins.rs` fetches `https://github.com/BackplaneWorks/Backplane/releases/download/kicad-pins/kicad-pins.json` at start, every 6 hours (15 minutes after a failed read), and when an editor attaches at Edit. It takes a pin only if all of these hold:

- The envelope `{"schema":1,"keyid","sig","doc"}` verifies with `update::rel::verify`, the same code and the same key (`release.pub`, id `6c899f869be656d3`) that check a Backplane release's manifest. Only that key is trusted. A build with feature `e2e-trust` also trusts an E2E key, and only for pins below 0.1.0. release.yml refuses a binary built with that feature.
- `min_hub` and `expires` are required: a pin without `min_hub`, or a document without an expiry, is refused whole.
- The body has `"kind":"backplane-kicad-pins"`. A release manifest signed with the same key has no kind, and a pin document has no `assets`, so neither passes as the other.
- The sequence is not below the last good document's. The same sequence is accepted only for the same document. The last good document is kept in `<home>/kicad-pins/pins.json` and verified again when it is read, so a replayed older document is also refused after a restart. An expired cached document still holds its sequence.
- The document has not expired.
- Every pin passes `install::check_package`: the version's characters, the release `backplane-kicad-v<version>`, the asset `backplane-kicad-<platform>-<version>.<format>`, a lowercase 64-hex sha256, a size up to 8 GB, a known platform, and well-formed capabilities and `min_os`. If one field fails, the whole document is refused.
- For this hub's pin: the platform is this computer's, the version is newer than the compiled pin of the same line (`install::older`), `min_hub` ≤ this hub's version, and this macOS is at least `min_os`.

If any check fails, or the feed cannot be read (offline, a 404, a GitHub outage), the hub keeps what it had: the last good document's pin, or the compiled pin. The log says why (`kicad: pins: ...`).

A pin taken from the feed is installed exactly as a compiled one is. The download comes only from the KiCad release the pin names on `github.com/BackplaneWorks/Backplane_KiCad`, because the release and asset names are fixed by the version. The download is checked against the pin's sha256 and the release's `SHA256SUMS`, and is unpacked with the same member, link and manifest checks. The format and launcher path are the platform's, never the document's.

The update row's text goes by the pin's capabilities where the pin lists them. `global-tables-seeded` says what bp.13 brought, and `embed-macos` says whether the pin can be shown in the Mac pane. A compiled pin lists none and goes by its number, as before.

## Threat notes

- **Who can publish.** Only someone who can run a workflow on main of BackplaneWorks/Backplane, and only after a required reviewer approves the run in the environment `kicad-pins`. The workflow refuses any other ref. The reviewer is the account the person's machines and agents use (i2cjak), so an agent holding that account's token could approve a run it started: the approval stops a dispatch by a stray token or another collaborator, not the account itself. The key stays a repository secret (release.yml reads it there), so another workflow on main could still read it, as before. The key is the repo secret, used only on GitHub, and nothing is signed locally. The offline copy of the release key (`~/.backplane/keys/release-signing.key`) can also sign a document, as it can sign a release.
- **A stolen or compromised GitHub account with write access** could run the workflow, but only with a version that has a published KiCad release whose asset matches its SHA256SUMS. It cannot point a hub at a file outside the Backplane_KiCad release. The same account could already publish a Backplane release, so the feed adds no new reach.
- **Compromise of the Backplane_KiCad release** (a replaced asset and SHA256SUMS): the pin's sha256 is signed in the document, so a replaced asset fails its check, as it does with a compiled pin.
- **A network attacker or a hostile mirror** cannot forge a document. A replayed older document is refused by sequence, and one older than the expiry by its expiry. Withholding the document (a freeze) only keeps the hub on its last good pin, or on the compiled one once that expires. The feed is a convenience: missing it costs only a newer offer.
- **A malformed but signed document** (a bug in the workflow) is refused whole, and the workflow checks the same rules before signing (`scripts/kicad-pins.py`).
- **A rollback by an attacker** is a replay and is refused. A rollback the lead makes is a higher sequence and stops offers, not installs (above).
- `BACKPLANE_KICAD_PINS_URL` (another feed, for E2Es and rigs) and `BACKPLANE_KICAD_PINS=off` come only from the environment, never from a setting a client can write. Another URL still has to serve a document signed by the release key.
- Each pin the hub takes is allocated once and kept for the hub's life (`pins::package_of`), so the memory used is bounded by the number of pins ever published.

## Testing

- Unit tests: `scripts/cargo-q test --offline -- kicad::pins` (in `native/host`) covers a bad signature, another key, an untrusted key id, a manifest passed as a pin document, replay, expiry, every malformed field, `min_hub`, another platform, the Mac's `min_os`, a served document taken and cached and read back after a restart, offline and 404, and the text driven by capabilities.
- E2E: `test/kicad-pins/e2e.sh BIN_DIR` with a hub built with `--features e2e-trust`. It signs documents with a key made for the run (the workflow's `scripts/kicad-pins.py` and openssl), serves them on loopback, and checks that the hub takes the pin, keeps it, and installs it through a stand-in installer that gets the feed's version and sha256. It also checks that the hub refuses another key, a body tampered with after signing, a test key pinning a real version, a replay after a restart, an expired document, and an unreachable feed.
