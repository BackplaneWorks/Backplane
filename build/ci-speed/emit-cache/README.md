# Verify raw emission bytes before cache reuse

`build-js.sh` and `local.py` previously accepted an emitted JS file if its
`.js.ok` sidecar held the same byte count. A same-size modification therefore
reused different code under the unchanged source key. The real shell entry
point reproduced this with fake Bend/Bun programs; no actual build ran.

The shared helper now stores a versioned JSON receipt with source key, size
and SHA256. Restore hashes the exact bytes copied into a temporary output,
then replaces the output only after the digest matches. Publish completes
the cache data before replacing its receipt. An interrupted/partial pair
can miss, never pass by size alone. Local warm checks use the same validation;
remote import also checks the expected transfer digest while copying, closing
the gap after its initial hash check. Retained producer admission is unchanged.

| Same fixture / observation | Before | After |
|---|---|---|
| Same-size corrupted cached JS | Copied to build input; fake Bun reached | Cache miss; fake Bend requested, corrupt bytes not copied |
| Local warm-build hint after same-size corruption | true | false |
| Valid cached raw emission | Reused | Reused without Bend |
| Legacy numeric receipt | Reused | Miss; fresh producer required |
| 1,959,238-byte saved output workload, real shell cache branch / fake tools | 0.0208 s | 0.0482 s |

One observation each, same host with fixture directories and fake key/guard/
runtimes. The last row includes Python helper startup; the roughly27ms cost
is not a real compiler/build-time result. The saved `.mjs` bytes came from
job `j20261104001107-414164` and were never executed. Its exact digest is
recorded. Whole benchmark: **0.36s /32,856KiB GNU peak RSS; $0**.
The fixture uses a synthetic source key and temporary cache, never the user's
real cache. `BASE.txt` pins the before source and `MEASURE.json` binds both
build scripts, the helper and all routing results.

## Migration and limits

- **One-time cold miss:** old size-only entries cannot establish their former
  producer bytes and are rejected. Normal memory admission still applies to
  the fresh emit. Older clients can cause another miss if they overwrite the
  receipt with the old format. No memory floors or fallback guards were lowered.
- **CI namespace:** the cache prefix becomes `emit-js-sha256-v1-`; the actual
  emit input key (`emit-js1` domain), retained producer keys and receipts are
  unchanged. No legacy restore-prefix fallback. If an exact v1 Actions entry
  is later corrupt, the job misses safely but cannot replace that immutable
  entry; recovery requires cache eviction or another prefix version.
- **Trust:** the writable local cache/parent and its producer remain trusted.
  Hashes detect corruption/torn copies, not a forged data-and-receipt pair or
  equivalence to Bend(source). This is not a shared signed proof-pass cache.
- **Concurrency:** readers require the known byte digest, not stable LRU
  timestamps. They do not touch receipt timestamps. Fresh publication still
  checks stable source metadata. A concurrent publish may cause a safe miss.
- **Files:** restored raw JS uses0600; stamp/Bun consumers run as the same
  account. Final-component links and nonregular data/receipts are refused;
  adversarial parent-directory swaps are outside the trusted-cache model.
  Normal failures clean their own staging files. Like the old protocol,
  SIGKILL can leave staging files; no age-based deletion of potentially active
  writers was added. These leftovers may be saved with the CI cache directory
  and accumulate across killed runs. Three-entry retention covers final files.

68 focused controls pass, including actual shell routing with fake runtimes,
legacy/malformed receipts, source-key mismatch, changed/truncated/missing
bytes, mutation during copy, a concurrent LRU touch, failed publication,
warm-cache reuse and transfer mutation after the first check. Existing
retained-consumer and heavy-guard controls remain green. Lane4 independently
reviewed and ran35 related controls before the LRU refinement; its findings
were addressed or documented without changing the trust claim.

Reproduce without any heavy build:

```
python3 -IB -m unittest discover -s test/tools -p emit_cache_test.py
python3 -IB build/ci-speed/emit-cache/measure.py
```

No compiler/proof execution, paid VM, live service/settings/auth change,
installation, package or release dispatch. Full low-memory hub/web and
two-minute PR qualification remain open.
