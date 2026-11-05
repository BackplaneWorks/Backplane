# GPUI experiment: perf evidence (stream `perf`)

Machine for hub numbers: desktop, 32 cores, 59 GiB, Linux 7.0. Hub: `build/gpui/backplane-serve` 0.14.0 (commit 96a230931d tree), fresh temp home per run, `--no-tailscale --foreground`, in `systemd-run --user --scope -p MemoryMax=4G -p MemorySwapMax=0`. Stand-ins: `test/tools/perf_standin.ts` (FAKE_PER=2 deltas per 100 ms tick, cycle 20). No model calls. Raw: `build/gpui/results/perf/`. Harness: `test/gpui/perf/`.

## M1 Hub under load (one WebSocket client, 10 s warm-up, 60 s measured, 1 run per count)

| agents | valid | min deltas/s/stream | longest gap ms | hub RSS MB start / after warm / end / peak | hub CPU (core %) | ping rtt ms p50/p95/max | delta lag ms p50/p95/max | deltas/s total | client KB/s | events.jsonl KB |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | True | 4.9 | 276 | 292 / 297 / 299 / 397 | 3.6 | 20 / 21 / 41 | 174 / 201 / 237 | 5 | 1.6 | 35 |
| 10 | True | 5.3 | 455 | 312 / 344 / 237 / 646 | 11.4 | 20 / 83 / 103 | 175 / 232 / 422 | 53 | 15.7 | 349 |
| 20 | False | 5.2 | 357 | 291 / 222 / 250 / 679 | 18.2 | 21 / 81 / 102 | 169 / 254 / 357 | 112 | 32.8 | 722 |
| 50 | False | 5.2 | 527 | 312 / 246 / 573 / 692 | 34.8 | 41 / 82 / 183 | 267 / 530 / 741 | 266 | 79.5 | 1754 |

Validity (plan rule: each stream >=1 delta/s, longest gap <=2 s): met at 1, 10, 20, 50. Note the hub merges and paces deltas, so each stream arrives at about 5/s (not the 20/s emitted); the first harness run flagged 20 and 50 invalid only because it counted the project's auto-created thread; fixed in `m1_load.mjs`, verdict recomputed from `perThread`.

Observations (n=1 sample per count, so treat as indicative):
- Idle hub is ~290-310 MB RSS right after start, before any agent. That baseline is the Bend/JS runtime, not load.
- CPU scales roughly linearly: 3.6 / 11 / 18 / 35 % of one core for 1/10/20/50 agents (the hub is single-threaded JS; 50 agents is a third of one core).
- Ping rtt has a 20 ms floor (event-loop tick), p95 ~80 ms at 10-50 agents, max 183 ms at 50.
- Delta lag (stand-in stamp to client): ~170 ms p50 at 1-20 agents (the hub's Delta.pace batching, not load), 267 ms p50 / 741 ms max at 50.
- RSS peak is 2x the idle level (650-690 MB) and RSS moves up and down by hundreds of MB inside a run: GC sawtooth, not a conclusion about leaks. M4 soak decides.
- events.jsonl grows ~30 KB per agent per minute of streaming; the WAL stays at ~4.3 MB (checkpointed).

M1 rerun (fixed harness, 20 agents, 60 s): valid=true, min 5.29 deltas/s/stream, longest gap 502 ms, 20 streams; hub CPU 22.1 % of a core; ping rtt p50/p95/max 20.6/100/184 ms; delta lag p50/p95/max 149/343/557 ms; RSS 310 start, 389 after warm-up, 239 end, 709 peak MB. Raw: `build/gpui/results/perf/m1-20agents/result.json`. (Concurrent with the hub soak and M2 on the same 32-core box.)

### Why the round trip is a flat 20 ms and delta lag ~170 ms
- Delta lag: `Delta.pace()` = 200 ms (src/core/hub.bend:1519). A read that ends in streamed text waits 200 ms before the next read and merges the run of deltas into one. That is built-in pacing, not work; load only moves it at 50 agents (p50 267 ms).
- WebSocket RPC round trip: HTTP `/hello` on the same idle hub is 0.95 ms p50; `load.ping` over the WebSocket is 20.3-20.5 ms p50 whatever the gap between requests (0, 3, 30, 200 ms; `test/gpui/perf/floor.mjs`). A constant independent of gap is a timer. I did not find the constant in src/**.bend, so its origin is unlocated; it is not computation. Every WebSocket-RPC number below carries a 20 ms floor (view.update with delta reads 3-10 ms, so view replies are not on that floor; plain `load.ping`, thread.rename acks are).

## M2 Network

Loopback = desktop client to hub on 127.0.0.1. Tailnet = client on the Mac (hs-mac-mini) to the hub bound to 100.64.84.41 (the Mac is no longer used; its folder `~/bp-gpui-perf` was removed). Hub had 20 streaming stand-ins ("load20") or none ("idle"). The hub soak and M1 reruns shared the machine. One run per cell; 100 requests (60 for switch). Renderer `mobile` view protocol, with and without offering `viewDelta`.

| path | request | p50 ms | p95 ms | bytes p50 |
|---|---|---|---|---|
| loopback, load20 | ping (ws rpc) | 20.6 | 119.8 | |
| | view.update, whole | 9.8 | 106.7 | 42121 |
| | view.update, keyed delta | 3.5 | 100.1 | 267 |
| | thread switch (view.open) | 24.3 | 155.2 | 47950 |
| tailnet idle | ping | 22.0 | 22.1 | |
| | view.update whole / delta | 21.3 / 14.5 | 42.5 / 30.2 | |
| | thread switch whole | 19.7 | 48.3 | |
| tailnet load20 | ping | 21.7 | 86.9 | |
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

Stuck reader (3 raw sockets that finish the ws upgrade of the full log and never read; 45 s before, 90 s stuck, 45 s after, 20 streaming agents): ping p50 20.6 / 20.6 / 20.5 ms, p95 83 / 83 / 122 ms, 0 timeouts; hub RSS median 383 / 491 / 476 MB (min 198 / 251 / 260). Other clients' latency is not affected. Memory: floor +50-60 MB during and after, but the baseline itself saws 200-950 MB at 20 agents, so this single run cannot attribute it to the stuck readers: inconclusive on memory. Raw `m2/loopback-load20-stuck3.json`.

Dropped link (reconnect with `since=<items held>&origin=<epoch>`, 20 agents streaming): first frame in 5.2 ms (3 s gap) and 13 ms (10 s gap), resume sends only the missed items (421 and 841), 0 duplicates, `log.since` honoured. 20 items of the reference stream (equal to the thread count) were absent from first+resume; unexplained, needs a follow-up before saying "nothing lost". Without `origin` the hub replays the whole log (21k items, `since:0`). Live (uncommitted) deltas are not part of the log: after reconnect text streamed in the gap is only what the `live` frame carries.

## M3 Database (store.db + events.jsonl; synthetic log)

Fixture: `test/gpui/perf/gen_log.py` (seeded, fresh synthetic text; mix 40% SettingSet, 25% MessagePosted, 20% ActivityLogged, 10% UsageSet, 5% TurnChanged; events/150 threads, one thread holds 3%). Disk checked before (163 GiB free of 1.8 TB, 91% used, need >=40 GiB). One fixture folder, removed after each run. Warm page cache; "cold" below means no store.db (import), not a dropped page cache (no root). One run per size. Hub in an 8 GiB scope.

| events | log MB | store.db MB | import: /hello s | import: first view s | restart with store: /hello s | restart: first view s | first-view body KB | thread switch p50/p95 ms | switch body KB | RSS after boot MB |
|---|---|---|---|---|---|---|---|---|---|---|
| 10,000 | 2.4 | 4.5 | 1.6 | 1.8 | 0.43 | 0.56 | 77 | 39.7 / 49.2 | 119 | 399 |
| 100,000 | 23.6 | 43.2 | 12.3 | 12.9 | 0.70 | 1.06 | 738 | 173.9 / 225.6 | 781 | 679 |
| 1,000,000 | 235 | 437 | 132.7 | 155.2 | 4.6 | 25.6 | see below | see below | | 935 (peak 1,551) |

Writes (`thread.rename` request to ack, 4000 sequential, 1 client): p50 20.3 ms, p99 41 ms, max 160-200 ms at both 10k and 100k; ~47 writes/s sequential, which is the 20 ms ack floor again, not the store. 6-7 of 4000 exceed 50 ms. WAL stays at ~4.1 MB at both sizes (checkpointed, no growing WAL).

What grows with the log: import time (~linear: 1.6, 12.3, 133 s), store.db (~0.44 KB per event), RSS after boot (399, 679, 935 MB), first-view and thread-switch bodies (77, 738 KB at 10k, 100k: 10x per 10x events) and their latency (40 to 174 ms). Opening a thread with 1,370 entries sends 799 KB, so the body is not bounded by the 32-entry message window; with 666 threads at 100k the shown sidebar structure is the likely driver, to be confirmed by splitting the body. What stays flat: ws-RPC and write ack latency (20 ms), WAL size, ping at idle.

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
