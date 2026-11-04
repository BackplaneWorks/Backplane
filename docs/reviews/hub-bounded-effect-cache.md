# Bounded effect metadata caches

Browser casting previously pruned its three-second client sightings but never
removed page keys from its lease map. Visiting new pages could grow that map
for the lifetime of the process.

`cacheio.bend` supplies typed browser-seen/browser-lease slots and their one
shared policy: at most 4,096 entries and 512 KiB of charged storage per slot,
with a 3,001 ms age (the existing inclusive three-second endpoint). A shared
8 MiB ceiling also limits combined admission. Keys and values are owned UTF-16
strings; model objects, event history and credentials cannot enter these slots.
Charged storage includes both strings and fixed node/index overhead; it is a
cache accounting bound, not a measured process RSS bound.

`effects/bounded_cache.js` owns LRU, age, accounting and counters. Hits promote
LRU without extending age. Writes replace their age index node, so repeated
updates cannot accumulate expiry records. Expired entries miss and are removed
on cache work or metrics reads. Pressure evicts the least recently used entry.
Rejected replacements remove a stale old value. Unregistered numeric slots
cannot create caches. Metrics contain only fixed names and numeric counts.

`castio.bend` initializes that effect before the existing casting operations.
`Cast.drop` removes a disconnected client's sightings. The ordinary leave and
oversized-request disconnect paths call it; page leases expire under the same
bounded policy. Stable target order and the one-second helper lease cadence
are preserved within the cache budgets. Under pressure, evicted sightings fall
back to the existing lower-rate broadcast frames rather than retaining an
unbounded subscription history.

Three new canonical laws fix the slot numbers and the exact budgets Bend
hands the effect. Existing laws and proof bodies are unchanged. Foreign-code
boundary checks separately cover LRU, expiry, caps, stale replacement, raw
Unicode units, disconnects, original cast traces and 60,000 churn/overwrite
operations. Eight cache checks pass under Node and Bun; three performance
counter checks pass under Node.

Scoped Bend qualification passed at source head `1ba75edef` in normal Boat
job `j20261104014800-1235111`. `test/tools/cache_effect_scope.py`
copies the exact three new canonical law/proof blocks and unchanged small
cache/cast/type-test modules into an isolated packet. It uses the original
shard executor under a 3.7 GB no-swap worker and separate 512 MiB no-swap
controller, with a changed-budget false theorem control that was rejected.
Positive compiler RSS peaked at 114,816 KiB; the negative control peaked at
119,616 KiB. The controller peaked at 33,464,320 bytes. All recorded OOM and
swap activity counters were zero. A prior SMALL admission refusal
(`j20261104014641-1218692`) ran no Bend and remains a separate incomplete
attempt; the successful run kept the same worker/controller caps.

[Receipts](../../extra/hub-cache-qualification/SUMMARY.json) record complete
source/block hashes and the pushed source head. Its interface type-check does
not execute the IO test's main or qualify the server/application closure.

No full/root proof, full application compilation, packaged hub or performance
acceptance is claimed. The server's two cleanup hooks remain a source-only
integration audit until the admitted application/compiler path is qualified.
This is the first consumer of the common cache mechanism; existing sidebar
projection caches remain with their memory-owner implementation.
