# KiCad over IPC

KiCad discovery in a linked Git worktree defaults to the main checkout's
project folder and `.backplane.json`. Git's common directory and registered
main worktree establish the link; a project below the repository root keeps
its relative subfolder. An ordinary project folder is unchanged. Absolute
`project`, `pcb` and `schematic` entries in the thread's `.backplane.json`
override the main configuration, preserving explicit external designs.

Open in KiCad on another machine (the viewer bar's button in a window that cannot hold KiCad, and the KiCad pane's "Open the KiCad
app instead"; action `kicad-open`, RPC `kicad.open {thread}`; `kicad/open.rs`. In a window on the hub's own machine the bar's
Open in KiCad is the editor in the pane, docs/embedded-editors.md) starts the
desktop KiCad beside the hub's `kicad-cli` on the thread's own design, detached: the project manager on its `.kicad_pro`,
else the board editor on the board, else the schematic editor. It opens the files the viewer shows (a worktree thread's
copy); 0.14's choice between main and the thread's copy (Open main, Open this thread's copy, Cancel) is not ported. How it
went is a line in the thread ("Opened … in KiCad", or why not: no project, KiCad not found, no editor beside `kicad-cli`).

Viewer defaults, `viewer_show`, `kicad_ipc` and `kicad_ipc_status` use the same
canonical files. MCP document paths inside the thread's worktree map to main;
nested absolute IPC document paths follow the same mapping. Omitting `path`
selects the canonical project for IPC, or the board/schematic for the viewer.
`thread_copy: true` explicitly selects the thread's files instead. STEP and
other non-KiCad files keep their paths. No worktree is merged or copied back.

IPC **writes require an explicit target when the linked designs differ**.
`BeginCommit`, `CreateItems`, `UpdateItems`, `DeleteItems`, `EndCommit`,
`SaveDocument`, document lifecycle commands, routing and other mutations are
refused before reaching the IPC actor unless the request passes
`thread_copy: true` for this thread or `main: true` for canonical main.
The error names both selected paths and both flags. Pass the chosen flag on
**each** mutating call; it is never remembered for later commands. Passing
both flags is an error. Absolute configuration overrides remain respected.

When working on the thread copy, pass **`thread_copy: true` on every IPC
call**, including `GetItems` and other reads, as well as commits, edits and
saves. Without that flag, reads select main: the returned items and KIIDs may
not identify the same items in the thread copy. Keep the target consistent
throughout the workflow. After a write refusal, ask the person which copy is
current before choosing `main: true`.

Reads still default to main. Identical copies and non-worktree threads keep
their existing write behavior. The read-only command list is explicit;
unknown/new commands require the write choice too, including unknown commands
whose names merely start with Get or List.

The exact read-only allowlist is `Project.reads` in
`kicad/project.rs`. It matches full protobuf type names, including
legacy board/schematic GetItems spellings; an unknown namespace is not allowed
merely because its command has a familiar leaf name.

Regression coverage: the tests in `kicad/project.rs`. The filesystem tests use actual linked Git
worktrees, including nested project folders and separate Git metadata.

Agents edit boards and schematics through the hub, not by starting their own KiCad. The hub runs one
headless `kicad-cli api-server` per project and sends commands to it over KiCad's IPC API (nng and
protobuf). Files and `kicad-cli` are the fallback, used only when IPC is unavailable, and every
fallback says so in the tool result.

## Pieces

All in `native/host/src/kicad/`:

- `kserve.rs` (pure rules): the project's socket directory (`$XDG_RUNTIME_DIR/backplane/kicad/<digest>`,
  else `<home>/run/kicad/<digest>`, one per project), `argv` (`--socket <dir>/api.sock`,
  `--parent-pid <hub>`, `--idle-exit 600` when the runtime has `idle-exit`), the ready line
  (`KiCad API server listening at <path>`), the `-bp.N` version check, `retry` (1, 5, 30 s within 300 s,
  then give up), `plan` / eviction (two servers at most), `headless_refused` (selection, view state,
  `RunAction`, ... answered by the hub at once) and `guard`.
- `ipc.rs`: the actor. A request for a project waits while its server starts (spawn,
  ready line within 30 s, `Ping`, `GetVersion`) and goes out when it is ready. A third project asks the
  least recently used server to `kiapi.backplane.Shutdown` first (only when the runtime lists `shutdown`).
  The servers' state is the info key `kicad.ipc.servers` (`project|state|why`, one line each); Settings
  shows it under the KiCad IPC row, and a `failed` server is a notice with Retry (`kicad.retry`).
- `kipc.rs` over `nng.rs` and `proto.rs`: the transport, in process. Pure-Rust nng and protobuf (the schema
  from the runtime's descriptor set; without one only `Ping` and `GetVersion` work). One task per socket
  owns its connection; a fresh socket in the project's private directory counts as readiness, including
  KiCad's `api-<pid>.sock` fallback. There is no helper process.
- MCP: `kicad_ipc {path, type, message}` and `kicad_ipc_status {path}`.

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

- The IPC minimum is Backplane KiCad `10.99.0-bp.4` (API 2): `KH.min_version()` / `KH.min_api()` (`kserve.rs`). It is not the
  install pin (below: one per platform, newer). `KH.version.ge` orders
  the `-bp.N` suffix numerically, a hotfix's parts too (bp.2 < bp.3 < bp.4 < bp.10 < bp.12 < bp.12.1 < bp.13; `10.99.0-unknown-bp.3` reads as bp.3 a test build `10.99.0-bp.12-851cbefda6` as bp.12, a describe name `backplane-kicad-v10.99.0-bp.7-15-g8d903542-dirty-bp.13` as bp.13 (the last `bp.` part; the tag prefix is dropped); no suffix is bp.0), so
  installs older than the pin are "old" and the notice offers Update. Discovery checks the pinned
  `<data>/kicad/<KiCad.version()>` first, then other managed versions newest-first. Older runtimes offer Update
  and cannot shadow the pinned install.
- The server is always started through the runtime's `bin/kicad-cli` (`KS.launcher` turns `kicad-cli.real` back into
  the launcher), which defines `KICAD10_*_DIR`. `kicad_ipc_status` shows the start command line (`argv`) and `version`.
- Compatibility guards, in the hub (`KS.guard`): `UpdateItems` of a `FootprintInstance` without `definition` on bp.3 wipes the footprint's
  pads and 3D models, so it is refused unless the runtime lists `footprint-partial-update`; `CreateItems` of a schematic
  symbol with only a library id makes an empty symbol, so it is refused unless the runtime lists `symbol-from-libid`:
  `LoadAllLibraries`, wait for `GetLibraryStatuses`, then `PlaceSymbolFromLibrary`.

Test: `cd native/host && cargo test kicad::` (a stand-in server stands in for KiCad); `sh test/files/boat-kipc-job.sh` runs the transport probe too.

## Real runtime e2e

`test/files/kipc_hub_e2e.mjs` runs the whole path against a real Backplane KiCad: a hub on a free port and a temp
home, the demo `ecc83` copied from the runtime, a stand-in agent and only the hub's MCP tools
(`bun test/files/kipc_hub_e2e.mjs HUB RUNTIME PORT WORK`). It checks the answers, that no Bun or JS process is in the
hub's tree, and that the server ends with the hub (`--parent-pid`).

Health caches are checked against the current minimum even after an app update. Their file stamps cover the selected
runtime and candidates ahead of it; adding unrelated PATH entries after it does not invalidate healthy startup state.

The bp.4 runtime advertises both fix capabilities: partial footprint updates and symbol creation from a library ID
are accepted. Symbol libraries load on demand; agents need no blanket library preload on this runtime.

The installer checks the pinned asset against both the release SHA256SUMS and the SHA-256 compiled into the hub
(`install.rs`, `Package.sha256`).

KiCad can buffer its ready line until exit when stdout is a pipe. The hub discovers a fresh socket in the
project's private socket directory, including KiCad's `api-<pid>.sock` fallback, and the hub then requires Ping
and a matching version. Sockets present before launch never count as readiness for that launch.

When the pinned managed install is absent, discovery retains other installed versions as newest-first fallbacks.
An old install therefore reports old and offers Update; the pinned directory appearing invalidates the health cache
and takes precedence.

The Settings installer selects a package from the hub machine's OS and
architecture, including when the client is a phone or another computer. The pins are one line each in
`native/host/src/kicad/install.rs` (`LINUX`, `WINDOWS`, `MAC`; their doc comment says which release each is on and
why), and the tests read them. There is no managed Linux ARM64 or Intel macOS package, so those hubs offer a
runtime path instead. The install action uses the same catalog as the backend.

macOS (`darwin-arm64`): the fork's macOS job (`.github/workflows/backplane-macos.yml`, `scripts/backplane-macos.sh`)
builds every release tag and every batch on the release line. Its package holds `KiCad.app` with every dylib moved
inside and signed ad hoc, `bin/kicad-cli` and the GUI launchers `bin/kicad`, `bin/pcbnew`, `bin/eeschema`. It is not
notarized, and the hub's install does not need it to be: Gatekeeper checks only files that carry the quarantine
attribute, which browsers and other apps that opt in set on what they download. The hub downloads with `curl` and
unpacks with `tar`, neither of which sets it (nor does the hub's process: it is not sandboxed and no bundle of ours
sets `LSFileQuarantineEnabled`, which would quarantine everything the app writes; keep it that way), so the runtime
runs as installed (Apple silicon requires a signature,
and the ad-hoc one is enough). A runtime downloaded by hand from the releases page with a browser is quarantined:
`xattr -dr com.apple.quarantine <folder>` clears it, as the release notes say. That is expected, not a bug.
Notarization would need a Developer ID Application certificate and an Apple notarization login; neither is used.

macOS privacy prompts: the hub is the responsible process of every KiCad it starts, so a folder macOS guards (Documents,
Desktop, Downloads, a removable volume) that KiCad touches makes macOS ask for Backplane, and KiCad's UI thread waits on the
answer. KiCad's own default documents folder is `~/Documents/KiCad`, so on macOS the hub gives every child but a person's own
terminal `KICAD_DOCUMENTS_HOME=<home>/kicad/documents` (`proc::KICAD_DOCUMENTS`; never over a value the hub itself was started
with): the embedded and headless KiCad, `kicad-cli`, Open in KiCad, and agents' own `kicad-cli`. KiCad's New Project lands
there by default; a project the person keeps in `~/Documents` (or on the Desktop) still makes macOS ask once, legitimately.
For that case a start that stalls is not restarted: when KiCad's API keeps answering "not ready" for 5 s with no dialog of
its own open, the pane says "macOS is asking for permission for Backplane: answer the prompt to let KiCad start", and the
hub waits up to 10 minutes before failing with that reason (`embed::mac_prompt_wait`; never a second KiCad into the same
prompt, `start_retry` does not retry it). Found on the Mac mini gate (Nov 16): the first Edit stalled on the Documents
prompt for 30 s, then a second KiCad met the same prompt.

Downloads must match both the release checksum list and the compiled digest.
The installer checks the archive format and runtime layout in a staging
folder before moving it into the managed runtime directory. It refuses to
overwrite an existing version, preserving that directory on failure; an
existing separately installed compatible runtime can be selected by path.

The companion footprint/model library is embedded in the shared service build;
[library setup and qualification](kicad-library.md) describes its immutable pin,
managed profile and actual CLI/IPC/GUI process environment.
