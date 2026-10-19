# KiCad over IPC

Agents edit boards and schematics through the hub, not by starting their own KiCad. The hub runs one
headless `kicad-cli api-server` per project and sends commands to it over KiCad's IPC API (nng and
protobuf). Files and `kicad-cli` are the fallback, used only when IPC is unavailable, and every
fallback says so in the tool result.

## Pieces

- `src/core/kserve.bend` (pure, laws `kserve_*`): `KS.dir` (`$XDG_RUNTIME_DIR/backplane/kicad/<digest>`,
  else `<home>/run/kicad/<digest>`, one directory per project), `KS.argv` (`--socket <dir>/api.sock`,
  `--parent-pid <hub>`, `--idle-exit 600` when the runtime has `idle-exit`), `KS.ready` (the
  `KiCad API server listening at <path>` line), `KS.version_ok` (full version ends in the manifest's
  `-bp.N`), `KS.retry` (1, 5, 30 s within 300 s, then give up), `KS.plan` / `KS.evict` (two servers at
  most), `KS.headless_refused` (selection, view state, `RunAction`, ... answered by the hub at once).
- `src/server/kserve.bend`: the actor. A request for a project waits while its server starts (spawn,
  ready line within 30 s, `Ping`, `GetVersion`) and goes out when it is ready. A third project asks the
  least recently used server to `kiapi.backplane.Shutdown` first (only when the runtime lists `shutdown`).
  The servers' state is the info key `kicad.ipc.servers` (`project|state|why`, one line each); Settings
  shows it under the KiCad IPC row, and a `failed` server is a notice with Retry (`kicad.retry`).
- `tools/kipc/` (`backplane-kipc`, Bun): NDJSON on stdin/stdout. `{"id","sock","type","message"}` is a
  command; `{"id","op":"spawn","key","argv"}` starts a server and reports `{"event":"line"}` and
  `{"event":"exit"}`; `{"id","op":"close","sock"}` drops a connection. libnng comes from the runtime's
  `lib/`, then the system; the schema from the runtime's descriptor set (`--desc`, per request `desc`);
  without one only `Ping` and `GetVersion` work.
- MCP: `kicad_ipc {path, type, message}` and `kicad_ipc_status {path}` (`KIpc.*` in server.bend).

## Rules

- Nothing signals or kills a process. A server ends itself: `--parent-pid` (the hub went away),
  `--idle-exit` (ten minutes unused), `Shutdown` (made room). A server that never says it listens is
  marked failed and left to end with the hub.
- `--idle-exit` is passed only from `apiVersion` 2 (`KS.caps.safe`): bp.2 (API 1) closes a server with
  unsaved changes when it goes idle. A `Shutdown` refused for unsaved changes holds that server (`KS.hold`):
  it is never forced and never retired again until it is asked for; the other server is asked instead, and
  only when none can stop is the request answered "busy" with the fallback.
- IPC is used only when the runtime's health is `ok` (Backplane KiCad at least `KH.min_version()`, API
  `KH.min_api()`) and the setting `kicad.ipc` is not `off`.
- Persisted data is untouched: no new events. The server lines live in the hub's runtime record only.

- The pin is Backplane KiCad `10.99.0-bp.4` (API 2): `KH.min_version()` / `KH.min_api()`, and `KH.version.ge` orders
  the `-bp.N` suffix numerically (bp.2 < bp.3 < bp.4 < bp.10; `10.99.0-unknown-bp.3` reads as bp.3; no suffix is bp.0), so
  installs older than the pin are "old" and the notice offers Update. Discovery checks the pinned
  `<data>/kicad/<KiCad.version()>` first, then other managed versions newest-first. Older runtimes offer Update
  and cannot shadow the bp.4 install.
- The server is always started through the runtime's `bin/kicad-cli` (`KS.launcher` turns `kicad-cli.real` back into
  the launcher), which defines `KICAD10_*_DIR`. `kicad_ipc_status` shows the start command line (`argv`) and `version`.
- Compatibility guards, in the hub (`KS.guard`): `UpdateItems` of a `FootprintInstance` without `definition` on bp.3 wipes the footprint's
  pads and 3D models, so it is refused unless the runtime lists `footprint-partial-update`; `CreateItems` of a schematic
  symbol with only a library id makes an empty symbol, so it is refused unless the runtime lists `symbol-from-libid`:
  `LoadAllLibraries`, wait for `GetLibraryStatuses`, then `PlaceSymbolFromLibrary`.

Test: `bun test/tools/kicad_ipc_e2e.ts` (a stub server stands in for KiCad), `bun tools/kipc/test/kipc.ts`.

## Real runtime e2e

`test/tools/kicad_ipc_real_e2e.ts` runs the whole path against a real Backplane KiCad: a hub on a free port and a temp
home, the real `backplane-kipc`, the demo `ecc83` copied from the runtime, and only the hub's MCP tools.

```sh
bun test/tools/kicad_ipc_real_e2e.ts build/backplane-serve build/wire /path/to/backplane-kicad-linux-x86_64-10.99.0-bp.4
```

Without a runtime (or without `bin/kicad-cli` in it) it prints `skipped: no runtime` and exits 0. It checks health and
status, footprint/pad/3D-model counts, an edit saved and reloaded, a partial footprint update preserving contents and attributes, a netlist import, symbols
placed and created from library IDs without bulk preload, a rolled-back commit and the server's argv. Each check is an `ok`/`FAIL` line. It stops the hub
it started; the KiCad server ends with it (`--parent-pid`).

Health caches are checked against the current minimum even after an app update. Their file stamps cover the selected
runtime and candidates ahead of it; adding unrelated PATH entries after it does not invalidate healthy startup state.

The bp.4 runtime advertises both fix capabilities: partial footprint updates and symbol creation from a library ID
are accepted. Symbol libraries load on demand; agents need no blanket library preload on this runtime.

The installer checks the pinned bp.4 asset against both the release SHA256SUMS and the verified SHA-256
`f60a71817d057f9dcf3faef632b2f85d06a613108dee23807c9de81aae97e0b4`.

KiCad can buffer its ready line until exit when stdout is a pipe. The helper discovers a fresh socket in the
project's private socket directory, including KiCad's `api-<pid>.sock` fallback, and the hub then requires Ping
and a matching version. Sockets present before launch never count as readiness for that launch.
`bun tools/kipc/test/readiness.ts` checks buffered readiness with a fresh requested socket, an occupied requested
path plus a new alternate, and an occupied path with no new socket.

When the pinned managed install is absent, discovery retains other installed versions as newest-first fallbacks.
An old install therefore reports old and offers Update; the pinned directory appearing invalidates the health cache
and takes precedence.

Managed discovery and installation are platform aware. The pinned bp.4
release publishes only a Linux x86-64 runtime; a known Apple-silicon bundle
layout does not imply an available Mac download. On macOS the installer
reports that missing asset and preserves the selected runtime. A local
KiCad.app remains available for checks and exports; stock KiCad does not
provide Backplane's patched IPC API. Managed manifests must match the host;
legacy manifests without a platform are treated as Linux x86-64 only.
Apple-silicon managed directories use a `-macos-arm64` suffix. A managed
runtime that fails its version probe falls through to working local copies;
an explicit path remains selected so its failure is visible.

Downloads are checked using `sha256sum` or macOS's `shasum -a 256`, then
extracted and smoke-tested inside the per-destination lock. The version
command identifies upstream KiCad (`10.99.0` for bp.4); the manifest and live
`api-server --describe` identify the fork (`10.99.0-bp.4`). Bend parses both
JSON objects, requiring the pinned fork version, matching source commit,
upstream core, platform, API 2, descriptor path and headless capabilities. The previous
runtime is moved only after validation, restored if promotion fails, and a
per-destination lock rejects concurrent installers. Failed downloads,
checksums, extraction, version checks, and promotion preserve the prior
runtime's bytes. The IPC helper must be installed beside Backplane and
answer `--describe` with protocol version 1 and runtime API 2; incompatible
or missing helpers fail visibly before receiving persistent requests.

Offline regression validation uses the production Bend selector and shell
transaction with synthetic runtimes, archives, checksums, and fault tools:

```sh
bend test/kicadcli_test.bend
bend test/kipc_protocol_test.bend
BACKPLANE_LOW_MEM=1 BACKPLANE_JOBS=1 scripts/build-app.sh test/native/kicad_platform.bend build/kicad_platform
bun test/tools/kicad_platform_e2e.ts build/kicad_platform
```

These fixtures simulate platform responses; they do not certify a Mac
launch, board export/view, or IPC session. Actual macOS acceptance still
requires an accessible Mac and a compatible published or locally built fork.
