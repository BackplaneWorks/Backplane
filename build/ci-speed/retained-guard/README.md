# Bind retained-compile admission to its real entry point

The `compile-js` profile admits at an expected3GiB peak, compared with16GiB
for a whole build. Previously the generic guard accepted any command using
that profile. This let a misclassified whole-program command receive the
retained-only admission floor. No claim is made that a real workload used
this route; the before/after controls mock all exec and memory operations.

The profile now accepts only this checkout's exact `scripts/build-js.sh SRC
OUT` command with all three retained-emission values present and correctly
shaped. Its `--held` check is bound to that same command. The shell helper
forwards the actual entry point and arguments with quoting preserved.

`build-js.sh` still validates the retained bytes and import/tool key before
admission and again when copying under the native bend lock. The guard's
routing check does not attest a producer, validate the retained path or
replace those checks. A missing or corrupt retained input cannot select a
Bend fallback. Extra retained-mode arguments now refuse with exit2; no
current caller uses them. Existing profile floors,8GiB host reserve, actual
flock behavior, shard command contracts and source-only offload refusals
are unchanged.

| Mocked routing fixture | Before | After |
|---|---:|---:|
| Unrelated whole-proof, Bun, foreign-entry and partial-retained commands reaching mock exec | 4/4 | 0/4; exit2 before admission/exec |
| Exact retained entry point with complete environment | Reaches mock exec | Still reaches mock exec |

Reproduce with `python3 -IB build/ci-speed/retained-guard/measure.py`.
`BASE.txt` pins the before source; `MEASURE.json` records source hashes,
individual routing outcomes/messages and timings. `MEASURE.time` is whole
fixture GNU time. It is a small routing measurement, not a real compiler,
startup or release benchmark. No paid work or heavy job ran.

Five new controls cover direct/held arbitrary command refusal, missing/bad
retained fields, valid40/64-character keys, memory refusal and lock miss,
interpreter/foreign-script/extra-argument refusal, and shell quoting.
The27 existing heavy-guard and13 retained-consumer controls also pass.
`TESTS.json` is the first test-run summary, not a raw stderr transcript.
Lane4 independently reviewed and ran the same45 controls; no blockers.

The patch does not make a Backplane named lock into a native flock or govern
commands that bypass the guarded entry points. Heavy experiments still need
the actual native admission/caps/reserve/sole-slot protocol or a reviewed
normal Boat packet. Full low-memory hub/web builds and release qualification
remain open. No compiler, law/proof, installer release file or workflow changes.
