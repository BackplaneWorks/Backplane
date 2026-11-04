# Authentication cache boundary

Base: `5cbf389a8ba80084625f55314e5e6e688cce0df3`.
The real sequence-keyed cache and scanner now live in the 31-line
`src/server/auth-cache.bend`, importing only Base and the original model.
Three cache bodies move unchanged except for qualification of scanner calls;
three scanner bodies are exact copies of still-used core/hub helpers. The
actor uses these real functions for lookup and storage. Model types are unchanged.

Boat's source audit checked all six bodies, every remaining server declaration
after qualification normalization, and every original law statement. The only
added laws are the four below; no original law is removed. The optional audit
uses the strict source reader, not experimental proof slicing.

## Guarantee preservation

Both original laws retain their names, equations and concrete model inputs;
only the module qualification changes. Their proofs explicitly instantiate
stronger branch laws over the actual implementation.

| Original law | Derivation |
|---|---|
| auth_beat_kept | auth_cache_key_sequence gives key `"1 "` for the real empty state. Its prefix matches `"1 1"`, so actual Auth.kept selects the hit branch. Instantiate auth_cache_hit with that string and state; its suffix predicate reduces to True, exactly the original equation. The original proof is this instantiation, with definitional reduction at the call boundary. |
| auth_beat_asked | The empty-state key does not prefix `"7 1"`, so the actual function selects the miss branch. Instantiate auth_cache_miss; Rec.threads of the real empty state is empty, and auth_empty_threads_waits_none gives False. The retained proof instantiates the miss law and reduces the empty scan, proving exactly the original equation. |

auth_cache_hit quantifies over every cached string and real state.
auth_cache_miss equates a miss to the real thread/settings scan for every
string and state. auth_cache_key_sequence binds lookup and storage to the
original sequence calculation. auth_empty_threads_waits_none covers every
state with an empty scanned list. All use concrete checked definitions;
there is no unchecked signature, callback assumption or stand-in.

## Measured scope and memory

Normal Boat job `j20261104035738-696465`, work `1526456`, used ordinary
`scripts/boat-job.sh --size default --idle 0 --timeout 900`, pinned Bend 2.0.36,
and the full canonical 3,191-law/128-shard partition, manifest
`2e84fa48871759fe40dbbf1609f33fe55c5ce8fcd23c7c9f1d7d8d2d63e0d16d`.
It ran only original native shard088 containing all six affected laws, with
no slicer or proof cache. Exit 0: `ALL PROOFS CHECK`. Time 0.661 s, peak RSS
325,804 KiB, cgroup peak 315,412,480 bytes, actual Bend flock held, effective
own cap 3,699,998,720 bytes with no swap. The separate 512 MiB/no-swap
controller peaked at 72,839,168 bytes with zero max/OOM events. RSS and
cgroup peaks use different accounting.

The same job passed the six-body/original-law audit, 35 Python tests,
canonical and frozen merged-law shape, and source patch applicability and
whitespace checks. The entire check process tree had a 512 MiB/no-swap cap.
The checked staged source tree was `130869f6dfd2430d81f811db2bb85d7a6b9ce39d`;
canonical source hashes are fenced in the identity maps. Receipt-only and
measurement-tool additions follow.

Before/after source analysis ran on normal daily Boat job
`j20261104040210-730522`, work `1527575`, via
`/home/h/Documents/i2cjak/bp-coord/bin/boat-job-day-20261103.sh`, supported
full source transport and an own 512 MiB/no-swap scope. Peak 419,557,376 bytes,
zero max/OOM events. The preceding attempt refused unsupported --include-file
before creating a sandbox and performed no measurement.

| Measure | Before | After |
|---|---:|---:|
| Original Auth laws: imported implementation modules | 367 | 12 |
| Original Auth laws: paired lexical declarations | 49 | 51 |
| Original Auth laws: reached modules | 7 | 6 |
| Largest law in the complete measured set | 5,500 | 5,500 |

The two added declarations are explicit composition proof helpers. New component
laws reach 39–44 declarations. METRICS.json and source-hash maps bind both
measurements: lexical closures excluding Base, not compiler elaboration traces.

This qualifies affected laws only, not a full app build, runtime integration,
all 128 shards or residual source coverage. The original capped full-root
baseline remains a failure. RC4 stays frozen under the build/install hold;
the installer confirmed no mid-cut. No local compiler, proof or test ran.

Raw receipts are beside this file. Unpack SNAPSHOT.tar.gz at the repository
root to restore identity-fenced replay inputs before transporting to normal
capped Boat. Archived books never replace the canonical planner or join gate.
