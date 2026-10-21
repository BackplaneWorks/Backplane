# Desktop release producer

The callable v2 producer assembles supplied, verified artifacts for exactly
`linux-x64`, `linux-arm64`, `darwin-arm64`, and `windows-x64`. It never fetches,
compiles, executes a payload, signs, installs, starts CI, or publishes.
Node and Python 3 standard libraries are its only assembly dependencies.

```sh
node desktop/produce-release.cjs describe
node desktop/produce-release.cjs stage --request REQUEST.json --output NEW_ABS_STAGE
node desktop/produce-release.cjs inspect --stage ABS_STAGE
node desktop/produce-release.cjs archive --stage ABS_STAGE --acceptance ACCEPTANCE.json --output NEW_ABS_DIST
```

`stage` emits `STAGE.json` with `STAGED_UNQUALIFIED`, an exact content inventory,
and its digest. It creates no public archive. The target owner signs and validates
that exact stage, then supplies `ACCEPTANCE.json`. `inspect` supplies the current
payload digest after signing. `archive` validates that digest and target evidence,
preserves the original stage, and writes a deterministic archive plus SHA256
sidecar, inner `DESKTOP.json`, `MANIFEST.json`, and `NOTICES.json`. Its JSON stdout
names those outputs. It never reports publication or infers platform acceptance
from assembly. A failed output directory must be discarded; output paths must be
new absolute paths.

| Target | Archive | Root and launch path |
| --- | --- | --- |
| linux-x64 | backplane-v0.12.0-linux-x64.tar.gz | backplane-linux-x64/backplane-open |
| linux-arm64 | backplane-v0.12.0-linux-arm64.tar.gz | backplane-linux-arm64/backplane-open |
| darwin-arm64 | backplane-v0.12.0-darwin-arm64.tar.gz | backplane-darwin-arm64/Backplane.app/Contents/MacOS/Backplane |
| windows-x64 | backplane-v0.12.0-windows-x64.zip | backplane-windows-x64/Backplane.exe |

Linux's Electron lives in `runtime/`; macOS uses `Backplane.app`; Windows uses
the archive root. Their resources contain `app/`, `backplane-service/`, and
`DESKTOP-BUILD.json`. Electron configuration stays separate from the preserved
native service `BUILD.json`. Packaged launch uses the user's Backplane home;
checkout launch continues to require an explicit isolated home. Window close
preserves the resident service. Windows selects `backplane-serve.exe`; its
managed shutdown may remain explicitly unsupported under the resident-service
policy below; it is not a release requirement for that policy.

Linux portable archives require either working **unprivileged user namespaces**
(with the `unshare` command from util-linux), or an administrator-installed
`runtime/chrome-sandbox` owned by `root:root` with mode `4755`. Start them with
`./backplane-open`: it checks both routes before starting Electron. If neither is
available it exits nonzero and prints one safely quoted command, using the actual
absolute helper path, for an administrator to review and run. The launcher never
runs `sudo`, changes permissions or falls back to `--no-sandbox`.

The tarball carries the upstream helper with ordinary mode `0755`; installing the
setuid helper is an explicit administrator action after extraction. On Ubuntu
23.10 and later an AppArmor profile allowing user namespaces for the Electron
binary may also be needed. Root-installed packages can set up that profile and
the helper during installation. These host choices do not change the source-bound
archive or imply that every Linux security policy has been qualified.

The hosted-runtime workflow loads an AppArmor userns profile only for its exact
extracted Electron path and `/usr/bin/unshare` in the disposable Linux runner.
It keeps the host-wide restriction and the stage's `0755` helper mode unchanged,
so the existing stage/acceptance/archive integrity checks still apply. See
[Ubuntu's user namespace profile guidance](https://ubuntu.com/blog/ubuntu-23-10-restricted-unprivileged-user-namespaces).

## Inputs

`release-inputs.schema.json` describes request and acceptance envelopes. Every
artifact reference is `{path,sha256,bytes}`. Relative paths, including a component's
`payload.directory`, resolve from the JSON that contains them. Archives can be
supplied as `payload.archive` plus a safe optional `payload.root`; no host runtime
or architecture is inferred. Component receipts remain byte-for-byte unchanged.

A desktop request has schema 1, kind `backplane-electron-release-inputs`, `target`,
`release`, `shell`, `electron`, and `service`. `release` contains exactly the actual
stable `0.12.0`, `v0.12.0`, `release` channel, full Git `commit` and `tree`, and a
`freezeReceipt` reference. The freeze has schema 1, kind
`backplane-release-freeze`, status `FROZEN`, the same five identity fields,
`sourceDateEpoch` (1980 through 2107), `signingPolicy[target]` (`required` or
`unsigned-portable-allowed`), `compileInputs`, and `components[target]` with exact
`shellReceiptSha256`, `electronReceiptSha256`, `webManifestSha256`, and either
`serviceReceiptSha256` or Windows `serviceArchiveSha256`.

Generic components have `{receipt,payload}`. Their schema 1 receipts have status
`VERIFIED`, the exact target, `sourceCommit`, `sourceTree`, version and channel,
and a complete `members` map. A regular member is
`{type:'file',sha256,bytes,mode}`; a link is `{type:'symlink',target}`.
The kinds are `backplane-electron-shell`, `backplane-electron-runtime`, and
`backplane-bend-service`. Shell/runtime both pin `electronVersion: '44.9.2'`.
The shell must include actual `package.json` name `backplane-desktop`, main
`main.cjs`, version `0.12.0`, and `features` source qualification for
`defaultPackagedHome`, `contextIsolation`, `sandbox` true and `nodeIntegration`
false. macOS needs `bundleIdentifier`. Windows needs verified startup path
behavior (`windowsNativePathMapping: true`). A managed-stop policy also needs
the implemented protocol in `features.windowsManagedControl`; resident policy
instead requires `windowsManagedControl: 'UNSUPPORTED'`, `managedStop: false`
and `managedUpdate: false`.
`features.directBrowser` enables the supplied guest/preload files and their pins.

The upstream Electron payload has `electron`, `electron.exe`, or `Electron.app`.
Its receipt's `signablePaths` lists the actual native files permitted to change
when signed. Electron and service receipts both name `runtimeManifest`,
`runtimeManifestSha256`, `closureManifest`, and `closureSha256`.

A generic service receipt requires `genuineService: true`, `entrypoint:
'src/server/main.bend'`, `roles: {service:'backplane-serve',web:'web'}`, and a
`buildReceipt` reference to the original `VERIFIED_GENUINE...` receipt. That raw
receipt must also be present byte-exact as service `BUILD.json`, with `target`,
`entry`, stable `version`, `source_commit`, `source_tree`, and `binary_sha256`.
It must record `versionStamp: '0.12.0'`, `telemetryChannel: 'release'`, and this
public tuple, identical to `freeze.compileInputs`:

```json
{
  "version": "0.12.0", "versionStamp": "0.12.0", "telemetryChannel": "release",
  "google": {"requested": true, "requiredInputsPresent": true},
  "telemetry": {"requested": true, "requiredInputsPresent": true,
    "endpoint": "https://backplane.works/api/v1/events"}
}
```

This witness records actual configured build inputs; an adapter must never add it
to an old build. No secret values or secret hashes belong in receipts.

`RUNTIME.json` has the exact `target`, nonempty `inventory` entries with
`path,package,version,architecture,sha256,bytes`, and dependency `packages`.
`CLOSURE.json` has the exact target, `measured: true`, status `PASS`, no
`unresolvedMandatory`, and `edges` with `from`, `kind` (`normal`, `delay`,
`dynamic`), `name`, and either `system: true, classification: 'approved-system'`
or an existing `resolved` payload path. Shell, Electron, service and runtime
`packages` retain name/version, nonempty `licenses: [{path,sha256}]`, and a
`sourceDisposition` for each redistribution/source obligation.

`WEB.json` has schema 1, kind `matching-generated-web`, exact `sourceCommit` and
`sourceTree`, entrypoint `src/web/index.html`, and exact web-relative `members`
with SHA256/byte counts. Every HTML/JS/CSS has a gzip sibling, and every gzip
roundtrips exactly. No extra assets, bundled fonts, homes/profiles, special files,
privileged modes, escaping links, or case-colliding members are admitted.
ELF/Mach-O/PE target headers are checked without executing them.

## Windows resources

Windows `service` is `{contractSha256,archive}` using the unchanged contract
`1c12bd80b6f994ecd0d68d06b8ada98e1958de725d33f6380da3c0243ad099e3`.
The ZIP must have only the `backplane-service/` root and original schema 3
`SOURCE.json`/`BUILD.json`, plus `WEB.json`, `RUNTIME.json`, `CLOSURE.json` and
native payload. The producer mounts that root exactly once and preserves all
five receipts byte-for-byte. Native version/stamp/channel/source, complete
member sizes/hashes, every PE normal/delay import edge, runtime licenses and
closure are checked. Without an explicit resident policy, `BUILD.managedControl`
requires status `IMPLEMENTED`, protocol and validation receipt hash. Under the
explicit resident policy, original `UNSUPPORTED` with null protocol is preserved
and allowed; no acceptance receipt is invented.
No lock PID is treated as a native process identity or signalled.

## Target acceptance and archive

`ACCEPTANCE.json` binds `target`, `stageReceiptSha256`, and the inspected final
`payloadManifestSha256`, plus four artifact references. Each proof has schema 1,
status `PASS`, target, version, commit, tree, that payload digest and
`freezeReceiptSha256`:

| Reference | Kind | Required target evidence |
| --- | --- | --- |
| signing | desktop-signing | boolean signed; true if frozen policy requires it |
| sandbox | desktop-sandbox | osEnforced true; actual flags array without --no-sandbox |
| lifecycle | desktop-lifecycle | nativeChildIdentity/authBound/singleInstance/reconnect true; windowClose resident-service; explicit resident-policy evidence below, or existing owned-native-child/approved-service-protocol shutdown evidence; Windows startup paths still require nativeToPosixPaths true |
| runtimeAcceptance | desktop-runtime-acceptance | scope desktop-release; actual target owner acceptance |

A root freeze may explicitly select this shipping policy per target:

```json
"lifecyclePolicy": {
  "windows-x64": {
    "mode": "resident-service", "managedStop": false, "managedUpdate": false
  }
}
```

It permits normal window close to leave the backend resident. The actual lifecycle
proof must additionally have `startup: true`, `closePreservesService: true`,
`shutdown: 'unsupported'`, `managedStopAdvertised: false`, and
`managedUpdateAdvertised: false`. Root policy, stage policy and final DESKTOP
policy must match exactly. It never substitutes for actual startup/auth/single
instance/reconnect, Windows path, OS sandbox or target smoke acceptance.
`DESKTOP-BUILD.json` carries the policy. Main passes it to Service, which starts
owned resident services with `BACKPLANE_NO_UPDATE=1`; no stop/update IPC is added.
Attached services still need the actual target lifecycle proof. No PID is killed
on close. Managed shutdown may be implemented separately without delaying this
shipping policy. Explicit `unsigned-portable-allowed` signing policy permits an
honest `signed: false` disposition; commercial codesigning/notarization is not
required unless the root freeze selects `required`.

Only frozen Electron native signing paths and macOS `_CodeSignature` files may
change after staging. App/service/web changes, mode changes, deletions or new
links fail. `DESKTOP.json` schema 1 kind `backplane-electron-desktop-package`
contains release identity/freeze reference, component provenance, exact versions,
web/notices hashes, the four embedded proof references and exact members.
`MANIFEST.json.files` additionally binds DESKTOP.json, excluding only itself.
Original component receipts and actual build provenance are under `PROVENANCE/`.
Release orchestration exposes the exact inner receipts as needed; root alone
installs and publishes. Synthetic offline fixtures prove adapter behavior only.

## Standalone web artifact adapter

```sh
node desktop/produce-web-release.cjs describe
node desktop/produce-web-release.cjs archive --request REQUEST.json --output NEW_ABS_DIST
```

This wrapper consumes emitted artifacts and never invokes `scripts/build.sh` or
Bend. Request kind is `backplane-web-release-inputs`, target `web`, with the same
stable release/freeze identity, a `web: {receipt,payload}` component and
`emissionReceipt`. The freeze binds `components.web.webManifestSha256` and
`emissionReceiptSha256`. The emission receipt has schema 1, kind
`backplane-web-emission`, status `VERIFIED`, target web, exact source/version/
channel, entrypoint `src/web/index.html`, `webManifestSha256`,
`compiler: {version,sha256}`, `packages` whose license paths belong to the exact
web manifest, and `sourceQualification` reference. That qualification has schema
1, kind `backplane-web-source-qualification`, status `PASS`, exact `commit`,
`tree`, `version`, and nonempty `checks` containing actual log artifact refs.
The shared source owner must supply those real emitted/compiler/check artifacts;
this producer cannot manufacture them or substitute dev web assets.

Output is `backplane-v0.12.0-web.tar.gz` rooted at `backplane-web/`, a digest
sidecar and external `WEB-PACKAGE.json` (schema 1, kind
`backplane-target-package`, target web). It binds the exact archive/members and
matching web rooted at `backplane-web/web`. Raw WEB.json, provenance/checks and
notices are retained inside. `releaseQualified: false` explicitly leaves actual
web runtime and full release coverage to the release owner.

Narrow offline checks:

```sh
node --test desktop/test/release-producer.test.cjs desktop/test/web-release-producer.test.cjs
```
