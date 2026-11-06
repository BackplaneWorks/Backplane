# Hub memory qualification

The 2026-11-03 incident is still open. Focused replay, image-path and JS checks
passed. Packaged acceptance of the final queue-boundary and image changes is
pending; the installer owns the single combined build. Full root proof was canceled before execution
and explicitly omitted under the user's later instruction; no replacement is
planned. The live hub has not been restarted, signalled, forced to collect,
or given synthetic work.

## Read-only observation

PID 3103745's executable SHA256 was
`ec0a8f82620b3c7634282d077f7a2b15c64c7415daebb04c45a29291111115da`.
Its `/hello` identified `0.13.1-dev.local.2f9d67d0a.33981f2e`; the reported
454ea658d identity was not confirmed on that PID. RSS varied between snapshots
(about 7.8–8.9 GiB), with an 11.6 GiB high-water mark. Almost all resident bytes
were private anonymous mappings; file-backed mappings accounted for about
24 MiB. The sampled process had no swap. This identifies heap/native allocation,
but does not distinguish live objects from allocator high-water retention.

The store had approximately 1.372 million applied changes, 406,241 indexed
entries, 10,333 threads, 229,599 settings and 13,925 mail rows. Its append-only
log occupied about 1.27 GB. Only counts were queried; private payloads were not
printed or uploaded. Four clients and ten agents were reported. Descriptor
inspection found about 47 unique sockets, with no queued TCP bytes at that
instant; it does not establish a historical socket bound.

There was no listening inspector. Attaching by signal or requesting a full
snapshot could disturb this large, busy process, so neither was attempted.
An owned synthetic hub is the heap-snapshot target instead.

## Retention paths

The serialized public wire history has an explicit root in `Hub.ls`: hot boot
reads the complete log, and each commit prepends another serialized string.
Broadcast and replay-credit closures can retain that same historical list.
These are references to shared objects, not evidence of three independent
copies. This root grows with events even when the canonical setting map has
only one overwritten key. The candidate replaces historical payload ownership
with counts and bounded indexed disk pages; canonical append-only data stays
on disk. A stream scanner validates the log without allocating it whole.

The owned baseline snapshot confirms a whole-history root in `Hub.lines`:
53,898 serialized event strings, with 56,925,413 snapshot-reported bytes in
heads plus 2,587,104 in list nodes (about 56.8 MiB). Several hub references
point at the exact same list target; these are aliases, not independent
copies. A client's `cr.rest` also points to a 25,424-row tail (28,496,388 head
bytes plus 1,220,352 list-node bytes), sharing historical strings. Do not add
these sizes to infer independent retained memory.

The snapshot has 582,072 nodes and 2,081,285 edges, with 136.0 MiB aggregate
reported shallow size; strings account for 62.4 MiB. The largest sampled
canonical entry chain has 219 rows. This is an owned synthetic 50,000-overwrite
fixture, not the live process. Snapshot-reported shallow sizes do not establish
exclusive retained size, all native allocations or allocator resident slack.

The raw log's disk size is not an explanation of the entire RSS. JS strings,
list nodes, parsed canonical records, secondary indexes, old retained roots,
transient serialization, and allocator slack have different costs. An owned
same-key workload and retained heap roots must establish bytes/event before
attributing all observed memory to one path.

`State.entries` and its secondary indexes are another growing resident root.
Pruning those requires preserving unread input, delivered reports, provider
recovery, design/terminal facts and subagent lifecycle state. The experimental canonical resident eviction hook was deferred after
review found provider-position, report-body, reopening and subagent identity
regressions. Durable cursors and hydration must be established first. A timeline-size assertion alone is insufficient.

Other source bounds already present include three delta bodies per view,
128 issued bodies, 512 KiB signal/text caches, an 8 MiB output-credit limit,
and fixed performance slots. Image/render artifacts are disk-backed and KiCad
runs in a separate helper. These source bounds do not measure their exact
live cardinality or retained bytes. Boat/allowance/client cleanup still need
runtime corroboration; the installed build exposes no complete heap inventory.

## Current candidate results

The corrected pre-queue-fix package SHA256 is
`f3db5d83f7910852047670b71be817dd1a0239e8ba4a538e6b084e22fb9b2a60`.
Its owned short diagnostic recorded RSS 483,835,904→529,793,024 bytes;
the baseline recorded 547,803,136→679,972,864 bytes. These runs use an
inspector and diagnostic collection; they are not RSS-slope or CPU acceptance.

The separate strict 20×40 Hz run failed: mixed CPU 117.33%, live-only
100.60%, overall 111.75% against the unchanged 80% target. Sampling later
identified full web rendering, message-image tokenization, SQLite materializing
and framing work. The image negative-scan guard is drafted and its six path
parity cases pass; it has not yet been measured in a rebuilt package.

The intended 100-minute soak stopped after 161.57 measured seconds following
warmup. It passed 15 health checkpoints before one p99 histogram upper bound
of 2,047 ms exceeded the 2,000 ms target. The coarse bucket does not reveal
whether the actual value exceeded 2,000 ms. Preserve this as FAIL, not PASS.
It captured one memory point: RSS 444,308 KiB, heap 104,361,649 bytes;
peak high-water RSS 525,160 KiB, swap zero. There are insufficient memory
samples to compute or accept a slope. No hours-long result is claimed.

The later harness correction splits its bounded histogram at exactly 2,000 ms
and saves best-effort process memory before shutting down a failed fixture.
Its negative controls cover fractional values and nearest-rank p99 at small
sample counts. This correction does not reclassify the prior failed soak.

The final disk-full boundary uses the durable public-wire prefix followed by
the queued public suffix. Reads never ask SQLite for rows not yet persisted;
successful queue flush drops the temporary history root. Seven pure cursor
checks, eight file-backed replay checks and six image-path parity checks passed
in Boat job `j20261103174003-1135763` (375,188 KiB peak RSS). No blocking finding
remained in the independent narrow queue review. Pagination over 1,024 queued
rows and flush during partial replay remain additional integration coverage.

The targeted `wire_history_soak.cjs` uses the exact production scan and replay
effects against 1.37 million generated same-key events, with a bounded writer,
SQLite index and replay cursor. It isolates the wire-history root while holding
canonical key cardinality constant. It measures heap and RSS without collection
and checks slope/endpoint growth during 30 minutes of continued activity.
It is explicitly a module soak: it does not qualify full-hub caches, transport,
CPU or latency. Its receipt is separate from packaged-hub acceptance.

## Gates

`test/tools/hub_memory_soak.ts` creates its own temporary home, loopback child
hub, fake agents, token and synthetic catalog. It accepts no live URL, private
home, imported event log or arbitrary inspector endpoint. It never forces GC.
Its controller retains bounded counters; memory samples stream to JSONL.

The default measured duration is two hours after five minutes of warmup,
with 20 agents, 1,000 catalog threads, committed message/tool cycles, view
changes and reconnects. RSS slope and endpoint growth must both remain within
10% of warm baseline per hour, RSS/HWM below 1 GiB, swap zero, and continuous
production/delivery/latency checks must pass. Missing heap gauges, samples or
workload invalidate the result. Short runs are explicitly labelled `short-ci`
and cannot establish a two-hour result. Pure self-tests exercise failure
controls without starting a hub.

Nightly CI runs this gate against its existing Linux package and retains the
report/samples. Manual CI can enable `memory_soak`. The ahead-validation cold
lane accepts an existing package and its expected SHA256:

```
scripts/ahead-validation/memory.sh /absolute/backplane-serve EXPECTED_SHA256
```

`AHEAD_MEMORY_SOAK_BIN` and `AHEAD_MEMORY_SOAK_SHA256` when invoking
`scripts/ahead-validation/ci.sh` to include it there. Without those inputs the
script explicitly reports the memory gate as not run. UI/source proof receipts
never stand in for this runtime result. Package commit/tree provenance must
also be retained by the build owner; a matching binary hash alone does not
prove which unbuilt source changes it contains.

Heap-growth diagnostics use a bounded, numeric, minute-sampled low-water
window and the existing private diagnostics controls. They record a local
`heap_growth` code after sustained RSS and JS/native growth. They do not
capture heap contents, collect garbage, enable diagnostics, or send telemetry.
