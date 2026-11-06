# Backplane GPUI experiment

Research date: 2026-11-04. Baseline: upstream `main` at `7c9b3c223c`.
Worktree: `/home/h/Documents/i2cjak/backplane-gpui`, branch `gpui`.
The shared checkout's local `main` was stale/divergent (`c2ca72cb6e`); only
the new, clean experimental branch was advanced to fetched upstream main.
Neither local main nor the running installation was changed.

## Decision

**Native Bend is feasible as a language target today, but the current
Backplane hub cannot simply be rebuilt as a native executable.** The pinned
Bend **2.0.36** CLI already emits C with `bend input.bend -o output.c`, and
builds a machine-code executable through Clang when the output has no source
extension. Its compiler implementation being TypeScript is unrelated to the
language of the programs it compiles. `bend2/main.ts` is not a JS-only backend.

Backplane deliberately removed its C hub build in commit `3de248dd5b`.
Current `src/server/effects/` contains roughly 10,700 lines of JS, with just a
small C memory effect remaining. `docs/js-service.md` accurately describes
the *application's* JS-only support, not the compiler's target support.
Several newer capabilities have no C twin at all. Reviving old effects
alone would lose current behavior.

Start with a **real GPUI client talking to a separately launched existing
Bend/JS hub**. This measures the UI change independently. In parallel in the
experiment's design, establish small native Bend policy executables and
their differential tests, then a native effects host. Do not call the first
prototype a native hub. The desired endpoint keeps Bend policy and laws;
Rust handles IO and mutable indexes, and GPUI renders the UI. In-process
integration is a later optimization behind the same protocol contracts.

## Bend to native options

Effort estimates are engineering estimates for one experienced developer,
not measured delivery commitments. Speedups below are hypotheses until the
same input and paired output checks run on native code.

| Route | Feasibility and effort | Expected performance / size |
|---|---|---|
| Bend 2 C emitter + Clang | Exists in pinned 2.0.36. A small pure-policy executable is hours to days. Restoring a minimal current native hub is several weeks; complete effects/behavior parity likely 2–4+ months. Old native function-ID limits and heap/runtime behavior need rechecking. | Removes Bun/JSC from the native process. Likely much smaller runtime closure, but persistent trees, lists, copying and allocation remain. No defensible whole-hub multiplier yet. |
| C emitted by Bend + Rust effects host / C ABI | Recommended eventual route. Keep the runtime opaque on its own thread or worker process; expose small explicit input/output values, not Bend heap objects across Rust. Prototype a boundary in days; effects parity is the work above. | Native mutable indexes and IO can target the existing TS kernel timings; boundary costs can erase gains if whole states cross it. Initially a subprocess avoids an undocumented runtime ABI and isolates failures. |
| New direct LLVM or Rust emitter | No supported direct target found in the pinned CLI. LLVM machine code is available indirectly through C/Clang. A new emitter/runtime/foreign-effect ABI is a compiler project: months, plus maintenance. | No evidence it beats the existing C route. Writing Rust output does not by itself fix Bend representations. |
| Historical Bend 1 / HVM | HVM runtimes and older Bend generations have native C/Rust/GPU execution, but they are not a drop-in target for Bend 2's dependent types, effects and current Base ABI. A translator or language downgrade would be a major port. | Parallel reductions are not a measured advantage for latency-sensitive SQLite/socket work; no transferable speedup claim. |
| JS AOT compiler, e.g. Static Hermes / restricted JS-to-C | Investigate only for a small isolated pure kernel. The generated hub relies on dynamic objects, closures, runtime FFI and Bun APIs. No verified compatible whole-hub AOT path. Weeks for an experiment; a full runtime/effects port is open ended. | An interpreter embedded in a native binary still does not meet the native-code requirement. AOT support must be demonstrated on actual generated functions, with no JS fallback. |
| Bun `--compile`, Node SEA, embedded QuickJS/V8 | Available packaging/hosting choices, **not native compilation of Bend application code**. Bun may JIT at runtime. | Bun currently contributes roughly 104 MB of the service's 115 MB executable per `docs/js-service.md`; packaging alone cannot deliver the requested architectural result. |

### What the existing benchmark establishes

Retained source: [PR #895 report](../../build/bend-vs-ts/REPORT.md),
[raw results](../../build/bend-vs-ts/RESULTS.json), and
[harness](../../test/perf/bend-vs-ts/README.md).
These are JS kernels on Boat, not native Bend, nor full-application metrics.

| Actual path | Bend JS | Plain TS | Gap |
|---|---:|---:|---:|
| Status, 1,000 threads | 4.922 ms | 0.335 ms | 14.7× |
| Broadcast, 1.45M events × 4 views | 7091.440 ms | 798.076 ms | 8.9× |
| Indexed history + renderer blocks | 2.953 ms/query | 0.519 ms/query | 5.7× |

Same-trie status control: Bend 0.866 ms, TS 0.936 ms; native JS Maps
0.087 ms. This points strongly to representation/library work, not a
universal code-generator penalty. Native compilation can retain that work.
For a kernel actually moved to an efficient native index, use the TS result
as a comparison target, never as an automatic native Bend prediction.

Amdahl example: if these kernels were 50% of all CPU time, accelerating
them 5.7–14.7× would yield only 1.70–1.87× overall. At 10% of CPU time the
gain is only 1.09–1.10×. IO waits, agent CPU and GPU memory are separate.
Measure the fraction; do not infer it from microbenchmarks.

The report's whole-hub JS emit took 120.236 s and **14.45 GiB peak RSS**.
That exceeds this experiment's 8 GiB cap. It is not permission to raise the
cap. Reuse validated artifacts for the UI prototype; attempts at the native
compiler must stay small/incremental or stop at the cap. Comparing that
whole-hub compile with 0.203 s for three TS equivalents is not a fair app
build comparison. Existing full proof failures (`MailProof.keeps` and
`stream_step_proof.bend`) are baseline findings, not permission to weaken laws.

### Native effects plan

Keep persisted events and RPC meanings backward compatible. Build a
small ABI with owned byte buffers, integer handles, explicit cancellation
and errors. Never share Rust references into a moving Bend heap. Keep each
foreign operation's result decoding and policy in Bend where practical.

| Existing effects / responsibilities | Native equivalent and acceptance |
|---|---|
| `db`, `evlog`, `storeio`, `mailstore`, history | Rust `rusqlite` with FTS5, bound parameters, WAL, exact event sequence/transactions, history IDs/bodies/blocks and recovery replay checked against JS. Preserve int64 and NULL behavior. |
| `host`, `loop`, pairing, `wire_log`, `farcodec`, sockets | Tokio + HTTP/WebSocket implementation; preserve auth, CBOR dictionaries, snapshots/deltas, backpressure, reconnect/generation and cancellation semantics. Native policy chooses access and routing. |
| files, hotboot, caches, project-home, render assets | Rust filesystem APIs and bounded caches; atomic writes, path validation, watcher generations and no-follow behavior tested. |
| processes, accounts, agent launch, `pty`, locks, liveness | Rust process groups/PTY support, captured PID lifecycle, hub/store locks, deterministic agent fixtures and interrupt/restart checks. |
| vault, pairing, push, Google, telemetry, zip, CAD helpers | Native crypto/keyring and HTTP libraries, preserve redaction, permissions and opt-outs; continue helpers over explicit subprocess protocols until replaced. Do not silently omit these from a parity claim. |

Do not port all 10,700 lines verbatim first. Separate native policy kernels
from effects, port one vertical slice (open home → sidebar → history → post),
then add agents and recovery. Full native acceptance requires protocol and
store differential tests, sustained 20-agent load and all effects accounted
for. Small standalone native kernels alone are not a native backend.

## GPUI architecture and visual parity

Pin Zed's published `gpui` crate and lock all dependencies. Use GPU-native
widgets, no browser for the sidebar/chat/composer. GPUI entities receive
immutable hub projections; a worker owns the connection. RPC replies and
subscriptions wake entities only when data changes. No continuous redraw.
The hub remains authoritative for ordering, phase, permissions, message IDs,
agent execution and persisted state. Selection and text editing stay local.
Use view/subscribe/history RPCs where possible; avoid replaying the entire
event log into a second client model. Keep request IDs and target generations
so a late response cannot overwrite the selected thread.

First prototype: sidebar project/thread rows, selected thread/messages,
composer with actual post RPC, disconnect/errors, and local command popup.
Document any unsupported formatting, attachments, IME or paging explicitly.
Do not present placeholder viewer buttons as implemented viewers.

| Current screen / code | GPUI mapping |
|---|---|
| Sidebar (`src/web/style.css`, `view.bend`, hub projections) | Fixed 264 px sidebar; virtualized project/thread rows, phase dots, selected accent, search, bottom settings/machine controls. |
| Thread / message history | Header and tools; virtualized, keyed message blocks with markdown, streaming row, subagent and todo docks; preserve scroll anchor per thread. |
| Composer / popups | Native text entity with selection, clipboard and IME; send/stop row; anchored skill/reference/slash/model menus. Match dimensions and keyboard behavior. |
| Project home / intent / order card | Flexible card/grid entities fed by hub card projection; existing goal/parts and status. |
| Settings, bots, rooms, Status | Forms, lists and tabs with shared hub decisions, same theme and typography. Later parity scope, not included in the first chat slice. |
| Viewer dock | Resizable pane and same tab order: Board, Schematic, 3D, Mech, Files, Diff, Browser, Review; preserve per-thread source/camera. |
| Files, Diff, Review | Native lists, text/markdown and review forms, authenticated resources fetched through hub. |

Extract exact dark/light theme colors from `src/web/style.css`, not a
similar palette: bg `#0d0f12`, panel `#13161b`, raise `#1a1e25`, line
`#262b34`, text `#d9dde4`, dim `#8a93a3`, accent `#5fb3ff`.
Use the same 13 px base, square corners, spacing, phase colors and light
theme values. Existing Backplane appearance takes precedence over a new
visual style. Validate screenshots at 1400×900 and narrower desktop widths;
the existing responsive web UI supplies the mobile layout.

### KiCad / 3D viewers

The hub already serves board/schematic plot chunks, info, model assets and
watch generations. Keep KiCad IPC and parsing in the hub/helper, never
inside a paint callback. Native 2D viewer can consume the same chunks into
GPUI canvas/path primitives, keeping hit testing, layers, cross-probing and
selection. Use large-scene spatial culling and benchmark real boards.

3D can render cached GLB/STEP-derived meshes with wgpu into a texture, then
present through a supported GPUI image/surface path. **GPUI 0.2.2 on Linux
uses Blade, not a shared wgpu renderer**, so zero-copy sharing cannot be
assumed; prove GPU interop or account for readback/copy. A native custom
renderer using the same graphics backend is another option.

Fastest compatibility option: existing web viewer in a separate companion
window, or a platform webview in the dock after focus/compositing is proven.
Embedding CEF/Chromium retains a substantial browser footprint; measure
that separately. Browser automation still needs its browser helper even if
the main UI uses GPUI. Do not count removing that helper from the test as a
GPUI memory win. Prototype chat does not establish viewer feature parity.

## Measurement design

Compare: (A) current Electron + Bend/JS hub, (B) GPUI + **same** Bend/JS
hub, (C) GPUI + future native hub. Also web/Safari + each hub for phone.
Record binaries/commits, GPU/driver, OS, viewport, store fixture hash,
agent fixture version, power mode and warm/cold filesystem caches.

Use fresh synthetic homes initially, then an explicitly sanitized copy of
the real store for scale. Never point any experimental process at the live
home. The same fixture and deterministic agent executable must be used by
both clients. A minimal fixture is a smoke measurement, not a large-store
performance conclusion. No API/model calls in load tests.

| Metric | Method |
|---|---|
| Startup to usable | Monotonic process-start timestamp → first displayed sidebar + selected history + editable composer, not just window creation or `/hello`. 10 cold and 20 warm samples, median/p95. Report hub startup separately. |
| Memory | Sum RSS of client process tree and hub separately and together; also PSS/private memory where available, GPU allocation separately. Idle after 30 s and steady 20-agent load. Avoid shared-page double-count conclusions from RSS. |
| CPU | Delta process-tree user+system CPU / monotonic wall over equal idle and load intervals; report in core-percent (100% = one core). |
| Input | Sidebar click, thread switch, popup open and typing. Timestamp injected event → next frame that shows expected state; median/p95 over at least 30 events, idle and load. Processing time is labeled separately from input-to-pixels. |
| Size | Stripped binary bytes, compressed download, installed runtime closure including assets/libraries/helpers. Do not compare GPUI client alone with Electron + hub. |
| Build | Boat, clean dependency/cold build and warm incremental build separately, elapsed time, peak RSS, cap, compiler and cache state. Artifact retrieval excluded. |

20-agent validity: all 20 stand-ins must actually stream decoded deltas,
each at least 1/s, longest gap ≤2 s; equal duration, rates and conversation
payloads. Record hub and client metrics independently. A disconnected or
unrendered window cannot produce a valid UI benchmark. Extend existing
`test/tools/agents_load.ts` / `perf_standin` fixtures instead of simulating
20 static rows. Stop only captured process IDs and remove only test homes.
Report unmeasured metrics as unmeasured; never infer latency from FPS.

## Isolation, build and phone access

Heavy compiles run where the section below says. On Boat:
**8 GiB max for big jobs, 3.7 GiB otherwise, swap max 0**. Compile serially (`CARGO_BUILD_JOBS=1`), strip debug info,
avoid LTO initially. Verify the cgroup limits before compiling and stop if
they cannot be applied without privileged approval. No local fallback.
Use the existing budget/admission/cleanup-aware Boat runner; artifacts and
provenance return to ignored `build/gpui/`. Do not modify the installed app.

Experimental home: `~/.backplane-gpui` (fresh). Experimental hub: port
**3799**, never 3787 or the existing dev port 3788. Runtime launch must
refuse a live home/path alias and occupied experimental port. Set
`BACKPLANE_NO_UPDATE=1`, disable peer discovery/order traffic for test runs,
use isolated locks and keep separate logs. Phone listener binds only the
Tailscale address `100.64.84.41` (plus loopback when needed).

**Updated user decision: native apps on the other platforms.** Desktop uses
GPUI; iPhone/iPad uses SwiftUI; Android uses Jetpack Compose. Reuse the
existing native screens in `mobile/ios` and `mobile/android`, not the
WebView/WKWebView shell that currently defaults on in production. Keep the
same hub view protocol and Bend decisions. Their existing Bend/JS bridge
is a transport/policy dependency, not a native-code backend claim; replacing
it with native Bend remains a separate measured step.

Give experimental builds a distinct app identity, name and private storage,
so installing one does not overwrite production Backplane or pair to its
store automatically. Default to native screens at build time, without
requiring the person to enable the legacy/classic toggle. Pair only to the
experimental endpoint `http://desktop.tail9e0cf2.ts.net:3799/`, using its own
invitation. Preserve native viewer and composer code and tokens. The web
endpoint may remain available for comparison, but it is not the phone
experience delivered by this experiment.

**Updated user decision: where work runs.** Linux is the desktop target: the
GPUI client is built on Boat and tested on this machine, on an isolated
display, with the Electron comparison run here too. `hs-mac-mini` is for iOS
only, because it has no graphical login session and the person chose not to
pursue one. Android builds run on Boat or on the desktop, in a capped scope
there; Bend builds run on Boat; nothing else compiles on the desktop. On the
Mac, heavy jobs take the named lock `mac-heavy`, each stream keeps its own
directory and simulator, work stops if free space falls under 20 GiB, and
`xcode-select` stays as it is (use `DEVELOPER_DIR`). Do not publish or replace
the production TestFlight app implicitly.

**Updated user decision: native apps only.** There is no web page in this
experiment. The web endpoint and the web renderer are dropped; see `PORT.md`.

## Gates and deliverables

1. Commit this plan and intent on `gpui`; publish the feasibility finding.
2. Build the minimal Linux client on Boat. Run it on this machine against the
   fresh isolated hub. Verify real sidebar/history and composer round trip.
3. Bring up native phone variants; verify isolated identity, native screen selection, pairing and tailnet reachability.
4. Collect paired baseline/prototype measurements, retain raw samples,
   screenshots and method in `docs/gpui/RESULTS.md`; distinguish smoke,
   full-load, missing metrics and unsupported UI.
5. Compile a small actual Bend policy to machine code and compare outputs;
   report separately from a native whole hub. Advance the native effects
   port only on evidence it improves the product under the build cap.
6. Commit and push regular commits to `gpui`. Any PR targets `gpui` only;
   no main merge, install-over-live, release or auto-deployment.

## Primary sources

- [Pinned Bend CLI / target dispatch](https://github.com/bendlang/bend/blob/v2.0.36/bend2/main.ts)
- [Bend compiler](https://github.com/bendlang/bend/blob/v2.0.36/bend2/comp.ts)
- [Historical HVM2](https://github.com/HigherOrderCO/HVM2)
- [Zed GPUI documentation](https://www.gpui.rs/), [pinned API](https://docs.rs/gpui/0.2.2/gpui/)
- [GPUI 0.2.2 source](https://docs.rs/crate/gpui/0.2.2/source/)
- Repository `docs/js-service.md`, `docs/ci.md`, `src/server/effects/`,
  `src/web/style.css`, `src/web/host.js`, and PR #895's retained data above.

The collaborative browser was attempted for upstream source inspection but
reported no attached desktop guest; primary source HTTP/API reads were used.
