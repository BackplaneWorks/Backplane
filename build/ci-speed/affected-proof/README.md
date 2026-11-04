# Original proof cache execution

Opt-in local cache tooling, not a production/full-suite gate or a shared cache.
`proof-cache.py` plans a verified original canonical partition, executes one
missing shard with the existing stock-Bend worker, and joins only a complete
current set of authenticated passes. No declaration slicing or replacement
checker. `scripts/proof-shards.py` is unchanged.

A plan binds the complete manifest/key inventory. Numeric shard slots are not
semantic identity: exact generated proof/dependency bytes, logical membership,
portable modes, compiler/Base/named-library bytes, environment, verdict and
executor code are. A changed proof invalidates its shard; unrelated shards can
keep their keys. Before/after snapshot validation fences lookups and execution.

Only the admitted process that calls the original worker records its pass,
after checking the verdict, its own executor PID, current inputs and resources.
`content-cache.py record-proof` now refuses caller-declared JSON, and proof keys
invalidate legacy receipt-import entries. The original raw worker receipt and
exclusive attempt marker remain authoritative and are never rewritten.

## Use

Generate a new partition and plan when sources change (never overwrite an
existing attempt). All paths below refer to the repository being checked;
`--root` may select another tree, but keys still bind all inputs/tool bytes.

```sh
python3 scripts/proof-shards.py generate --out build/proofs --shards 128
python3 scripts/proof-cache.py plan --partition build/proofs --plan build/proof-plan.json
```

For each ID in the reported `miss` list, on an admitted normal-size machine:

```sh
systemd-run --user --scope --quiet -p MemoryMax=3700000000 -p MemorySwapMax=0 --   python3 scripts/heavy-guard.py proof-shard --   python3 scripts/proof-cache.py run --plan build/proof-plan.json     --shard 000 --result build/proofs/receipts/000.json
python3 scripts/proof-cache.py join --plan build/proof-plan.json
```

The guard requires its actual native bend flock, admission and real <=3.7 GB
no-swap cgroup. Reserve admission remains mandatory on shared hosts. Select
one shard per isolated worker and budget any controller separately; do not
launch a host fanout with this example. Guard only accepts the exact executor
and unique known options; arbitrary checker argv and `--held` are refused.
A cached `run` also requires its admitted capped scope. `plan` and `join` are
metadata operations; a miss, missing/forged MAC or partial inventory cannot
join. Stale or malformed plans fail and must be regenerated. Inspect an
existing attempt instead of restarting it after an observation timeout.

The cache MAC is a same-user trust boundary. It is not producer attestation;
anyone reading the private key can forge entries. Never export that key or
restore untrusted PR cache entries. Cross-PR/main/Boat reuse needs trusted
asymmetric signatures and worker provenance first. The full nightly run must
still ignore cached passes; this API has no force-fresh nightly mode.

## Measurement

[MEASURE.json](MEASURE.json) and the indexed [EVIDENCE.tar.gz](EVIDENCE.tar.gz)
preserve one actual stock Bend 2.0.36 default Boat run, its exact source packet,
raw canonical templates/partitions, worker/attempt receipts and all refusal
logs. No fake compiler, app build, full repository proof or cache pass import.
Fixture: two typed laws with original `{==}` proofs, one per shard.

| Same fixture | Cold before | Cached after |
|---|---:|---:|
| Proof executions | 2 | 0 warm; 1 after one proof comment edit |
| Sequential worker scopes + final join (plan/diagnostic joins excluded) | 1.288 s | 0.270 s warm; 0.788 s edited |
| Peak worker RSS | 120,448 KiB | 118,784 KiB edited (no worker warm) |

Cold planning separately took 0.272 s. Raw Bend invocations were ~0.08 s each;
the scope, key validation and MAC handling dominate this small fixture. These
are single observations, not production PR timings or a claimed 2-minute gate.
False theorem: original Bend exit1 at the proof body, no cache entry, join
incomplete. Its peak123,136 KiB is the experiment maximum. Stale source/plan,
dropped key, partial join and tampered MAC controls refused. Restoring only the
exact previously authenticated entry reused its completed gate without rerun.

Work `w-t1793650867-1261555-1486773`, job `j20261104012254-1001769`; 4-core
default Boat, source push2s/setup1s, whole remote experiment5.979s. Each fresh
worker scope had actual memory.max3,699,998,720 B and swap.max0; controller
536,870,912 B / swap0 (peak37,904,384 B). Combined hard caps4,236,869,632 B
<4 GiB; zero max/OOM/swap activity in all four workers. Raw summary uses the
nominal 3,700,000,000 B request, 1280 B above the kernel-rounded limit; raw
receipt is preserved. VM had provisioned swap, but both actual scopes disabled
its use and kernel swap I/O stayed zero. Sandbox deleted verified;
12credit-s/$0.00012 total. Private MAC cache stayed in /tmp and was deleted;
its key and entries are not in artifacts.

[TESTS.json](TESTS.json):126 metadata/policy/integrity controls,2.904s. Unit
fixtures use toy identities for cache/key controls; they are not proof passes.
Lane4 independently reviewed the executor/guard and caught the snapshot fence
fixed before the real Boat run. These receipts do not seed a production cache.

No workflow/check.sh/local-ci wiring changes: existing heavy canonical shards
still need the compiler-memory lane, and shared trust remains unimplemented.
This supplies the missing executable local cache path without promoting those
gates or replacing full-nightly obligations.
