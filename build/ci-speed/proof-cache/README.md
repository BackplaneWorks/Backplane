# Canonical proof cache key retention

`content-cache.py shard_units()` now keys the executable transitive closure of
each generated `PROOF.bend`. Canonical LAWS/PROOF files remain inputs to the
independent whole-partition verification on every plan. Their entire contents
enter a shard key only if that shard's actual imports reach them. This retains
unrelated shard keys after a canonical edit while invalidating changed proofs
and all readers of changed dependencies.

Keys bind content, file modes, logical law/declaration membership, toolchain,
environment, cache logic, partition planner, root-discovery helper, verdict and
guard identities. Numeric scheduling slots are excluded. The new body format
misses old entries automatically. Unknown, missing, linked or unverified inputs
fail closed. Conservative invalidation is retained when a shard imports a full
canonical file transitively.

The 16 dedicated mutation controls include a false theorem with an old
authenticated local pass: only the unchanged shard can hit after regeneration.
They test key invalidation, not theorem execution. Lane 4 independently reviewed
the code and ran these 16 plus the existing 39 cache tests successfully.
The combined cache, closure and canonical-planner suites pass 67/67 in 1.612s;
the saved runner summary is [TESTS.txt](TESTS.txt).

## Measurement

[MEASURE.json](MEASURE.json) records one offline measurement against a full Git
export of `34aa8a0c9a81cf345f05df873dbecdb7492e59f4`, covering 3,165 canonical
laws in 128 partitions. Adding one comment line to an actual proof block changes
128/128 old keys, versus 1/128 new keys. No proofs were executed and no pass was
recorded. These counts measure potential key reuse, not executed cache hits or
CI/proof speed. Replanning/keying after the edit took 2.604s before and 3.108s
after. The complete comparison used 17.11s and 204,628KiB peak RSS (GNU time),
with zero paid compute. One observation each, sequentially on the same host.

Reproduce from this commit's worktree (the source revision is recorded anew):

```sh
PYTHONDONTWRITEBYTECODE=1 /usr/bin/time -v python3 -I \
  build/ci-speed/proof-cache/measure.py --before 34aa8a0c9 \
  --out /tmp/proof-key-measure.json
```

The first attempt omitted imported test modules from its export and failed
before measuring. Its [receipt](FIRST-ATTEMPT.txt) and timing are retained. The
corrected attempt exports the entire tracked tree. It does not run Bend.

## Gate and trust limits

A shard hit establishes only that shard's prior local pass for the exact keyed
inputs. A full gate still requires verification of the complete current
partition and satisfaction of **every** shard by a valid exact-key pass or fresh
execution. Missing shards must fail loudly. The existing all-fresh receipt join
continues to require all receipts; this change does not replace that join with
a mixed cached/fresh gate, wire a workflow, or enable affected-only production
checks. `--all` continues to ignore the cache; nightly wiring remains pending.

Proof receipts remain trusted same-user inputs, and the local cache uses a
same-user MAC. Shared/CI main-seeded trust, sliced-worker integration and actual
capped cold/warm/changed proof timing remain follow-ups. This change neither
qualifies a release nor claims the two-minute PR target has been reached.
