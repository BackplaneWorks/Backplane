# Graph format 2: two focused observations

These are existing owner-run experiments, reconciled offline. No compiler,
proof, app, package or paid job was rerun for this report. They do not qualify
a full hub/web build, proof gate, shared cache or release.

| Workload / composition | Earlier observation | Graph2 observation |
|---|---|---|
| Focused `.js`: D versus D + interface sharing + graph2 | 71.297 s /2,841,160 KiB RSS; 1,012,539,236 stored bytes | 56.898 s /2,934,736 KiB RSS; 562,262,792 stored bytes |
| Focused `.mjs`: V21 + lane2 stream/GC/settle/counters + interface sharing; graph2 also changes instrumentation | 140.770 s /3,107,580 KiB RSS; 1,012,512,326 stored bytes | 211.284 s /3,022,820 KiB RSS; 562,250,832 stored bytes |
| `.mjs` large `Card.of` blob | Owner diagnostic: 822,532,769 B, decode ~26.7 s | 436,041,027 B; logged decodes 28.997 s and 40.118 s |

One sample per composition on separate VMs. The `.js` comparison combines
interface sharing with the format change. The `.mjs` compiler identity
differs in **both comp.ts and offload.ts**; instrumented code and VM effects
are not isolated. These are observed values, not a causal speedup/regression
estimate. Compact format cuts saved bytes but does not establish reduced
decoded heap or faster analysis. The large blob is read twice in the saved
`.mjs` log: **69.115 seconds** of decode in this focused pass.

Both outputs match their saved reference digests:

- `.js`: `a7401f2712f024aa9f4b7c2101b454e15c6fba0001eb9f5ec7bafb5419b7097f`
- `.mjs`: `96706cca9467256056bc2fee70b5905ac9547d624eef6fb086452a52200e30a8`

The emitted files, hashes, full profiles, samples, time receipts and packet
source/tool verification are preserved in the indexed evidence bundle.
Canonical source commit/tree is frozen33ce/aa65231 in both owner receipts;
the compiler compositions are different and explicitly identified below.

## Memory, cleanup and cost

| | `.js` graph2 | `.mjs` graph2 |
|---|---:|---:|
| Compiler hard cap, swap0 | 3,699,998,720 B | 3,699,998,720 B |
| Controller hard cap, swap0 | 536,870,912 B | 536,870,912 B |
| Controller peak | 315,842,560 B | 317,779,968 B |
| Sampled compiler cgroup peak | 3,519,389,696 B | 3,655,557,120 B |
| Cap minus sampled total peak | 180,609,024 B | **44,441,600 B (42.4 MiB)** |
| Sampled max/OOM/OOM-kill events | 0/0/0 | 0/0/0 |
| Paid cost | 102 credit-s /$0.00102 | 225 credit-s /$0.00225 |

The actual enforced hard caps sum to **4,236,869,632 B <4 GiB**. Raw receipts
carry 4,236,870,912 B from nominal pre-rounding compiler bytes; that field is
1,280 B high. Raw receipts remain unchanged. The additive summary uses actual
kernel-rounded individual caps. The reported ~0.65 GB anonymous-only gap
excludes file cache and kernel memory; it is not total spare headroom.
Event samples do not establish an exact unsampled end-of-scope count.

Jobs `j20261104000814-389476` and `j20261104001107-414164` completed exit0;
sandboxes `bx_csm3wpcd` and `bx_zz2vhtwg` were deleted, verified in work logs.
Two earlier graph2 submissions failed in preflight; their original cause
receipts and cleanup logs are preserved: 15+24 credit-s /$0.00039.
The earlier `.mjs` contextual job is `j20261103234906-254156`; its cost is
not counted again here. Total cost of the two successful graph2 observations
is **327 credit-s /$0.00327**. This report itself used no paid execution.
Including both failed preflights, these graph2 jobs cost **366 credit-s
/$0.00366**. Neither preflight is counted as a successful measurement.

## Identity and review

The exact lane5 patch `0b0ffaf640e40fd5f3197290cf114cb5e8800678d49429789b95f7eebbf80806`
applies with zero fuzz to interface-sharing offload
`a654aea8db9085b1f4bafdca48eef52fcfb347a998ca01d04a1149bd676c4b51`,
producing `.js` candidate
`bfd57df906b6751d561e3a5cd487acf90ffc2b11128965a2bca9047f616575fa`.
Lane2's `.mjs` confirmation uses composed offload
`600fe090cda9943a2658fc243a43e08cfe16e80c1ddf59548473d7b49d8f432e`.
It is not a run of that exact lane5 postimage.

Read-only review found no new corruption false-green in the changed reader:
it hashes the exact parsed bytes and does not return the root before digest
acceptance. Intermediate span metadata is authenticated, and interface cells
remain lazy. The isolated patch changes offload.ts; protected checker and
law/proof bodies are untouched. Graph2 has **26 passing assertions and one
informational size row**, not27 assertions. Existing integrity, GC boundary,
differential, changed-mode and interface-sharing controls are owner receipts;
this audit did not rerun them.

The `.js` raw comparison fields still point at older V21 timings; this report
uses the recorded D context instead. Such inherited fields are preserved and
explicitly not used for claims. `summarize.py` verifies indexed raw bytes,
emitted-file digests, individual caps, sampled events, compiler differences,
two large-blob reads and cleanup/cost records:

```
python3 -IB build/ci-speed/graph2-review/summarize.py
```

The remaining measured problem is decoded node materialization/analysis.
Authenticated segmented materialization remains the compiler owner's work;
no full web, broad proof or release retry follows from these observations.
