# GPUI experiment: perf evidence (stream `perf`)

Machine for hub numbers: desktop, 32 cores, 59 GiB, Linux 7.0. Hub: `build/gpui/backplane-serve` 0.14.0 (commit 96a230931d tree), fresh temp home per run, `--no-tailscale --foreground`, in `systemd-run --user --scope -p MemoryMax=4G -p MemorySwapMax=0`. Stand-ins: `test/tools/perf_standin.ts` (FAKE_PER=2 deltas per 100 ms tick, cycle 20). No model calls. Raw: `build/gpui/results/perf/`. Harness: `test/gpui/perf/`.

## M1 Hub under load (one WebSocket client, 10 s warm-up, 60 s measured, 1 run per count)

| agents | valid | min deltas/s/stream | longest gap ms | hub RSS MB start / after warm / end / peak | hub CPU (core %) | ping rtt ms p50/p95/max | delta lag ms p50/p95/max | deltas/s total | client KB/s | events.jsonl KB |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | True | 4.9 | 357 | 299 / 275 / 297 / 407 | 4.2 | 1.03 / 1.7 / 153 | 174 / 201 / 315 | 5 | 1.7 | |
| 10 | True | 4.9 | 404 | 315 / 344 / 221 / 690 | 12.6 | 0.97 / 36.3 / 89 | 177 / 202 / 368 | 53 | 15.7 | |
| 20 | True | 5.3 | 462 | 296 / 395 / 256 / 656 | 20.4 | 0.97 / 73.4 / 93 | 165 / 326 / 469 | 107 | 31.3 | |
| 50 | True | 5.2 | 586 | 312 / 253 / 563 / 638 | 35.5 | 26.42 / 65.6 / 217 | 190 / 379 / 597 | 267 | 79.5 | |

Validity (plan rule: each stream >=1 delta/s, longest gap <=2 s): met at 1, 10, 20, 50. Note the hub merges and paces deltas, so each stream arrives at about 5/s (not the 20/s emitted); the first harness run flagged 20 and 50 invalid only because it counted the project's auto-created thread; fixed in `m1_load.mjs`, verdict recomputed from `perThread`.

Observations (n=1 sample per count, so treat as indicative):
- Idle hub is ~290-310 MB RSS right after start, before any agent. That baseline is the Bend/JS runtime, not load.
- CPU scales roughly linearly: 3.6 / 11 / 18 / 35 % of one core for 1/10/20/50 agents (the hub is single-threaded JS; 50 agents is a third of one core).
- Ping rtt (event-driven client, corrected): p50 ~1 ms at 1-20 agents, p95 1.7 / 36 / 73 / 66 ms at 1 / 10 / 20 / 50 agents, p50 26 ms at 50 agents. The hub's single thread is what the ping waits behind.
- Delta lag (stand-in stamp to client): ~170 ms p50 at 1-20 agents (the hub's Delta.pace batching, not load), 267 ms p50 / 741 ms max at 50.
- RSS peak is 2x the idle level (650-690 MB) and RSS moves up and down by hundreds of MB inside a run: GC sawtooth, not a conclusion about leaks. M4 soak decides.
- events.jsonl grows ~30 KB per agent per minute of streaming; the WAL stays at ~4.3 MB (checkpointed).

M1 rerun with the valid-streams fix, then rerun again with the event-driven client (the table above is that last rerun, 2026-11-04 evening; raw `m1-<N>agents/result.json`; the earlier results are kept in `superseded-20ms-poll/`). One run per count, concurrent with nothing else.

### CORRECTION: the "20 ms WebSocket floor" was my harness, not the hub
`lib.mjs` `client().until()` waited with `await sleep(20)` in a loop and `rpc()` used it, so every RPC it timed read about 20 ms (plus up to 20 ms of quantisation). Found by the `files` stream, whose event-driven client measures `files.dir` at 4.26 ms (JS hub). `rpc()` now resolves in the WebSocket's onmessage. What this changes:
- WRONG, now fixed: M1 ping rtt (p50 20.5 ms was ~1.0 ms; the p95/max shape under load is real), `floor.mjs` ("20.3-20.5 ms regardless of gap"), the M2 `load.ping` rows and the stuck-reader ping series, M3 write-ack latency, the soak's `rtt` column. Do not quote the old "flat 20 ms" or "47 writes/s".
- NOT affected: view round trips (`viewClient` resolves in onmessage; they read 3-10 ms), delta lag (computed in the message handler; ~165-190 ms p50 matches `Delta.pace()` = 200 ms batching, src/core/hub.bend:1519), HTTP timings, byte counts, startup/boot times, RSS/CPU, the soak's slopes.
- Event-driven re-measure on an empty-ish log (1 project, 1 thread, `rpc-eventdriven.json`): JS hub `load.ping` p50 0.22 ms / p99 1.13 ms (500 pings); `thread.rename` ack p50 11.5 ms / p95 15.9 / p99 20.0 / max 26.6 (2000 acks, all `ok`), i.e. ~83 sequential writes/s: the write path really costs ~11 ms on the JS hub, not 20 ms of polling. Write latency at 10k/100k/1M events is NOT re-measured yet (needs the scale runs; those go to Boat).
- The Rust `backplane-hub` (files stream's build) answered 500 pings in p50 0.016 ms and 2000 `thread.rename` in p50 0.017 ms, but `renameOk` = 0 and `pingOk` = 0: it does not implement those methods yet, so those are error replies, not writes. They are not a comparison. Only requests both hubs really serve (`files.dir`: 4.26 ms JS vs 0.029 ms Rust, from the `files` stream) are comparable.

## M2 Network

Loopback = desktop client to hub on 127.0.0.1. Tailnet = client on the Mac (hs-mac-mini) to the hub bound to 100.64.84.41 (the Mac is no longer used; its folder `~/bp-gpui-perf` was removed). Hub had 20 streaming stand-ins ("load20") or none ("idle"). The hub soak and M1 reruns shared the machine. One run per cell; 100 requests (60 for switch). Renderer `mobile` view protocol, with and without offering `viewDelta`.

| path | request | p50 ms | p95 ms | bytes p50 |
|---|---|---|---|---|
| loopback, load20 | ping (ws rpc) | superseded (was 20.6, polling artifact); see M1 rtt | | |
| | view.update, whole | 9.8 | 106.7 | 42121 |
| | view.update, keyed delta | 3.5 | 100.1 | 267 |
| | thread switch (view.open) | 24.3 | 155.2 | 47950 |
| tailnet idle | ping | superseded (polling artifact; the Mac is no longer used, not re-measured) | | |
| | view.update whole / delta | 21.3 / 14.5 | 42.5 / 30.2 | |
| | thread switch whole | 19.7 | 48.3 | |
| tailnet load20 | ping | superseded, not re-measured | | |
| | view.update whole / delta | 24.2 / 11.0 | 129 / 102 | |
| | thread switch | 35.6 | 143 | |

The p95/p99 of 100-170 ms under load is the hub busy with 20 streams (single-threaded JS), not the network: the tailnet adds about 1-15 ms.

Bytes per second per viewing client, 20 streaming agents, 30 s, viewers re-fetch `view.update` on each `view.changed` push (<=10/s):

| viewers | whole bodies B/s/client | keyed delta B/s/client | update p50 whole / delta ms | (tailnet) whole / delta B/s |
|---|---|---|---|---|
| 1 | 84,269 | 2,772 | 33 / 50 | 85,552 / 2,827 |
| 5 | 95,610 | 2,993 | 82 / 66 | 92,252 / 3,064 |
| 20 | 111,109 | 3,411 | 204 / 181 | 114,418 / 3,413 |

Finding: keyed delta cuts per-client traffic about 30x (and the hub's total egress at 20 viewers from 2.2 MB/s to 68 KB/s). Both renderers accept the offer (`p.viewDelta = {version:1, token}`). I could not find `viewDelta` anywhere under `native/`, so the GPUI client as it stands gets whole bodies. Thread-open (`view.open`) is never a delta (~48 KB loopback with 21 threads).

Stuck reader (3 raw sockets that finish the ws upgrade of the full log and never read; 45 s before, 90 s stuck, 45 s after, 20 streaming agents): ping series superseded (polling artifact; there were 0 timeouts, and the p95 shape of 83 / 83 / 122 ms came from the hub being busy, but its p50 of 20.6 ms was the harness); hub RSS median 383 / 491 / 476 MB (min 198 / 251 / 260). Other clients' latency is not affected. Memory: floor +50-60 MB during and after, but the baseline itself saws 200-950 MB at 20 agents, so this single run cannot attribute it to the stuck readers: inconclusive on memory. Raw `m2/loopback-load20-stuck3.json`.

Dropped link (reconnect with `since=<log lines held>&origin=<epoch>`, 20 agents streaming): first frame 4.9 ms (3 s gap) and 10.6 ms (10 s gap); the resume sends only the missed log lines (386 and 902), **0 duplicates, 0 missing, first+resume equals the reference stream** for both gaps (`m2/drop-counted-*.json`). Resolved finding: an earlier run reported "20 items absent". Cause: my harness counted the in-flight `live` frame items (one per running thread, 20 here: not log lines) into its `since`, so the resume started 20 lines late. Counting only `log` and `changes` items gives an exact `since` (638 held, `log.since` 638) and nothing is lost. Without `origin` the hub replays the whole log (`since:0`). Text streamed during the gap is not in the log: after a reconnect the `live` frame carries the in-flight reply; deltas are not replayed (by design, not tested for completeness here).

## M3 Database (store.db + events.jsonl; synthetic log)

Fixture: `test/gpui/perf/gen_log.py` (seeded, fresh synthetic text; mix 40% SettingSet, 25% MessagePosted, 20% ActivityLogged, 10% UsageSet, 5% TurnChanged; events/150 threads, one thread holds 3%). Disk checked before (163 GiB free of 1.8 TB, 91% used, need >=40 GiB). One fixture folder, removed after each run. Warm page cache; "cold" below means no store.db (import), not a dropped page cache (no root). One run per size. Hub in an 8 GiB scope.

| events | log MB | store.db MB | import: /hello s | import: first view s | restart with store: /hello s | restart: first view s | first-view body KB | thread switch p50/p95 ms | switch body KB | RSS after boot MB |
|---|---|---|---|---|---|---|---|---|---|---|
| 10,000 | 2.4 | 4.5 | 1.6 | 1.8 | 0.43 | 0.56 | 77 | 39.7 / 49.2 | 119 | 399 |
| 100,000 | 23.6 | 43.2 | 12.3 | 12.9 | 0.70 | 1.06 | 738 | 173.9 / 225.6 | 781 | 679 |
| 1,000,000 | 235 | 437 | 132.7 | 155.2 | 4.6 | 25.6 | see below | see below | | 935 (peak 1,551) |

Writes: SUPERSEDED. The earlier figures (p50 20.3 ms, ~47 writes/s) were the harness's 20 ms polling. Event-driven on a tiny log the JS hub's `thread.rename` ack is p50 11.5 ms (see the correction under M1); at 10k/100k/1M events it is unmeasured. The WAL stayed at ~4.1 MB at both sizes (that part stands; 6-7 of 4000 acks exceeded 50 ms in the polled run, a count that is also not reliable).

What grows with the log: import time (~linear: 1.6, 12.3, 133 s), store.db (~0.44 KB per event), RSS after boot (399, 679, 935 MB), first-view and thread-switch bodies (77, 738 KB at 10k, 100k: 10x per 10x events) and their latency (40 to 174 ms). Opening a thread with 1,370 entries sends 799 KB, so the body is not bounded by the 32-entry message window; with 666 threads at 100k the shown sidebar structure is the likely driver, to be confirmed by splitting the body. What stays flat: WAL size (write and ping latency at scale: unmeasured after the correction).

## Wire: what the hub sends a `renderer: mobile` client, 22 vs 301 threads (stand-in agents)

One project with 22 or 301 threads (`THREADS=`), a mobile-renderer view client on one thread, 70 s per cell, 0 or 5 simulated agents streaming. `push` = fetch `view.update` only after a `view.changed` push; `poll` = fetch once a second. delta0 = no `viewDelta` offer (whole body every time), delta1 = offer. Raw: `build/gpui/results/perf/wire/`. Harness `test/gpui/perf/m2b_wire.mjs`, `wire_ops.mjs`. One run per cell, desktop loopback, hub without the sidebar bound beyond what this build does.

| threads | agents | mode | whole bodies B/s | frame KB | keyed delta B/s |
|---|---|---|---|---|---|
| 22 | 0 | push | 2,464 | 34.4 | 44 |
| 22 | 0 | poll 1/s | 33,931 | 34.4 | 287 |
| 22 | 5 | push | 73,957 | 52.6 | 2,957 |
| 22 | 5 | poll 1/s | 52,023 | 52.6 | 2,667 |
| 301 | 0 | push | 23,989 | 336 | 355 |
| 301 | 0 | poll 1/s | 315,051 | 335 | 591 |
| 301 | 5 | push | 530,232 | 354 | 3,371 |
| 301 | 5 | poll 1/s | 333,058 | 354 | 2,978 |

Cause, by field. A whole mobile body is `{shown, overlay, window, session, ...}`. At 301 threads `shown` is 230-240 KB and `overlay` another 230-240 KB (`overlay.j.projects` repeats `shown.projects`): the thread list is sent twice, about 740 bytes per thread per copy, and it is not bounded in this build. At 22 threads `shown` is 24-34 KB. Frame size grows 9.8x for 13.7x the threads (34 -> 336 KB idle); with the polling clients the byte rate grows 9.3x (34 -> 315 KB/s).

What changes each frame when idle: only each thread row's `ago` text ("3m", "8m"; paths `shown.projects.*.threads.*.ago` and `overlay.j.projects.*.threads.*.ago`). It ticks over once a minute for every thread; at 301 threads that is 903 ops in a minute over 5-6 consecutive frames (3-7 KB each, about 100 KB per minute) against 92 ops (10 KB per minute) at 22 threads. Everything else is quiet when idle: 0.07 pushes/s.

With keyed deltas the traffic is nearly independent of thread count (idle: 287 B/s at 22 threads, 591 B/s at 301; the 5-agent runs are 3 KB/s at both, dominated by the streaming thread's entries). Without deltas it is whole-body repeats. The iOS client's ~20 MB in 70 s at 300 threads equals roughly 285 KB/s, the polled whole-body rate measured here (315-333 KB/s); that points at whole bodies each poll (no `viewDelta` offer taking effect, or delta replies not being applied and the app re-asking whole). The 40x versus 14x ratio was not reproduced: this bench sees 9x idle, 6.9x with 5 agents and polling. The app's frame-size records may double count, as the person said.

Proposed fixes (for the coordinator; I did not edit src/**.bend): (1) make mobile and GPUI clients offer `viewDelta` and apply delta replies; (2) stop sending `overlay.j.projects` as a second copy of `shown.projects` when the client renders only one; (3) page the sidebar thread list (the request's `sidebarRegion` machinery exists for the web renderer) instead of sending all 301 rows; (4) make the `ago` text a client-side function of a timestamp so a clock tick is not 903 ops.

## M4 Leaks: hub soak (2.09 h, 20 simulated agents, JS hub)

`test/gpui/perf/m4_soak.mjs`, fit by `fit.py`. Fresh synthetic home, hub 0.14.0 in a 4 GiB scope, 20 stand-in turns streaming continuously (FAKE_PER=2), one client pinging, sampled every 30 s (252 samples). Raw `build/gpui/results/perf/m4-hub-soak/` (samples.jsonl, debug-perf-*.txt). Other benchmark runs shared the machine at times. Heap-only numbers are unmeasured (the JS runtime does not expose them through /debug/perf; RSS is what was sampled).

| measure | result (after 30 min warm-up) |
|---|---|
| fds | flat: 77-81, slope -0.07/h +-0.31 |
| sockets | flat: slope -0.04/h +-0.27 |
| child processes | flat: 34-43 (the 20 stand-ins and their shells), slope -0.4/h +-1.2 |
| threads | 15-17, slope +0.96/h +-0.23 (about +2 in 2 h; no growth trend worth calling a leak) |
| WAL | flat 4.4-5.8 MB |
| delta lag p50 / ping rtt p50 | 134-168 ms (no drift) / rtt unmeasured event-driven (the logged 20.6 ms was the polling harness) |
| hub RSS, 30-min medians | 518, 561, 530, 559, 587 MB (min 266, max 752) |
| hub RSS floor (10-min minima) | 438, 466, 487, 497, 509, 515, 523, 542, 553, 562 MB |
| RSS floor slope | **+76 MB/h +-9 (95%, 10 bins)**; raw RSS slope +38 MB/h +-15 |
| log growth in the same period | events.jsonl +37.5 MB/h, store.db +53 MB/h (about 157k events/h at 20 simulated agents) |

Reading: no leak of descriptors, sockets, processes or WAL. The RSS floor does rise, steadily, about 76 MB/h, or roughly 0.5 KB per logged event; the hub keeps state derived from the whole log in memory (consistent with the M3 boot figures: 399, 679, 935 MB at 10k, 100k, 1M events, about 0.5-0.9 KB/event). So the rising floor is growth with the log, not an unexplained leak; it is still unbounded, and at this event rate would add about 1.8 GB over 24 h. I cannot separate retention from a true leak with this run alone: the churn test below holds the log nearly constant. A 24 h projection is an extrapolation of a 2 h fit, not a measurement.

## Electron vs GPUI on Linux (RTX 5080), 2026-11-04

Both clients on the same machine, hub, fixture and display. Raw and drivers: `build/gpui/results/perf/cmp/`, `test/gpui/perf/cmp_run.mjs`, `cmp_hub.mjs`, `cdp.mjs`, `scripts/gpui/bench-electron-launch.sh`. Simulated agents only (the hub's `BACKPLANE_AGENTS` read back from `/proc/<pid>/environ` before use).

**GPU / display (logged for both).** Isolated Xvfb `:62` (1440x900x24) with i3 on an empty config; nothing on the person's display or workspace. Adapter: **NVIDIA GeForce RTX 5080, driver 595.99.02, Vulkan, software=false** for both. GPUI logs it (`GPUI_METRIC gpu`). Electron needs flags to leave software rendering on this display: default launch used `llvmpipe` with `gpu_compositing: disabled_software` (measured, not used); the runs use `--use-angle=vulkan --enable-features=Vulkan,DefaultANGLEVulkan,VulkanFromANGLE --ignore-gpu-blocklist --disable-gpu-sandbox --enable-gpu-rasterization --enable-zero-copy` with `VK_ICD_FILENAMES=nvidia_icd.json`, and `SystemInfo.getInfo` reports `ANGLE (NVIDIA, Vulkan 1.4.329 (NVIDIA GeForce RTX 5080))`, `gpu_compositing: enabled`, `vulkan: enabled_on`. Xvfb has no scanout: frames are presented through its software copy path for both clients, so this is GPU rendering into an offscreen X server, not a real on-screen display (unmeasured: true scanout on the person's monitor).

**Hub:** `build/gpui/backplane-serve` 0.14.0 (JS), one project, 20 threads each with a short seeded history, loopback 127.0.0.1:3807, in a 3700M scope. Each client ran in its own `systemd-run --user --scope -p MemoryMax=3700M -p MemorySwapMax=0`. For the load rows the same hub streams 20 stand-in agents (FAKE_PER=2, one delta batch per 100 ms each).

### Results

| | GPUI (`build/gpui/linux/backplane-gpui`, 9.6 MB) | Electron 0.13.0 (installed, packaged) |
|---|---|---|
| Startup to usable: sidebar drawn, median / p95 ms, 10 cold, 20 warm launches | cold 515 / 560; warm 511 / 570 | fresh profile 1065 / 1146; reused profile 782 / 875 |
| First frame, median / p95 ms | cold 472 / 525; warm 471 / 517 | first contentful paint (n=2 fresh, 18 reused): fresh 1136; reused 780 / 842 |
| Idle, thread open, no agents: CPU % of one core (median of 3 x 60 s) | 0.3 | 7.9 |
| Idle RSS / PSS / private MB (client tree) | 284 / 269 / 262 | 719 / 392 / 298 |
| Processes / open fds | 1 / 76 | 9 / 303 |
| 20 agents streaming, thread open: CPU % of one core | 6.7 | 114.8 |
| 20 agents: RSS / PSS / private MB | 288 / 254 / 241 | 716 / 369 / 280 |
| Sidebar click -> thread shown, idle, p50 / p95 / max ms (n=40) | 80 / 96 / 112 | 67 / 98 / 102 |
| Composer popup (type "/"), idle, p50 / p95 / max ms (n=40) | 48 / 112 / 112 | 141 / 154 / 154 |
| Sidebar click, 20 agents streaming | 80 / 160 / 192 | 150 / 243 / 263 |
| Composer popup, 20 agents streaming | 80 / 160 / 352 | 195 / 353 / 362 |
| Size, client only (hub excluded) | 9.6 MB binary, 4.3 MB gzipped; 11 shared libraries from the system | about 288 MB installed (389 MB package minus the 101 MB bundled hub), 9 processes |

Sampling: process-tree `/proc` (RSS, PSS from `smaps_rollup`, private = Private_Clean+Private_Dirty, utime+stime, fds) at 1 Hz for 60 s after 30 s idle (10 s with agents); the figures are medians of 3 launches. GPU memory per client is unmeasured (system-wide `nvidia-smi` only, which moved 1.2-1.7 GB between runs from other users of the card).

### Where the two are not equal (read before quoting)
- **Not the same feature set.** Electron is the full web app (tour, settings, viewers, bots, every panel); GPUI draws only the sidebar, thread, composer and popup so far. GPUI's lower memory/CPU partly reflects doing less.
- **Version.** Electron is the packaged 0.13.0 client; GPUI is built from the gpui branch. The hub is 0.14.0 for both: the Electron app refuses a hub of another version, so it ran with `BACKPLANE_SERVICE_VERSION=0.14.0`. Electron also runs a splash page and joins the hub (`/auth/pair`, discovery) before the page loads; GPUI connects over its own WebSocket.
- **"Usable" differs.** GPUI: first frame with the sidebar drawn (its `usable` metric, taken externally as epoch of the metric minus spawn time, which adds the exec cost). Electron: first sidebar row in the DOM, found by polling over CDP every 20 ms (so +/-20 ms), spawn to detection. Neither includes a selected history or editable composer.
- **Input latency is not like for like.** Electron: real mouse/keyboard events injected through CDP `Input.dispatch*` into Chromium's input pipeline; end = the message-channel callback after the animation frame that follows the DOM showing the thread (a good proxy for the frame, not a scanout measurement). GPUI: the scripted step calls its handler directly (no X11 input path) and ends on the next frame callback after the state shows, so it omits input delivery that Electron includes. GPUI's values come in 16 ms steps (47.8, 63.9, 79.8, 95.8 ...) because it reports on a ~60 Hz frame grid. Both include a round trip to the hub (select, skills list), so the hub's speed is in both. Popup medians moved between the two idle runs I made (GPUI popup p50 96 ms in one run, 48 ms in the final one): with 40 samples and a 16 ms grid, differences under about 30 ms between runs are not meaningful.
- **Cold/warm.** The file-system cache is always warm (no root to drop it). "Cold" here is a fresh Electron profile (empty userData); GPUI keeps no profile, so its cold and warm are the same thing.
- **Counts.** Startup: 30 launches per client. Idle and load: 3 launches, 60 s each. Latency: 40 events per cell from 2 fresh launches (20 threads each). One machine, one day.
- **Earlier runs not used.** A first pass had a contaminated "idle" (agents from an earlier phase still streaming: GPUI 6-7 % CPU, Electron 90-97 %) and an Electron driver that timed out on the page's CSP (`eval` forbidden): both kept under `cmp/run1-*`, `run2-*`, `run3-*` and not quoted.

## M0 baselines: JS hub on the store copy (`~/.backplane-gpui-copy`, 1,691,619 events, 1.56 GB log)

Hub `build/gpui/backplane-serve` 0.14.0 on Bun, `BACKPLANE_COPY=1`, fake HOME, stand-in agents only (`BACKPLANE_AGENTS` verified from the hub's own environ), `127.0.0.1`-independent: bound to 100.64.84.41:3830, `--no-tailscale`, in a `systemd-run --user --scope -p MemoryMax=3700M -p MemorySwapMax=0` scope (no raised cap). Machine: desktop, 32 cores. Harness `test/gpui/perf/m0_baseline.mjs`, `m0_watch.mjs`; raw `build/gpui/results/perf/m0/` (m0.json, import-samples.jsonl, hub logs). The store copy stays on this machine, out of git, and nothing here prints conversation content: only counts, sizes and times. **Note on method:** these M0 numbers were measured with the hub running directly on `~/.backplane-gpui-copy`, before the coordinator made that copy read-only; the runs appended 7,177 events (my stand-in project, threads and their turns), and the copy was then restored and verified by the coordinator. From now on `m0_baseline.mjs` and `m0_watch.mjs` copy first (`cp --reflink=auto` into `build/gpui/perf-run/m0/home`, `df` must show at least 40 GiB free, 3700M scope) and delete the throwaway copy afterwards; the baseline figures themselves stand.

### First start (no store.db): the import does NOT fit under 3.7 GiB
| step | result |
|---|---|
| `store.db` build from `events.jsonl` | finished: 1,691,619 of 1,691,619 lines applied (`applied_seq`), 1.96 GB, about 6 minutes; RSS 175 MB at 30 s, 1.4 GB at 330 s, 2.6 GB at 360 s |
| boot after the build (fold the log into in-memory state) | RSS 2.2-3.4 GB for 6 more minutes, then killed at **12 min 6 s** wall, before `/hello` answered. Sampled peak 3,556 MB; the scope reports "3.6G memory peak" at the 3,700M limit. I did not raise the cap. |
So the first start of a 1.69M-event log needs more than 3.7 GiB with this JS hub; the point at which it stops is the boot fold, after the store is built (the database itself completes). Its memory use after boot was never observed in that first start.

### Later starts (store.db present, hot boot)
3 starts + the main run, all under the cap: `/hello` at 5.6, 8.9, 7.4 s (main run 7.4); first view (mobile renderer open) at 9.3, 12.0, 10.7 s (main 10.7); first-view body 214-239 KB; RSS 1108 / 1097 / 1081 MB after 30 s idle, boot peak (VmHWM) 1617 / 1605 / 1589 MB (the 1M-event synthetic hot boot peaked at 1.55 GB, for comparison). Warm page cache.

### Steady and under load (same hub, after boot)
- Steady idle (60 s after 60 s): RSS **1122 MB**, **3.7 % of a core with nothing happening**, 13 fds, 15 threads. `hub.tick` logs a 1.3-1.8 s stall once a minute at this log size, which accounts for the idle CPU and for pings that land behind it.
- Sidebar holds 504 threads.
- **20 simulated agents streaming (90 s): the hub does not keep up, so this run is INVALID by the plan's rule** (each stream >= 1 delta/s with longest gap <= 2 s): minimum rate 1.7 deltas/s but longest gap 6.2 s. Hub CPU **97 % of a core (saturated)**, RSS 1202-1526 MB (peak 1583), 40 child processes, delta lag p50 **21.5 s**, p95 38 s, max 40 s (3,637 deltas received, 35/s; on the 20-thread fixture hub the same agents give 165 ms lag at 20 % CPU), ping p50/p95/p99 488 / 772 / 2281 ms (event-driven client). Every Rust milestone is measured against this: the JS hub on a real-shaped 1.7M-event store cannot sustain the 20-agent load.

### Click and popup latency (mobile-renderer view client over loopback to 100.64.84.41; 40 events each; event-driven)
| action | idle p50 / p95 ms | body p50 | 20 agents p50 / p95 ms | body p50 |
|---|---|---|---|---|
| thread switch (`view.open`, my own thread) | 461 / 632 | 329 KB | 560 / 701 | 347 KB |
| thread switch (a thread from the real sidebar) | 527 / 822 | 360 KB | 665 / 887 | 371 KB |
| sidebar click (`view.action select`) | 12.5 / 442 | 15 KB | 96 / 615 | 23 KB |
| composer popup (draft "/" then `view.update`) | 441 / 527 | 1.1 KB | 698 / 880 | 1.3 KB |
These are request round trips to the hub (server time + transfer), not a client's input-to-pixels; the GPUI/Electron comparison above measures the client side on the small fixture. "Idle" here has the hub's 3.7 % background and its 1.4 s per-minute stall in it. The sidebar-click p50 of 12.5 ms is the select action alone returning a small body (16 KB); p95 442 ms is the clicks that land behind the minute tick.

### Live hub, reference only (`/proc`, nothing sent to it)
Hub pid 3890 at 22:57: RSS about 20.5 GB (peak 32.0 GB), 140 % of a core over 30 s, 16 threads. A single sample, not a baseline; it only shows the scale the production hub has reached.

## GPUI client soak (Linux, RTX 5080, isolated Xvfb :62)

`test/gpui/perf/cmp_soak.mjs`; raw `build/gpui/results/perf/gpui-soak/samples.jsonl`. The GPUI binary (9.6 MB) on the small fixture hub with 20 simulated agents streaming (hub RSS about 700 MB), thread 0 open, in a 3700M scope; sampled every 30 s from `/proc` (RSS, PSS, private, fds, threads). **38 minutes, 77 samples: shorter than the 2 hours I would like, so this is a short soak, not a leak verdict.** The harness process ended about 2 minutes before its 40 minutes (the work watcher's time limit); the client and hub were stopped by PID afterwards.

| measure | result |
|---|---|
| RSS (after 5 min) | 287 MB at start, floor 262 MB, max 290 MB, 263 MB at the end |
| RSS floor slope | -49 MB/h +-11 (naive 95 %, n=67) after warm-up, i.e. falling, not rising; the 10-minute floors are 290, 262, 262, 263 MB |
| PSS / private at the end | 226 / 216 MB |
| fds / threads | 76 / 47, constant for the whole run (slope 0) |
| CPU | 7.1 % of a core on average (agents streaming) |
| GPU memory (system-wide, nvidia-smi) | 1.2-1.8 GB, shared with other users of the card; per client unmeasured |

Reading: no growth in memory, fds or threads over 38 minutes with the sidebar and one thread updating continuously. A 2 h run and a churn run (switching threads, reconnecting, creating and deleting threads) on the GPUI client are not done; `m4_churn.mjs` exists for the hub and was not run (the desktop pause came first, and it was not on the approved list).

### Correction note on what "ping" measured (2026-11-05)
`load.ping` is not a method of either hub: the reply is `{"ok":false,"error":"unknown method"}` on the JS hub and on the Rust `backplane-hub` alike (checked with `remeasure_rpc.mjs`, which now prints the first replies). So every "ping rtt" in this file is the round trip of a cheap error reply through the socket and the hub's request path: a valid floor for "a request is read and answered", but not a work request and not comparable with a write. `thread.rename` on the JS hub is a real write (reply ok, 2000 of 2000): p50 14.6 ms / p95 21.3 / p99 27.0 / max 134 ms in a rerun made while another run was using the machine (11.5 ms p50 in the quiet run above). The Rust hub answers `thread.rename` with "unknown method" too, so no Rust write latency exists yet; its ping and rename timings (p50 0.02 ms) are error replies and stay out of every comparison. To be rerun when the Rust hub implements them (`rpc-eventdriven-2.json` has this run).

## Churn and 2 h GPUI soak: stopped, partial (2026-11-05)
The person needed the desktop (RTX 5080 and CPU) for something else, so `run_final.sh` was stopped by killing its whole process group about 90-130 seconds into its first stage. Nothing after the hub churn started (no Xvfb :62, no i3, no GPUI client, no 2 h soak).
- **Hub churn on the seeded log: PARTIAL, about 100 s of the planned 20 minutes.** `build/gpui/results/perf/m4-churn/samples.jsonl` has 10 samples; it is far too short to say anything about leaks and must not be read as a churn result. Kept only as a record that the harness ran.
- **GPUI churn with reconnects (20 min) and the 2 h GPUI soak on the seeded hub: NOT RUN.** They wait until the desktop is free. The 38-minute soak above remains the only GPUI soak result.
- Ready to run when asked: `test/gpui/perf/run_final.sh` (hub churn, GPUI churn with two hub restarts, then `cmp_soak.mjs 120`); the hub churn can also go to Boat or h@backplane.

## M4 hub churn on Boat (seeded log, 20 min) — replaces the partial desktop run above
Boat sandbox (4 cpus, 7.9 GB, the sandbox is the hard cap), JS hub 0.14.0 on a seeded log (20 threads, a short finished turn each, stand-in agents), `test/gpui/perf/m4_churn.mjs` via `boat-churn.sh`; raw `build/gpui/results/perf/m4-churn-boat/`. Three workers for 20 minutes: 5,466 plain-client reconnects, 10,496 view opens across threads, 3,261 thread create+delete pairs; **0 errors**. Sampled every 10 s (121 samples).
- Flat: fds 74-77, sockets 64-67, threads 12-13, child processes 20, WAL 4.1 MB, log +1.2 MB (events.jsonl 0.29 -> 1.52 MB), store.db 0.8 -> 3.7 MB.
- RSS floor rose: 10-minute-ish floors 278, 347, 387, 403 MB, from 155 MB at the start; naive slope +406 MB/h +-55 over the run, with the median RSS 292 -> 467 MB. That growth is not explained by the log (1.2 MB of log for about 300 MB of RSS), so it is churn-related retention or slow GC, not log growth. After the workers stopped the final reading was 331 MB, so part of it is released: not a monotonic leak, not proven harmless either.
- Limits: 20 minutes, one run, no no-churn control on Boat, no heap numbers. It cannot say whether the floor levels off. A 2 h churn plus a quiet control is what would settle it; the desktop is off limits, so that waits for Boat or h@backplane time.
- Boat quirks met: `--include-file` ships only that file (the whole-repo + binary run uses an untracked file under `test/gpui/perf/bin/`, removed afterwards); `build/` is never shipped; the job ended "exit 70, status never arrived" although the command exited 0 and printed its results.

## Final-gate e2e harness (`test/gpui/e2e/`), status 2026-11-05
Harness for the 12 gate journeys and the frame measurements; design and the requests G1-G5 for GPUI are in `test/gpui/e2e/CONTRACT.md`. Unit-tested on Boat (`frames.test.mjs` incl. an ffmpeg changed-frame round trip: 18 of 20 moving frames found, 0 of 20 static). Proof runs on Boat (software-rendered llvmpipe under Xvfb+i3, seeded JS hub, stand-in agents, Linux GPUI binary): the pipeline runs end to end and records an mp4 and a screenshot per journey.
- GPUI journeys that run today: switch-fast (60 selects, result p50 about 72 ms under llvmpipe), stream, popups, long session. 8 of 12 journeys are `blocked` on GPUI hooks G5 (pair, scroll, paste image, bots/rooms, viewers, files, download, settings, network sleep), settings also on G4; none passes, because click ack, frames, layout shifts and dead controls need G1-G4.
- Real GPUI defect found: popups for `@`, `>` and `%` never appear (`/` and `$` do): 3 of 6 popup steps failed, and 13 of 50 steps in a 3-minute random walk (reported to the screens lane).
- Evidence limits stated in the output: click ack derived from the recording is coarser than the 16.7 ms budget at x11grab 30 fps (alignment about +-60 ms; the median ack came out later than the median result), so `pass` is reported unknown, not true; only GPUI's own ack event (G1) can settle it. Boat renders in software, so frame figures from it are harness validation, not GPU evidence.
- iOS and Android drivers are written by their streams under `test/gpui/e2e/{ios,android}/` and plug in through `adapters/`; the fixture hub for devices is `serve-fixture.mjs` (run on h@backplane; cut/restart control endpoint). Nothing ran on the desktop.

### e2e, first run with the G1-G5 GPUI build (Boat, llvmpipe, JS hub; 2026-11-05 10:02)
`gpui` build of 05:01 (G1-G5, `frames_ms` fix a2d73ca74) against the seeded JS hub, software-rendered, so none of this is GPU or Rust-hub evidence and nothing is a pass (rule in `test/gpui/e2e/CONTRACT.md`). Raw: Boat job j20261105100229-1877456, artifacts/gpui/summary.json (+ mp4/png per journey).
- Click ack (G1, GPUI's own event): p50 48-64 ms, p95 72-83 ms; the software renderer's frame interval is about 48 ms (19-23 fps; 89 of 91 frames janky in projects-threads, 343 of 490 while streaming), so the one-frame budget (16.7 ms) is not judgeable here. Result times p50 60-144 ms.
- Layout shift (G3): score 0.006-0.063 (<= 0.1, passes); the composer and the rail are what moved without an input.
- Controls (G4): 44 visible controls on a thread screen, 23 wired, 0 dead among the wired; 21 visible controls are unwired (`wired:false`), which the sweep now fails. Defect to GPUI: `net_sleep` over 15 s always fails (15 s timer).
- Popups `@`, `>`, `%`: still fail (15 of 65 steps in the 3-minute walk). Memory over 3 minutes: +24 +-11 MB/h (too short to read).
- 6 journeys blocked on G5 steps not implemented (pair, paste image, bots/rooms, viewers, downloads, settings). The dead-controls rerun that saves the control list did not start: Boat refused ("start limit per day: 837 of 840"); reruns wait for the quota to reset.

### e2e with the 839a84025 GPUI build (Boat, llvmpipe, JS hub; 2026-11-05 10:32-10:45)
Still software-rendered and on the JS hub, so no journey is a pass (rule in CONTRACT.md). Raw: Boat jobs j20261105103241-2102997 (all journeys), j20261105104525-2228495 (dead-controls).
- Popups: `@`, `>` and `%` now open (6 of 6 popup steps ok; a popup step succeeds only when the hub's rows are non-empty). In the 3-minute random walk 3 of 160 steps failed, all `$` popups (cause unknown).
- Reconnect: `net_sleep` 45 s now succeeds (the secs+15 timer), so GPUI reconnects.
- Dead controls: 48 visible controls on a thread screen: 25 wired, all 25 clicked, **0 dead**; 23 have no handler and are drawn disabled (Terminal, Commands, the 11 rail tabs, Jump or run a command, Improvements, Search, Settings, Add project / New thread / New bot / New room, Account, Edit, Context, Dictate). `dead.pass` is false because of them; `passIfDisabledAccepted` is true. Whether disabled-looking placeholders satisfy "no dead buttons" is the reviewer's call. (Rows of a streaming thread change ids every few seconds and are skipped as transient by the click step's `found:false`.)
- Click ack p50 48-61 ms under llvmpipe (frame interval about 48 ms): the one-frame budget stays unjudged until real hardware. Long session (3 min): 160 inputs, +24 +-11 MB/h RSS, too short.
- Boat note: the harness now keeps a warm sandbox for the thread (no new start per rerun); the day's start limit was 837/840 when I looked.

## GPUI input driver and the must-pass input actions (Boat Xvfb, llvmpipe, JS hub; 2026-11-05)
Driver: `test/gpui/e2e/input/` (xdotool/XTEST mouse and keys, xclip clipboard/PRIMARY, an XDND file-drop source, ibus + Hangul), README for mechanics there; `boat-run.sh input-selftest` proves each mechanism (xev saw click, 13 motion events, release, Ctrl-C, é/日, wheel; clipboard text/image/TARGETS/PRIMARY; XDND drop finished and the target read the path; ibus starts with the Hangul engine). `boat-run.sh input-run HUB GPUI` runs the actions against the real client and judges them by what the hub logged, not by changed pixels (a streaming thread changes pixels by itself; my first verdicts used pixels and were wrong).
Software-rendered, JS hub, so nothing here is a pass; these are findings about the current GPUI build (05:31 binary):
- Drag-select: nothing is selected (no highlight, PRIMARY empty); Ctrl-C copies nothing (clipboard empty).
- Paste an image: GPUI shows "Uploading pasted-….png…" and the file does arrive on the hub (1.2 KB attachment), but the message that is sent has only the text ("see pic"): no image reference in `MessagePosted` or any other new event.
- Drop a file: the XDND drop is accepted (XdndFinished accepted) but no upload follows (no new attachment).
- IME: with ibus + Hangul running before the client and `XMODIFIERS=@im=ibus`, typing `g k s space` arrives as the raw text "gks", not "한": no composition.
- Not testable yet: paperclip and link clicks (the controls and link events carry no rectangles: request G6, to the GPUI desktop stream).

### Input actions with the G6 build (06:35 binary; Boat job j20261105113556-2656130)
Same software-rendered, JS-hub run, now with control rectangles and the composer read:
- IME: the composer reads back `"gks "` (supported:true) with ibus + Hangul running first: confirmed, no composition. Drag-select and Ctrl-C: still nothing selected or copied (`read selection` is `supported:false`).
- Paste image: the "Uploading pasted-….png…" chip never leaves the composer (still there more than 60 s later and after later messages were sent) although the 1.2 KB file is on the hub, and the sent message has no image reference: the upload completion is not being registered. Drop file: accepted by XDND, no upload.
- Paperclip (aimed from the control's own rect): no new top-level window and nothing drawn after 2.5 s. On a Boat sandbox that may only mean there is no file-chooser portal service, so it is unverified rather than failed; a real-hardware run decides.
- Link clicks: blocked, GPUI has no link detection yet (`links` supported:false).

## GPUI input driver with a harness self-check (Boat job j20261105144124-70308; 06:35 GPUI build, llvmpipe, JS hub; 2026-11-05)
`test/gpui/e2e/input/preflight.mjs` runs before GPUI starts and proves, against plain X clients, the pointer, keys, wheel, clipboard text/image/files, PRIMARY, TARGETS, an XDND drop, the ibus daemon + engine switch + XIM server registration, and that a **plain XIM client (xev) gets 한 from `g k s space`**; core-input failures stop the run as `harness`, a failed tool marks only its own action `harness`. `input-run` also proves the client receives pointer events (click another thread row, require 48,866 changed pixels in the main area: passed) and clears clipboard/PRIMARY before the actions (xclip forks, so a leftover from the preflight had produced two false passes: fixed and checked by `selections-cleared-after-preflight`). Earlier "all false" results (job j20261105141708-3946944 and mine before today's fixes) came partly from the harness: missing tools in a fresh sandbox (python3-xlib, ibus-hangul), stale selections, an ibus started after the client.
Per action on the 06:35 build (SUPERSEDED by the current-tip run below: that build predates screens' Ctrl-C, PRIMARY, paste and drop fixes, and two of these falses were also my harness's):
- drag-select: **false, app**. Nothing is selected (PRIMARY empty); GPUI receives the drag (pointer proven).
- Ctrl-C: **false, app**. Clipboard empty (`read selection` is `supported:false`).
- paste-image: **false, app**. The image uploads (1,229-byte file on the hub) but the "Uploading pasted-….png…" chip never clears and the sent message is text only (no image reference in `MessagePosted` or any other new event).
- drop-file: **false, app**. XDND accepted (`finished`), no upload started.
- ime-hangul: **harness, not run**. The IME check fails in the preflight itself: ibus runs with the Hangul engine (engine process present, XIM server registered, UTF-8 locale) but a plain XIM client gets no filtered keys (`XFilterEvent: False`). Not solved; GPUI is not blamed. (Earlier raw-"gks" result from the composer read stands as a fact about GPUI only if XIM composition is first shown to work in a plain client.)
- paperclip: **the client's own click works, the OS click missed**. GPUI's scripted `click ui:Attach` calls the file-chooser portal (stand-in logged `OpenFile`, returned the picked file); an xdotool click aimed at the reported control rect (1113,847,40,44) changed nothing and never reached the portal, because the painted button sits about 25 px higher (screenshot overlay: red rect below the button): the `controls` rect for `ui:Attach` is offset in y.
- link click: blocked (`links` supported:false).


### Input actions on a client built from the current gpui tip (the verdicts that stand; 2026-11-05)
Client: built on Boat (job j20261105144704-134325, `scripts/gpui/boat-build.sh`, large sandbox, 5 min 33 s) from my worktree at `db29b9cee`, which is `gpui` at `8c2701e772` plus test-only commits; `native/gpui` is byte-identical between `8c2701e772` and the current `gpui` tip `b7802aac97`, so this is the current tip's client. Run: Boat job j20261105151657-419917 (software rendering llvmpipe, JS hub, harness commit `c5a18df6e8` on `gpui-perf`). The 06:35 binary's falses are retired.
Harness fixes that changed verdicts (each was a false result of mine, found by reading the evidence): preflight left text in the clipboard/PRIMARY (two false passes); the pointer-sanity click chose the empty "New thread" row and the drag then ran in an empty thread (a false fail); pointer delivery was first judged on the main area only, which does not change between two seeded threads with identical text (now also the sidebar highlight); the image check first matched the word `images` in the key `"images":[]` (a false pass).
| action | verdict now | evidence |
|---|---|---|
| drag-select | **pass-so-far** (partial: software render, JS hub) | PRIMARY holds the dragged assistant text |
| Ctrl-C | **pass-so-far** | the clipboard holds the same text, targets UTF8_STRING, text/plain;charset=utf-8 |
| paste image | **false, app** | the image uploads (1,229-byte file on the hub) but the thread the hub shows has `"images":[]` and the sent message is text only |
| drop file | **false, app, changed** | now uploads (new since 06:35: `paste.png` on the hub) but the same: nothing attached to the message |
| paperclip | **unverified** | the client's own `click ui:Attach` calls the OS chooser (stand-in logged one OpenFile); an OS click aimed at the reported rect does nothing: the rect is ~25 px below the painted button |
| IME | **harness, not run** | one attempt each with ibus+Hangul and fcitx5 (XIM frontend + hangul): a plain XIM client (xev) never gets 한 (`XFilterEvent: False`; ibus engine runs, fcitx5 reports no active engine), so GPUI cannot be judged; parked for a real-hardware run |
| link click | blocked | GPUI has no link detection (`links` supported:false) |
None of these counts as a pass: the gate needs the Rust hub on real hardware.

### Input actions on client `cbbd8ebec4` (screens lane's 12:05 Boat build; run job j20261105171749-1578612; llvmpipe, JS hub; runner `a5bcbc95ae`)
This supersedes the paste and drop falses above: they were about the older client. The sent message now carries the hub's attachment reference (`see pic\n\n[attachment: …/attachments/<thread>/…-pasted-….png]`, and the same for the dropped `paste.png`), and the file is on the hub, so both actions pass here (pass-so-far only: software render, JS hub). My earlier test looked for an `images` field, but the hub records an attachment as that text line; the judge accepts it because it reads the whole sent message.
| action | verdict | evidence |
|---|---|---|
| drag-select | pass-so-far | PRIMARY holds the dragged assistant text |
| Ctrl-C | pass-so-far | clipboard holds the same text (UTF8_STRING and text/plain targets) |
| paste image | **pass-so-far, changed** | sent message has `[attachment: …pasted-….png]`, file on the hub |
| drop file | **pass-so-far, changed** | sent message has `[attachment: …paste.png]`, file on the hub (XDND accepted) |
| paperclip | unverified | the client's own `click ui:Attach` reaches the OS chooser (stand-in logged one OpenFile); the OS click aimed at the reported rect (still 40x44 at 1113,847) does nothing |
| IME | harness, not run | the plain-client check still fails (ibus and fcitx5), so GPUI is not judged |
| link click | blocked | no link detection |
Runner change: a missing GPUI window is now a nonzero `harness` failure (exit 4) with the client's stderr/stdout, the hub log, token/asset checks and an xwininfo tree in the artifact (`summary.windowSearch`); the Boat wrapper no longer hides the exit code behind `tail`. Mechanics' job j20261105171315-1515229 (client `fb25d984d1`, no window) cannot be explained from its artifact, which has no client log; with `cbbd8ebec4` the window appears at once (title "Backplane GPUI · experimental" found by name). Next time it happens the artifact says why.

## The switch-over path: the Rust hub on the real data copy (2026-11-05, desktop, 3700M scope, stand-in agents, copy guard on)
Hub: `backplane-hub` built on h@backplane from gpui `e086c32e2c` (12.8 MB), started with `--policy-bin bp-policy-pager --window-bin window` (the helpers from `build/gpui/slim` and `build/gpui/hub/new`), `BACKPLANE_COPY=1`, a fake HOME, on a throwaway copy of the store copy's `events.jsonl` (1,691,619 events, 1.56 GB; the JS `store.db` not used), in `systemd-run --user --scope -p MemoryMax=3700M -p MemorySwapMax=0`. Raw: `build/gpui/results/perf/m0-rust/` (import-samples.jsonl, hub-first.log), harness `test/gpui/perf/m0_baseline.mjs`, `m0_watch.mjs`, `run_m0_rust.sh`.
**Result: the first start does not finish under 3.7 GiB. The kernel OOM-kills the scope 33-37 s after start (3.6 GB peak, five OOM kills of six launches, the sixth stopped on a permission error before any work; same every time), before `/hello` answers. I did not raise the cap.**
Where the memory is (cgroup `memory.stat` plus every process in the scope, sampled every 0.5 s in the last run): the hub process stays at 71 MB anonymous; **`bp-policy-pager` (the Bend-native policy child) grows linearly at about 112-118 MB/s: 380 MB at hub+4 s, 1.1 GB at hub+12 s, 2.0 GB at hub+20 s, 3.3 GB at hub+28 s, 3.6 GB at hub+32 s**, then the scope is killed. Two clocks, so the numbers can be compared: the *hub clock* starts when the hub process starts (the kernel's journal: killed 33-37 s after start, 3.6 GB peak); the watcher's raw file `import-samples.jsonl` uses the *watcher clock*, which started about 30.8 s earlier because the script copies the 1.56 GB log first (watcher 34.8 s = hub+4 s, 63 s = hub+32 s). The two statements are the same observation. Page cache is 0 and dirty pages 0: it is the child's anonymous heap. So it is the transitional presenter/policy sidecar (docs/gpui/PORT.md stage 1), not the Rust hub's store, that cannot open a real-size log within the cap; it needs 1.5 GB of log to be folded and apparently keeps all of it.
Not measured, because the hub never answers: startup to usable, steady memory and CPU, 20-agent load, click/popup latency on the Rust hub at real size. The JS hub baselines in the M0 section are unchanged (JS hub could not import this log under 3.7 GiB either, but its later starts from a built store.db worked: 1.1 GB RSS, 7 s to /hello).

**Made under a hold.** These runs (six launches on 2026-11-05 EST: one at about 12:58-12:59 that exited after 3 s on `append events.jsonl: Permission denied`, then five OOM kills at 13:00:33, 13:02:02, 13:03:17, 13:04:46 and 13:06:06) were made while a hold was in force on any Rust hub on a copy of the real log, which I had not been told. The coordinator has the facts. `run_m0_rust.sh` now refuses a real log and defaults to the synthetic real-size log in `~/.backplane-gpui-scratch`; `m0_baseline.mjs` also refuses it unless `HOLD_LIFTED=coordinator-lifted`.
**Does this explain `SIDE-BY-SIDE.md`'s "28 s stop"?** No, not as written. That run peaked at 1.6 GB with no OOM line in the journal; every one of mine reached 3.6 GB and the journal says `oom-kill`. The same helper growth (100+ MB/s) would reach 1.6 GB in about 14 s, so a stop at 28 s with 1.6 GB is a different end (a different helper build, cap or cause); it stays unexplained by this evidence.

## Lane 7: the real-log open test (`test/gpui/perf/real_open.mjs`, tip b04f1f52a2; synthetic only so far)
What it is: a hub and the GPUI client, each in its own 3700M/swap-0 scope, on a throwaway copy of a log (`restart.continue off` appended, stand-in agents, `BACKPLANE_COPY=1`, no D-Bus/XDG_RUNTIME_DIR for the hub), 10 streaming stand-in agents and 5 views, on an isolated display it requires. It reports the person's pass condition (`result.criteria`, `pass`): whole startup including the store build under 1 GB hub anon memory, `/hello` answers, no OOM, steady hub anon memory at most 500 MB, 10 agents and 5 views up, client `usable`. It refuses real-log paths without the coordinator-relayed lift, and refuses to run uncapped.
What has run: **only synthetic logs, and with a stand-in script in place of the GPUI client.** 100k-event log: `pass:true` (hub up in 5.8 s, startup anon peak 346 MB, steady hub anon 403 MB including the policy pager at 343 MB). That `pass` used the fake client, so it says **nothing about the gate**: no real GPUI client has been through this test and no client figure (usable time, client memory) is measured. 1.09 GB synthetic log (1.69M events): `oom`, hub killed at 26 s, 3.63 GB peak of which the policy pager holds 3563 MB; the same finding as the switch-over section above. A hub SIGKILLed mid-steady gives `hub-exited`, `pass:false`, exit 2. Nothing has run on a copy of the real log.
Next: on this desktop in an isolated Xvfb (`:61`), capped, on the synthetic log, once the helper-free hub exists; until then it repeats the OOM.
