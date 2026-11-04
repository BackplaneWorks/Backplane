# PR929: final lane A review

**APPROVE the narrow Disk.sync effect/component change.** Tested source: `0139fd6d12910082e7be7f930cb5a12a0eec3ead`, tree `c285018e43b807e2535f70fb382e3aeefd37cfbc`. PR head `74440d2824a637cf3588b432ee40f3b03e763ad3` only adds documentation and receipts; I independently verified that every `src` and `test/tools` byte matches the tested revision. No remaining code-review blocker in this slice. This verdict does not authorize a merge or establish whole-hub, actual Hub RPC/SQLite commit, startup, private-store, native Windows/macOS, or packaged acceptance.

## Independent measurements

Both fresh normal/default Boat runs passed **19 boundary cases, 25 checkpoint cases and 32 alternating A/B pairs**, with two excluded warmups per variant. Each burst contains 128 real append/fsync completions. There is no artificial fsync delay.

| Component median, ms | Writer j20261104103008-1718048 | Independent A j20261104103713-1761208 |
|---|---:|---:|
| /hello during burst, n=32/variant | 2454.074 → 0.294 | 76.590 → 0.566 |
| 256 KiB static response, n=32/variant | 2458.741 → 4.932 | 81.221 → 4.657 |
| Modeled append + Disk.sync ACK, n=4096/variant | 16.373 → 16.356 | 0.487 → 0.568 |
| Entire 128-commit burst, n=32/variant | 2452.433 → 2473.958 | 74.543 → 81.960 |

Independent response p99: /hello **122.182 → 2.027 ms**; static **126.513 → 7.787 ms**. At n=32 this nearest-rank p99 is the largest round observation. Modeled ACK p99 was **1.847 → 2.183 ms**, and maximum **4.632 → 20.277 ms**.

The independent run exposes a cost: ACK median increased **0.081 ms (16.6%)**, and burst median increased **7.417 ms (9.95%)**. After was slower in 28 of 32 paired bursts; the median paired difference was 10.196 ms. Retain this tradeoff in the PR. This is a socket-responsiveness win, not faster durability or storage throughput. The two sandboxes had very different native storage timing; do not pool them or publish a universal speedup factor. The underlying storage/host-load cause remains unestablished. The writer job had ended and its VM was deleted before the independent replay; other remote placement/load is unknown.

On both runs, both response types finished during the burst in **0/32 before versus 32/32 after** rounds. Every probe's issuance bracket was inside the actual burst. I recomputed all percentiles and checked raw row counts, clock-domain bounds, hashes, response/file bytes and order, completion counters, resources and cleanup. The 4096 ACK samples are pooled operations within 32 rounds, not independent process trials. RTTs use shared CLOCK_MONOTONIC brackets; the original get helper's redundant process-local result is not persisted or claimed.

## Durability and resume review

The production `store_sync` helper remains byte-identical for synchronous promotion/adoption callers. Only the Disk.sync effect registration uses the new wrapper. Explicit sync-loop mode remains synchronous. A missing async bridge fails before opening; async failures do not fall back to blocking. Async success follows awaited native open, full fsync and finally close. Failure at each stage returns failure. Windows writable-handle policy and POSIX flags are preserved; only Linux Bun 1.3.10 ran natively here, while other-platform flag cases are mocked.

`Sto.committed` still waits for Disk.sync before `Sto.write`/Db.script. Bend planning, SQLite FULL policy, disk-cut behavior and durable output ordering are unchanged. There is no earlier ACK or speculative write. **Hub.loop still awaits its current handler**, so the next queued Hub view/RPC need not benefit from this effect-only change. Other fibers and sockets can progress while the effect is parked. The component uses the exact production park helper body with a minimal continuation adapter; it does not qualify full-hub Bend marshalling.

Strict accounting passed: **4352 parks = 4352 resumes**, 8704 total completions including warmups, zero duplicates and zero pending continuations. Immediate/delayed duplicate negative controls, failure stages, sync modes, internal helper behavior, UTF-8 prefixes and descriptor cleanup passed. PR925's unchanged template and production sender/main scheduler are identical across both variants and both jobs; PR925's candidate scheduler is not included.

## Receipt identities and limits

Independent work: `w-t1793774614-1546807-1585651`; job: `j20261104103713-1761208`; fresh sandbox: `bx_92e3zdjm`, deletion verified. Separate detached checkout: `/home/h/Documents/i2cjak/bp-latency-a-review929-0139fd6d`. Same writer command and pinned day-20261103 runner, no reuse/setup-Bend. No local test/compiler, private fixture, product build/install, merge, or production edit by reviewer.

Actual worker cap: **3,699,998,720 B**; controller: **536,870,912 B**; swap max/current/peak and OOM counters all zero. Shared worker cgroup peak was **195,698,688 B**, set during checkpoint testing; controller peak was **159,698,944 B**. These are not isolated benchmark-child RSS. Total command: 10.29 s; A/B stage: 6.379 s. Bun 1.3.10 executable SHA: `795b37d736a88d746b0e8bb5fcde92bf29bd0f9ac6a1dd1a950fd37440cf5822`.

Both jobs used the same 14-file staging manifest SHA `80a6887a5d8b2809406269304631eef1ab9969f3d22a635ad8d25df1e4582d0d` and public INPUTS SHA `466aa1599802d011765900777d87fb85a3b24e82f7ba90edd9a9bb02d9c72bbd`. Source HEAD/tree are recorded separately from the runner-created snapshot. Independent RESULTS SHA: `ca9a516794fe01f6d424c8b5af8082504b07d8d5ef02e9c5c14baf87110dd456`.

Raw artifacts, logs, provenance, input copy, terminal cleanup receipt and recomputed audit: `A/review-929-independent-pass/`. Writer receipt/hash audit: `A/review-929-writer-pass/`. Earlier failures remain separate and are never relabeled PASS. The coordinator retains merge and next-slice decisions.
