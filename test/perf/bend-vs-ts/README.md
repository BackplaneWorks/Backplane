# Bend versus TypeScript

An offline benchmark of three real hub paths. It never starts a hub. Production
sources are untouched. `expose.py` removes exactly one emitted bootstrap and
exposes existing functions; unexpected emitter shapes fail closed. It does not
use exported Nat marshalling, which the internal hub does not pay.

The report and raw results belong in `build/bend-vs-ts/`. Record the source
revision and Boat provenance alongside them. The cold whole-hub emission uses
the repository's pinned compiler and the same `bun build` / `js-native.py
--server` processing as `scripts/build-js.sh`. `bend-raw` deliberately omits
those native primitive replacements; it is a diagnostic, not the shipped hub.

1. Copy a fixed prefix of the real store **locally**, read-only:

   ```sh
   python3 test/perf/bend-vs-ts/snapshot.py ~/.backplane-bend/events.jsonl build/bend-vs-ts
   python3 test/perf/bend-vs-ts/fixture.py build/bend-vs-ts build/bend-vs-ts/fixture
   ```

   `private-events.jsonl` stays local in ignored `build/`. Never upload or commit
   it. The anonymized projection retains event order/tags, relationships,
   timestamps, entry lengths, roles, and relevant metadata cardinalities. Text
   becomes ASCII of the same code-point length. This is not full state replay.
   Status uses the most recently touched 500/1,000 threads, all asks/tasks/queues,
   and work settings; other settings, bot/mail state and transcripts are omitted.
   Broadcast uses every event's routing fields (legacy `CtxNoted` is ignored as
   by today's codec). History uses every real timeline insertion, including
   cross-thread links, with the same SQL and indexes. Doubling appends a copy
   with distinct sequences/ordinals; user message IDs may legitimately repeat.

2. Run `compile.sh` in a disposable Boat sandbox and retrieve its artifacts.
   It records cold emit/post-processing separately. Keep the untouched
   `hub-native.js` too: the final Bun executable is built but never executed.

3. Stage only anonymized inputs and emitted modules under
   `test/perf/bend-vs-ts/input/`: `events.jsonl.gz`, `entries.jsonl.gz`,
   `metadata.json`, `manifest.json`, `hub-native.mjs`, `hub-raw.mjs`, and
   `hub-original.js` (the untouched `hub-native.js`). `input/` is intentionally
   untracked, not ignored, so `boat-job.sh` can inventory/upload it. Never
   include this folder in a commit. Delete it after retrieving the results.

4. Run via Backplane `work_run`:

   ```sh
   scripts/boat-job.sh --size default --timeout 1500 --idle 0 --no-reuse \
     --no-setup-bend --artifact build/bend-vs-ts/runtime \
     -- sh test/perf/bend-vs-ts/runtime.sh
   ```

   `runtime.sh` pins Bun/Node through the repository's tool installer and
   installs TypeScript 5.9.3. Runs are serial, JIT enabled, three fresh processes
   per case, alternating implementation order, four warmups. Parsing, metadata
   conversion, database creation, forced GC, canonicalization and hashing are
   outside the timed region. History is 16 queries per batch (four longest and
   four median threads, newest and middle pages); reported per-query values
   divide the batch by 16. Outputs include IDs, counts, bodies and **all** text
   blocks. Every event's touch/level/pace fields are compared, not just a sum;
   timed output digests must also agree across implementations and trials.

5. Copy retrieved `runtime/` into `build/bend-vs-ts/runtime/` and the original
   emit metrics into `build/bend-vs-ts/emit.json`, then run
   `python3 test/perf/bend-vs-ts/report.py`. Keep the final report beside its
   `RESULTS.json`; never include the private snapshot or staged inputs in a PR.

`status` includes building `Side.ix` and all phases. `phase` excludes index setup;
`ts-trie` uses the actual immutable Bend tries with ordinary TypeScript control
flow. `bend-scan` uses the existing unindexed `Side.phase` as an algorithm
control, not as today's production path. `ts-query` deliberately skips text
blocks to measure their share of history work; its final canonicalization
builds them outside the timed region for output comparison.

Peak RSS is the entire fresh process (`wait4`), including inputs, setup, JIT and
output buffers, not the retained hub model alone. GC pauses sum JSC's logged
`p=` intervals strictly within the timed region. Allocation is an approximate
lower bound from `ca=` plus the maximum concurrent `a=` per completed cycle;
a forced collection outside timing closes the last allocation interval. It
does not count every native SQLite/malloc allocation. Tiny late allocations
and log rounding remain unaccounted. See the pinned Bun JSC
[scheduler](https://github.com/oven-sh/WebKit/blob/4a6a32c32c11ffb9f5a94c310b10f50130bfe6de/Source/JavaScriptCore/heap/StochasticSpaceTimeMutatorScheduler.cpp)
and [pause accounting](https://github.com/oven-sh/WebKit/blob/4a6a32c32c11ffb9f5a94c310b10f50130bfe6de/Source/JavaScriptCore/heap/Heap.cpp).

The TypeScript compiler numbers cover only these equivalents, while Bend's
emit covers the entire hub/import closure and checks proofs. They are a rough
reference, not an apples-to-apples compiler speedup or a full-app migration
estimate. No UI, network throughput, full replay/startup, or proof productivity
benefit is measured here.
