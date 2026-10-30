# Service release producer

`scripts/service-release.py` supplies the genuine service component consumed by
the desktop producer and the matching emitted web consumed by the web producer.
It does not install, sign, invoke CI, run a server, change a live home, or publish.
The only payload execution is the new service's `--version` check, and only on its own target.

```sh
python3 scripts/service-release.py describe
python3 scripts/service-release.py js --request /absolute/request.json --out /absolute/new-build
python3 scripts/service-release.py component --request /absolute/component.json --out /absolute/new-component
python3 test/tools/service_release_test.py
```

The service is the JS one (`docs/js-service.md`): `scripts/build-js.sh` emits
**src/server/main.bend** once and `bun build --compile` links it onto the
target's own Bun runtime. There is no C path. Targets are `linux-x64`,
`linux-arm64`, `darwin-x64`, `darwin-arm64` and `windows-x64`; every one
cross-builds from any host, since nothing is compiled for the host. The service
file is `backplane-serve`, and `backplane-serve.exe` on Windows (build-js.sh
appends `.exe`). Windows builds but cannot serve until Bend's JS runtime has
Win32 syscalls.

The helpers the hub runs (`src/server/kicad.bend`'s `Helper.find`:
`backplane-kipc`, `-step2glb`, `-voice`, `-browser`) sit beside the service.
On Linux and macOS `js` builds them for the target on its pinned Bun runtime
(`scripts/build-helpers.sh`, their notices in `licenses/helpers/`) and records
their hashes in BUILD.json's `helpers`; `component` copies them executable and
lists them as packages. Windows' (`backplane-kipc.exe`, `backplane-step2glb.exe`)
come in its pinned runtime bundle (`scripts/service-runtime.py`). Each step
refuses a missing one, a wrong architecture or, on macOS, an unsigned one, and so
do the desktop producer and `scripts/check-helpers.py ARCHIVE --target T`, which
`release.yml` runs on every staged and archived desktop app. 0.13.0's macOS and
Linux apps shipped none, and KiCad IPC never started (issue #510).

The checkout must be clean at the final committed release identity, checked
before and after the build. Inputs and outputs should live outside the checkout
or in an ignored build directory. Source check receipts must already refer to
this same commit and tree; this producer does not manufacture a whole-proof pass.

A separately pinned execution helper may use `--source-root ABS_FROZEN_CHECKOUT`
to operate on an unchanged product freeze. The same clean commit/tree and full
source-byte checks still apply. BUILD records the helper's hash and copies its
exact bytes as `EXECUTION-HELPER.py`; component assembly retains that provenance.

A build request has this shape (all artifact references use actual values):

```json
{
  "schema": 1,
  "kind": "backplane-service-js-inputs",
  "target": "windows-x64",
  "release": {
    "version": "0.12.0", "tag": "v0.12.0", "channel": "release",
    "commit": "FULL_FINAL_COMMIT", "tree": "FULL_FINAL_TREE"
  },
  "toolchain": {
    "bend": {"path": "/absolute/bend", "sha256": "ACTUAL_SHA256", "bytes": 0, "version": "EXACT_VERSION_OUTPUT"},
    "bun": {"path": "/absolute/bun", "sha256": "ACTUAL_SHA256", "bytes": 0, "version": "1.4.3"},
    "bunRuntime": {"path": "/absolute/bun.exe", "sha256": "ACTUAL_SHA256", "bytes": 0, "version": "1.4.3", "target": "windows-x64"}
  },
  "sourceQualification": {"path": "/absolute/qualification.json", "sha256": "ACTUAL_SHA256", "bytes": 0},
  "webNotices": {"path": "/absolute/web-notices.json", "sha256": "ACTUAL_SHA256", "bytes": 0}
}
```

`bend` is checked with `bend version`, `bun` with `--version` (Bun 1.x). The
`bunRuntime` is the target's own Bun executable, of the pinned Bun's version;
its header must be the target's (ELF64 x64/ARM64, x64/arm64 Mach-O, or an AMD64
PE: `MZ`, `PE\0\0` at `e_lfanew`, machine `0x8664`). It goes to
`bun build --compile-executable-path`. Build-js.sh's `date +%s%N` needs GNU date
in PATH (Darwin: coreutils `gnubin`).

References resolve relative to the containing request, or may be absolute. The
source qualification uses the desktop web producer's schema:
`schema:1`, `kind:backplane-web-source-qualification`, `status:PASS`, `commit`,
`tree`, `version:0.12.0`, and nonempty `checks` containing references to actual
final source/entry/proof logs. Their bytes are checked, copied and rebound.
`webNotices` is a source-bound `schema:1`, `kind:backplane-web-source-notices`,
`sourceCommit`, `sourceTree`, `licenses` mapping web destination names to
checkout files (tracked, or ignored under `build/` for external notices), and
`packages`. Each package has `name`, `version`, `sourceDisposition`, and nonempty
`licenses:[{path,sha256}]` relative to web. Supply all redistribution notices for
the actual emitted assets. No license is inferred from a binary or silently downloaded.

Protected configuration is passed only through the build environment:

* `BACKPLANE_GOOGLE_CLIENT_ID` and `BACKPLANE_GOOGLE_CLIENT_SECRET`;
* `BACKPLANE_TELEMETRY_URL=https://backplaneworks.com/api/v1/events`;
* `BACKPLANE_TELEMETRY_KID` matching `rel-[a-z0-9-]{1,28}`;
* `BACKPLANE_TELEMETRY_KEY`, 64 lowercase hex characters.

Missing configuration fails before anything is built. The build sets both bakes
to 1, version and stamp to `0.12.0`, telemetry channel to `release`, and turns
the emit cache off; an inherited `BACKPLANE_JS_TARGET`, `BACKPLANE_JS_BUN_RUNTIME`
or `BACKPLANE_COMMIT_STAMP` is dropped, so only the request picks the target.
The credentials go into build-js.sh's private prelude, which is removed after
the compile (and again by this producer). Public BUILD contains only the agreed
boolean configuration witness and public endpoint; it contains no credential
values or credential hashes. Build-js.sh's "baked Google client" line and every
protected value are removed from the logs.

Build outputs are the service file, original `BUILD.json`, `SOURCE.json`,
`WEB.json`, `WEB-EMISSION.json`, `web/`, source qualification and sanitized
logs; anything else build-js.sh left beside the service fails the build. The
service header is checked for the target, and on its own host its actual
version must be `backplane 0.12.0`; a cross-built one is run by runtime
acceptance on its target. Web emission uses the production Bend root,
unmarshal/native postprocessors and service worker; every HTML/JS/CSS file gets
a deterministic gzip sibling checked against the complete decoded bytes. WEB
schema1 declares every member, including all notice files. No fonts are added.

`BUILD.status` is `VERIFIED_GENUINE_SERVICE_ONLY_CONFIGURED_BUILD`. The original
BUILD records `target`, `entry:src/server/main.bend`, `source_commit`,
`source_tree`, `version`, `versionStamp`, `telemetryChannel`, `service` (the
file name), `binary_sha256`, `serviceRuntime:"bun"`, source/web manifest hashes,
the `bend`/`bun`/`bunRuntime` identities and sanitized log references. Its
`compileInputs` is exactly:

```json
{
  "version": "0.12.0", "versionStamp": "0.12.0", "telemetryChannel": "release",
  "google": {"requested": true, "requiredInputsPresent": true},
  "telemetry": {"requested": true, "requiredInputsPresent": true,
    "endpoint": "https://backplaneworks.com/api/v1/events"}
}
```

This witness is written only after a successful configured build. Existing
development BUILD receipts cannot acquire it through component assembly.

Offline component assembly takes `schema:1`,
`kind:backplane-service-component-inputs`, the same `target` and `release`,
`buildDirectory`, a reference `buildReceipt` to its original BUILD,
`runtimeDirectory`, and a reference `runtimeQualification`. The BUILD must say
`serviceRuntime:"bun"` and name the target's service file. Runtime approval is
`schema:1`, `kind:backplane-service-runtime-qualification`, `status:PASS`,
`target`, `sourceCommit`, `sourceTree`, `binarySha256`,
`runtimeManifestSha256`, `closureSha256`, and exact `members` for runtimeDirectory.
It comes from `scripts/service-runtime.py` on a matching host after actual
dependency measurement (ELF, Mach-O or PE imports, and the libraries the service
opens through `bun:ffi`). It is not a substitute for installed product runtime acceptance.

Runtime input contains `RUNTIME.json`, `CLOSURE.json`, libraries and notices,
without colliding with service/web files. RUNTIME has `target`, nonempty
`inventory:[{path,package,version,architecture,sha256,bytes}]` and `packages` with
the license shape above. CLOSURE has `target`, `measured:true`, `status:PASS`,
`unresolvedMandatory:[]`, and `edges:[{from,kind,name,...}]`. Edge `kind` is
normal, delay or dynamic. Every edge is either `system:true` with
`classification:approved-system`, or resolves to an exact supplied file via
`resolved`. The measured edges refer to the final combined payload, from the
service file. This wrapper never invents an empty measurement or resolves a
library by searching the build machine.

Component output `payload/` preserves original BUILD bytes, service/web/source,
runtime files and approval, plus the copied source logs and web emission evidence.
Internal provenance references are relative to their containing JSON so the
component and standalone web evidence survive transfer to another host.
Adjacent `SERVICE.json` is schema1 `backplane-bend-service`, `status:VERIFIED`,
with target, commit/tree, `version:0.12.0`, `channel:release`,
`genuineService:true`, server entry, `roles:{service:SERVICE_FILE,web:web}`,
original BUILD reference, complete member hashes/sizes/modes, runtime/closure
hashes and package notices. The desktop producer consumes
`{receipt:REFERENCE,payload:{directory:PAYLOAD}}`, Windows included. The
standalone web producer consumes build `web/` with `WEB.json` and
`WEB-EMISSION.json`, pinned in the final release freeze.

Both operations leave installed-runtime acceptance pending and `published:false`.
`.github/workflows/release-bootstrap.yml` (operation `js`) runs the build for
every target on Linux and the measurement and assembly on each target's runner.
