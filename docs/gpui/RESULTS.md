# GPUI experiment: perf evidence (stream `perf`)

Machine for hub numbers: desktop, 32 cores, 59 GiB, Linux 7.0. Hub: `build/gpui/backplane-serve` 0.14.0 (commit 96a230931d tree), fresh temp home per run, `--no-tailscale --foreground`, in `systemd-run --user --scope -p MemoryMax=4G -p MemorySwapMax=0`. Stand-ins: `test/tools/perf_standin.ts` (3 deltas/tick, 10 ticks/s... FAKE_PER=2 here, cycle 20). No model calls. Raw: `build/gpui/results/perf/`. Harness: `test/gpui/perf/`.

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
