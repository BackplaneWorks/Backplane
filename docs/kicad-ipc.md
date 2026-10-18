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

Test: `bun test/tools/kicad_ipc_e2e.ts` (a stub server stands in for KiCad), `bun tools/kipc/test/kipc.ts`.
