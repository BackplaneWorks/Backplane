# Native Bend: what was measured, and how far a native hub gets

Stream `bend`. Measured 2026-11-04/05. Source commit of the kernels: `f7fe97238e`/`4bf31f377a` on branch `gpui` (base upstream `7c9b3c223c`).
Everything below ran on **Boat**, the same sandbox type the earlier 5.7-14.7x benchmark used: default size, 4 vCPU x86_64, 7.9 GiB, Ubuntu 24.04, clang 18.1.3, Bun 1.3.14, rustc 1.97.1, **Bend 2.0.36** (sha256-verified pin). The Mac was used only to install the pin and sync sources; no build or measurement ran there, and its folder is removed. Raw output: `build/gpui/results/bend/` (git-ignored); harness: `test/gpui/bend/`; kernels: `native/bend/`; host spike: `native/host/`.

## Summary

* Three hub kernels (broadcast pacing, sidebar status, history page policy) were compiled **unchanged from the real hub Bend source** (`src/core/thin-pace.bend`, `client.bend`, `thin.bend`, `thinq.bend`) to C and then machine code. Native, Bend-JS and plain TS produced **identical digests** on identical generated input in every run.
* Native Bend is **1.4-2.7x faster than the Bend JS emit** on these kernels (broadcast 2.3x, history 2.7x, status 1.4x against the hub-style JS build and 2.7x against the raw emit), with 3-38x less memory, and **slower than plain TypeScript on all three: 3.6x (broadcast), 5.5x (history policy), 45-95x (status)**. It did not use extra cores (1 thread = default).
* The native runtime did **not** show the bend#1007 slowdown: 2,000 iterations of two kernels and 1,000 iterations of a 20x bigger heap stay flat in time and RSS.
* **The hub cannot be emitted to native today, for two independent reasons.** (1) The C backend refuses any effect def without a `.c` twin, and 212 defs in `src/` import only `.js` (one `.c`: `mem`). (2) The whole-hub C emit does not fit 8 GiB, with the stock compiler or with the build coordinator's low-memory compiler.
* A **Rust host (2.7 MB, 10 MB RSS) with rusqlite and a WebSocket listener, calling the native Bend policy as a child process**, answered `/hello` **byte for byte identical to the real JS hub** and four `/history` pages and their WebSocket replies identical to a bun:sqlite+TS oracle. The boundary is cheap in bytes (24.6 KB in, 0.8 KB out) but the Bend compute behind it dominates, so the page path was **9x slower than Bun+TS** (3.70 vs 0.405 ms).
* Recommendation: do **not** pursue a whole native-Bend hub. See the end.

## 1. Kernels in machine code

Method: `native/bend/{pace,status,history}.bend` import the real modules and a deterministic generator (`kcommon.bend`; `test/gpui/bend/ts/*.ts` mirror it exactly). `boat.sh` emits C (`bend K.bend -o K.c`) and JS (`-o K.js`), builds the C with `clang -std=c11 -O2` and `-O3` (Bend's own default is `-O3`), then runs 3 fresh processes per variant. Digests are order-dependent hashes over every decision. The JS emit runs under Bun; "hub JS" additionally applies the hub's `bun build --minify` and `scripts/js-native.py` primitives. Timing is inside the process, around the kernel only (inputs are built first, as in the earlier harness).

| Kernel (input) | Variant | Median | p95 | Peak RSS | Binary / bundle | Digest |
|---|---|---:|---:|---:|---:|---|
| Broadcast, 1.45M events x 4 views (5 timed runs x 3 procs) | native -O2 | 3510 ms | 3725 ms | 406 MiB | 755 KB | same |
| | native -O3 | 3699 ms | 3951 ms | 406 MiB | 759 KB | same |
| | native -O3, `--threads 1` | 3475 ms | 3632 ms | 406 MiB | | same |
| | Bend JS (raw emit) | 8109 ms | 9227 ms | 1323 MiB | 59 KB | same |
| | plain TS | 970 ms | 1040 ms | 504 MiB | | same |
| Status, 1,000 threads (30 runs x 3 procs, 20 inner) | native -O2 | 11.00 ms | 12.25 ms | 4.5 MiB | 1.17 MB | same |
| | native -O3 | 10.47 ms | 12.15 ms | 4.5 MiB | 1.18 MB | same |
| | native -O3, 1 thread | 10.45 ms | 11.40 ms | 4.4 MiB | | same |
| | Bend JS (hub build) | 14.85 ms | 26.70 ms | 173 MiB | | same |
| | Bend JS (raw emit) | 27.98 ms | 35.60 ms | 186 MiB | 123 KB | same |
| | plain TS | 0.23 ms | 0.39 ms | 93 MiB | | same |
| History policy, 16 pages x 49 rows (pure part only) | native -O2 | 43.77 ms | 48.95 ms | 22.4 MiB | 866 KB | same |
| | native -O3 | 43.67 ms | 48.35 ms | 22.4 MiB | 878 KB | same |
| | native -O3, 1 thread | 43.60 ms | 45.60 ms | 22.2 MiB | | same |
| | Bend JS (hub build) | 117.90 ms | 149.50 ms | 192 MiB | | same |
| | Bend JS (raw emit) | 118.03 ms | 130.60 ms | 249 MiB | 92 KB | same |
| | plain TS | 7.96 ms | 9.57 ms | 139 MiB | | same |

Notes and limits:
* The status block above is the second of two runs; the first gave native -O3 12.35 ms, raw JS 30.02 ms, TS 0.13 ms (the TS number moved from 0.13 to 0.23 ms between runs, so the status ratio is 45-95x, not one figure). Broadcast was run once (3 processes x 5 runs; the hub-JS variant was added afterwards and is not measured for broadcast; the earlier report's hub-JS figure for the same shape is 7091 ms on its 1.45M-event real log, a different input).
* These numbers are **not** the earlier report's. Its fixtures were an anonymized copy of a real log and its history path ran SQL; mine are generated and the history kernel excludes SQL (the SQLite effect has no C twin). Compare ratios, not absolutes. Status here builds the state with generated threads/asks/tasks/settings; the TS side is the repo's `equivalents.ts`.
* `-O2` vs `-O3` is within noise. Multi-core: the runtime can fork, but these kernels are sequential folds, so `--threads 1` equals the default.
* Emit and compile cost: C emit 6.1 s / 2.0 GiB (broadcast), 7.3 s / 2.1 GiB (status), 1.0 s / 0.4 GiB (history); clang 1.4-3.9 s. Emit of `client.bend`'s closure dominates; all three kernels emit.
* First attempts failed on Bend constraints worth knowing: `Nat` is an immediate only below 2^48 (a hash multiply overflowed: "a Nat past the largest immediate 2^48-1"); defs must precede uses.

## 2. Memory behaviour (bendlang/bend#1007)

`native/bend/loop.bend` repeats a kernel in one process and prints per iteration the wall ms, RSS (`/proc/self/statm`) and the digest (one digest throughout every run).

| Run | Iterations | Median per window | RSS | Slope |
|---|---:|---|---|---|
| status, 1,000 threads, native -O3 | 2000 | 10.0 ms in all 10 windows of 200 | 23.2 MiB, flat | -0.14 ms / 1000 it, +0.00 MiB / 1000 it |
| history, 16 pages, native -O3 | 2000 | 39-40 ms in all windows | 24.0 MiB, flat | -0.15 ms, +0.00 MiB |
| status, **20,000 threads**, native -O3 | 1000 | 404-422 ms | 60.7 MiB, flat | -1.73 ms (first-100 408.5 -> last-100 416.0 ms) |
| same status, Bun (bend JS) | 2000 | 23-27 ms, no drift | ~180 MiB, +2.2 MiB / 1000 it | |
| same history, Bun | 2000 | ~104 ms, no drift | ~185 MiB, +1.9 MiB / 1000 it | |

No slowdown and no growth were observed. This does not refute #1007: that report was a ~0.2 s board parse in a long-lived process with a larger, differently shaped heap; these workloads are persistent-trie builds and string blocks. Time per iteration grew 39x for 20x the threads (10.5 -> 409 ms), i.e. mildly super-linear; TS at 20,000 threads was not measured.

## 3. What a native hub needs

**Emit.** Whole-hub C emit (`src/server/main.bend`) on Boat:

| Compiler | Result |
|---|---|
| stock Bend 2.0.36 | SIGKILL (OOM) after 51 s at 7.49 GiB peak RSS; no output. The sandbox had 2 GiB swap (COMMON asks for 0), so the cap was not softer than stated. |
| build coordinator's low-memory composite compiler (41262875, flags from `bp-coord/pr944-capped-recipe`), C target | Load and check of the whole hub completed at 441 s with RSS < 1 GiB. The **C emit stage** then grew from 0.9 to ~6.7 GiB RSS (JS heap 4.8 GiB) and was SIGKILLed at 753 s, peak 7.45 GiB. The same recipe emits **JS** at ~3.4 GiB RSS (project note, not re-measured here), so the C emitter is not covered by its settle/spill tuning. |
| any compiler | `Error: no .c import: ../../src/server/db.Db.open`: tested on a 6-line program. This stops independently of memory. |

Raising the cap was not tried (not permitted). So: the hub does not emit to C under 8 GiB; whether it would at 16 GiB is **unmeasured**, and it would fail at the effect check anyway.

**Foreign code.** 212 defs in `src/` import `.js` only (185 in `src/server`), 1 imports `.c` (`mem`). Before commits `e1ce7b4fb3`, `65267a92b4` and `42dc36d174` the hub had C twins for 12 effects (host 1646 lines, perf 530, pty 319, adopt 312, mailstore 234, hublock 185, evlog 122, locks 115, cast 79, telemetry 80, google 38, loop 16); they are gone. The other 25 JS effects never had one.

Effects (`src/server/effects/*.js`, 10,707 JS lines, 37 files; counts generated from the source; difficulty is my judgement):

| Effect | JS lines | Bend defs | Host APIs used | C twin existed | Difficulty | Native plan |
|---|---:|---:|---|---|---|---|
| `host.js` | 2383 | 53 | bun:ffi, crypto, fs, node:child_process, os, path | yes (1646 C lines then) | hard | Rust: HTTP/WebSocket server, auth, process spawn, CBOR frames, file serving. The largest effect; has no small seam. |
| `loop.js` | 1376 | 2 | bun:ffi, crypto, fs, os | yes (16 C lines then) | hard | Not ported: Bend native has its own event loop (IO.fork/Chan); the JS loop is the Bun-specific scheduler/timer glue. Needs a design, not a port. |
| `pty.js` | 786 | 2 | bun:ffi, fs | yes (319 C lines then) | hard | Rust: forkpty/ConPTY (portable-pty). Windows is the hard part. |
| `zip.js` | 593 | 4 | bun:ffi, fs, os, path, zlib | no | medium | Rust zip crate; streaming and path-safety rules. |
| `vault.js` | 542 | 8 | node:child_process, node:crypto, node:fs, node:os, node:path | no | hard | Rust keyring crates (Secret Service, Keychain, DPAPI) + AES-GCM; preserve redaction rules. |
| `accounts.js` | 530 | 5 | node:child_process, node:crypto, node:fs, node:path | no | hard | Rust: child-process CLIs, OAuth state, files; many provider quirks. |
| `diagnostics.js` | 460 | 7 | fs, path | no | medium | Rust fs/JSON. |
| `kicad_library.js` | 312 | 2 | bun:ffi, node:crypto, node:fs, node:os, node:path, node:zlib | no | medium | Rust zlib + FFI/file work. |
| `db.js` | 303 | 13 | bun:sqlite | no | easy | Rust rusqlite (bundled, FTS5). Spiked: reads work. |
| `perf.js` | 298 | 12 | bun:jsc, fs | yes (530 C lines then) | easy | C twin existed; counters, trace file. |
| `storeio.js` | 276 | 8 | bun:sqlite, crypto, fs, path | no | easy | Rust rusqlite + fs. |
| `adopt.js` | 260 | 12 | bun:ffi, fs | yes (312 C lines then) | easy | C twin existed; libc. |
| `pairing.js` | 258 | 11 | node:crypto, node:fs, node:os, node:path | no | easy | Rust crypto + fs. |
| `hublock.js` | 234 | 4 | bun:ffi, node:fs, node:path | yes (185 C lines then) | easy | C twin existed; flock. |
| `locks.js` | 229 | 3 | bun:ffi, node:fs, node:os, node:path | yes (115 C lines then) | easy | C twin existed; flock + files. |
| `render_asset.js` | 210 | 5 | node:crypto, node:fs, node:os, node:path | no | easy | Rust fs + hash. |
| `project-home.js` | 173 | 1 | node:child_process, node:crypto, node:fs, node:fs/promises, node:path | no | easy | Rust fs + child process. |
| `cad-install.js` | 166 | 3 | node:child_process, node:crypto, node:fs, node:path | no | easy | Rust child process + fs + hash. |
| `mailstore.js` | 147 | 5 | bun:ffi, fs | yes (234 C lines then) | easy | C twin existed; rusqlite/fs. |
| `live-checkpoint.js` | 142 | 6 | node:crypto, node:fs, node:path | no | easy | Rust std / small crate. |
| `message_image.js` | 120 | 3 | bun:sqlite, node:crypto, node:fs, node:os, node:path | no | easy | rusqlite + image header parsing + fs checks. |
| `liveness.js` | 119 | 1 | node:crypto, node:fs, node:path | no | easy | Rust std / small crate. |
| `bounded_cache.js` | 106 | 5 | - | no | easy | Rust std / small crate. |
| `components_cache.js` | 89 | 5 | node:crypto, node:fs, node:os, node:path | no | easy | Rust std / small crate. |
| `push-gate.js` | 86 | 2 | node:crypto | no | easy | Rust std / small crate. |
| `evlog.js` | 86 | 1 | fs | yes (122 C lines then) | easy | Rust std / small crate. |
| `hotboot.js` | 67 | 3 | fs | no | easy | Rust std / small crate. |
| `storefar.js` | 55 | 2 | fs | no | easy | Rust std / small crate. |
| `plot_board.js` | 50 | 3 | node:fs | no | easy | Rust std / small crate. |
| `wire_log.js` | 46 | 1 | bun:sqlite, fs, path | no | easy | rusqlite + fs. |
| `telemetry.js` | 44 | 3 | node:crypto | yes (80 C lines then) | easy | Rust std / small crate. |
| `cast.js` | 44 | 4 | - | yes (79 C lines then) | easy | Rust std / small crate. |
| `farcodec.js` | 35 | 2 | node:zlib | no | easy | Rust std / small crate. |
| `omp-launch.js` | 31 | 2 | node:fs, node:path | no | easy | Rust std / small crate. |
| `restart-hold.js` | 19 | 2 | - | no | easy | Rust std / small crate. |
| `mem.js` | 18 | 1 | - | no | easy | Rust std / small crate. |
| `google.js` | 14 | 2 | - | yes (38 C lines then) | easy | Rust std / small crate. |

Plan: effects that wrap SQLite, files, hashing and child processes are routine Rust. The hard ones are `host` (the HTTP/WebSocket server and about a fifth of all effect lines), `loop` (an event-loop/timer layer that a native Bend runtime replaces rather than ports; I did not analyse it in depth), `pty` on Windows, `vault` (three OS keystores) and `accounts`.

## 4. Rust effects host spike

`native/host` (Rust, rusqlite bundled, tungstenite) + `native/bend/policy.bend` (native Bend child process). Rust owns the listener and SQLite; Bend owns the policy: `/hello` is the **real** `Hello.answer.c` from `src/core/tailnet.bend`; `/history` runs the **real** `Q.Q.rows`, `T.Index.window`, `T.Index.page`, `T.Blocks` on the rows Rust read with the hub's own page SQL. One request file per call; the child writes its answer to stdout.

Fixture: a store created by the real hub (schema from its `store.db`), 4 threads x 200 entries, text containing newlines, tabs and `é`. Compared on Boat in one job (`test/gpui/bend/boat-m4.sh`):

| Check | Result |
|---|---|
| `GET /hello`, headers + CBOR body vs the real JS hub binary (0.14.0, same commit) | **identical, 188 bytes** |
| `GET /history?thread=t0..t3` body vs bun:sqlite + the repo's TS equivalents | **identical** (1006/824/1030/829 bytes) |
| WebSocket `history tN` replies vs the same oracle | **identical** |

What this is not: the JS hub's own page reply goes through its authenticated WebSocket view protocol and was **not** reproduced; the page oracle is TS over the same SQLite file, not the hub. No auth, CBOR framing, deltas or writes.

Cost (sequential, new connection per request, 1,000 requests; 8-way concurrent in brackets):

| Path | Median | p95 | Requests/s | Process peak RSS |
|---|---:|---:|---:|---:|
| `/hello` real JS hub | 0.135 ms (0.499) | 0.281 ms | 5,789 (12,246) | 133 MB |
| `/hello` Rust + Bend child | 0.490 ms (1.422) | 0.652 ms | 1,971 (5,334) | 10 MB |
| `/history` Bun + bun:sqlite + TS policy | 0.405 ms (3.189) | 0.566 ms | 2,309 (2,551) | 90 MB |
| `/history` Rust + rusqlite + Bend child | 3.695 ms (7.981) | 4.984 ms | 259 (974) | 10 MB |

Binaries: host 2.7 MB, policy 0.92 MB, against the 105.9 MB `backplane-serve`.

Boundary breakdown for `/history` (the host's own counters, averaged over all 2,108 calls including the 8-way run): SQL 0.82 ms; SQL plus row serialization 1.19 ms; policy child 4.11 ms; final formatting 0.012 ms. The child's fixed cost is about 0.7 ms (`/hello`: 0.73 ms to spawn, run and read). Payload crossing: **24.6 KB in, 0.8 KB out** per page; only rows cross, not states. So the child's compute (about 3.4 ms, matching the 2.7 ms/page of section 1) dominates, not the boundary. Had whole states crossed, the cost would be larger; they did not. **Even a free boundary leaves native Bend slower than the TS path (3.4 vs 0.4 ms).** A persistent child or C ABI would remove the ~0.7 ms spawn, not the compute. Not measured: a persistent child (Bend's stdin read proved unreliable for a stream, so requests go through a file); a C ABI.

## 5. What this predicts for a whole hub

Only what the numbers support:

* Replacing the Bend **JS** build by native Bend would make these three kernels 1.4-2.7x faster and cut their memory 3-38x. It would not reach plain TS. Where the hub's hot work is "Bend on persistent tries", native Bend sits between Bend-JS and TS, closer to Bend-JS.
* The fraction of a real hub's CPU time spent in such kernels is **unmeasured** here (the earlier report also did not measure it), so no whole-hub speedup is predicted. Amdahl's bound from the earlier report applies unchanged: kernels at 10% of CPU cap any gain near 1.1x.
* The part that is clearly large is process size: a Rust host with SQLite and a listener is 10 MB RSS and 2.7 MB on disk for the paths spiked, against 133 MB / 106 MB for the JS hub. That is the cost of the paths implemented (two routes and one WebSocket echo), not of a complete hub.

## 6. Remaining work (engineering judgement, one developer, not measured)

| Work | Effort |
|---|---|
| Make C emit possible: a `.c` (or Rust-linked) implementation for each of 212 foreign defs across 37 effects | 3-5 months for all; this is the same work as the Rust host below, in C |
| Or: Rust host replacing the 37 effects with Bend policy as child/FFI behind a stable ABI; protocol, store and event-log differential tests; agents, ptys, vault, accounts, zip, pairing | 3-4+ months (matches `docs/gpui/PLAN.md`: 2-4+ months) |
| Compiler memory: whole-hub C emit under 8 GiB needs compiler work (the C emitter is not covered by the low-memory settle tuning) | unknown; a compiler project |
| Persistent-child or C-ABI policy boundary, cancellation, crash isolation, measured | 1-2 weeks |
| Move the hot read-model kernels (status index, broadcast pacing, history blocks) out of Bend into the host language | days per kernel; this is where the measured gain is |

## 7. Recommendation

Do not build a whole native-Bend hub. On the three real hub kernels native Bend beats Bend-JS by 1.4-2.7x but loses to plain TS by 3.6-95x; the compiler cannot emit the hub under 8 GiB and refuses it outright until every effect has a C twin. Keep Bend for the small pure policies where laws pay (routing decisions, protocol, geometry), run them natively as a child or later through a C ABI **off the hot path**, and put the mutable indexes and IO in Rust (or keep them in TS) where the measured kernels are 4-100x cheaper. If a native hub is wanted for size and memory, the Rust host route is the one that has shown a working slice (byte-identical `/hello`, 10 MB RSS); budget it as a rewrite of the 37 effects, not as a recompile. Re-measure the unmeasured items first if the decision turns on them: the share of hub CPU in these kernels, TS at 20,000 threads, a persistent policy child, and a Bend representation (maps, strings) tuned for native.

## Reproduce

`test/gpui/bend/boat.sh KERNEL N REPS INNER` (pace 1450000 5 1; status 1000 30 20; history 0 30 20), `boat-loop-job.sh`, `boat-loopbig-job.sh`, `boat-hub-job.sh`, `boat-hub2-job.sh` (needs the coordinator's `lowmem/` files, not committed), `boat-efx-job.sh`, `boat-m4-job.sh` (needs a copy of `backplane-serve` at `test/gpui/bend/m4/jshub`, not committed). Summaries: `summarize.py`, `loop-summary.py`.
