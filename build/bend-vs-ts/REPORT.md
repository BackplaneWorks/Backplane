# Bend versus TypeScript — measured 2026-11-04 UTC

**Bend offers provable policy and deterministic shared logic; its current JS representation and whole-program compiler impose substantial costs. This supports a hybrid hub, not a wholesale rewrite.**

Source `a4a14b3728237aeb669cf9e74588fa154d410c7f`, Bend 2.0.36, Bun 1.3.10, Ryzen 9700X Boat VMs. Runtime: 4 vCPU / 8 GiB; three fresh processes, four warmups, medians, JIT on. Snapshot: exactly 1,450,000 events, SHA-256 `fd3330c645e5a96f5241425125568d5108e05adee5c15cd226d5e2db2bdb47b9`. Only anonymized projections were uploaded; no live hub ran.

All paired outputs matched: every broadcast routing/pace field; status names/counts; history IDs, bounds, bodies and every renderer text block. Bend is the actual hub emit after today's native replacements, with its bootstrap removed. TypeScript uses ordinary Maps/arrays. Parsing/setup/conversion are outside timing.

| Path / input | Wall ms, Bend / TS | Ratio | Peak RSS MiB, B / TS | Allocated MiB, B / TS* | GC pause ms, B / TS |
|---|---:|---:|---:|---:|---:|
| Status, 500 threads | 3.255 / 0.234 | 13.9× | 214 / 85 | 7.55 / 0.37 | 0.349 / 0.000 |
| Status, 1,000 threads | 4.922 / 0.335 | 14.7× | 221 / 92 | 10.96 / 0.51 | 0.517 / 0.000 |
| Broadcast, 1.45M events × 4 views | 7091.440 / 798.076 | 8.9× | 1611 / 1058 | 5235.95 / 1072.50 | 20.290 / 1.329 |
| Broadcast, 2.90M events × 4 views | 9061.701 / 1244.070 | 7.3× | 1968 / 1285 | 10569.80 / 2144.90 | 12.464 / 1.540 |
| History query, 420,004 entries | 2.953 / 0.519 | 5.7× | 220 / 98 | 4.82 / 0.99 | 0.148 / 0.015 |
| History query, 840,008 entries | 3.711 / 0.714 | 5.2× | 221 / 101 | 6.22 / 1.29 | 0.172 / 0.014 |

*Allocation is a JSC-managed lower-bound estimate (`ca` plus concurrent `a`); native SQLite/malloc is not exhaustively counted. GC excludes the final forced collection. RSS includes the whole process, fixture parsing, JIT and output buffers; it is not retained hub-state size. History is measured in 16-query batches then divided by 16. Zero GC means no observed in-window pause.*

**Attribution.** With the index already built, 1,000 phases took **0.866 ms in Bend, 0.936 ms in TypeScript using the same Bend tries, and 0.087 ms using native Maps**. This points to representation/library costs, not a universal 15× generated-control-flow penalty. Status index construction also performs more intermediate grouping/relationship work; its full 14.7× gap cannot be assigned solely to codegen. Keeping Bend but using its old scan path costs **16.91 versus 4.92 ms**: an independently measured **3.4× algorithm difference**. Removing today's native patches costs **140.85 versus 4.92 ms** (28.6×); the raw emitter is much worse than what ships.

Broadcast uses the same linear touch/pace algorithm: its remaining gap combines strict evaluation, string/list intermediates and generated calls. History uses identical indexed SQL: TS without renderer blocks takes **0.252 ms/query**, with them **0.519 ms**, versus Bend's **2.953 ms**. The remaining cost includes building blocks and a temporary persistent index, not a full-log scan.

**Scaling.** Doubling threads took **1.51× / 1.43×** (Bend/TS); doubling history **1.26× / 1.38×**. Broadcast's measured doubling was **1.28× / 1.56×**, but host noise was substantial (Bend 1.45M: **6.41–7.98 s**; 2.90M: **9.05–15.51 s**). An earlier independent run doubled **2.22× / 2.07×**. There is no evidence here of an O(n²) hot path; precise scaling needs quieter hosts. Neither an invariant language multiplier nor a 100× algorithm claim is justified.

| Compile phase (single run) | Wall s | Peak RSS MiB |
|---|---:|---:|
| Whole hub Bend emit (8 vCPU / 16 GiB) | 120.236 | 14801.6 |
| Bun executable link (4 vCPU / 8 GiB) | 0.715 | 351.1 |
| tsc --noEmit, equivalents only | 0.344 | 226.1 |
| tsc emit, equivalents only | 0.203 | 157.0 |
| Bun transpile, equivalents only | 0.004 | 20.6 |

The emit alone peaks at **14.45 GiB**; the expensive stage is Bend, not Bun linking. **The TS comparison covers only the equivalents; Bend checks/emits the whole hub import closure and proofs. These are rough references, not an equivalent full-app build speedup.**

**Recommendation.** Keep Bend for small pure policies, protocol decisions, geometry and laws worth proving. Move hub I/O, mutable read-model indexes, history materialization and high-volume routing mechanics to TypeScript. Pass small inputs/results across that boundary; avoid walking whole states through Nat marshalling. Keep event-transition policy in Bend until replay is measured separately. This test measures three kernels, not full replay, UI rendering, network throughput or the economic value of proofs; their absolute hot-path times alone do not explain multi-second UI stalls.

Method/reproduction: [harness](../../../test/perf/bend-vs-ts/README.md). Raw samples and provenance: [RESULTS.json](RESULTS.json). The real snapshot is mostly settled and text is anonymized to ASCII of matching code-point lengths; status is a metadata projection, not the full bot/mail/settings state.

Validation: paired runtime/digest checks passed; TypeScript strict check/emit passed; phase/Unicode branch checks and 15 laws-shape tests passed. Repository-wide checking hits the pre-existing `test/stream_step_proof.bend:30` `fallback_outs` proof mismatch. Full `PROOF.bend` also fails in existing `MailProof.keeps` (336.9 s, 13.53 GiB RSS). No production code or laws changed. Validation alone used 32 GiB after the memory guard refused 16 GiB; benchmark numbers above are from 8/16 GiB VMs.
