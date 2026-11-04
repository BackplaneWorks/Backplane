# Promotion fixture experiment: helper change dropped

The remaining-guard profile found `store_promote_test.cjs` took13.04s while
the other four remaining commands totaled0.71s. A hypothesis was that batching
schema and metadata setup could help, as it did for the checkpoint fixture.
The difference matters: this fixture already opens through the real production
`db_open`, which sets WAL and synchronous=NORMAL. Its commit/checkpoint costs
therefore differ from the checkpoint test's delete-journal/FULL fixture.

The isolated candidate changed only `open(c,file)` in the promotion test:
retain real `db_open` and `db_exec(schema)`, put the same five metadata rows
and schema inside one real transaction, close the actual slot on setup failure,
return the same live handle on success. Both runtime and Windows owners
reviewed the exact candidate.patch SHA256
`5448e4252383de2a34b5b5c2f806849f660e8bae489854491cc252aa141fe3b2`.
Everything outside this helper, including every build()/marker/PERSIST_WAL/
checkpoint/copy/close/fsync/assertion, was checked byte-identical.

The candidate is **not adopted**. The branch restores the original test bytes;
the final PR contains measurements and documentation only.

## Matched whole-suite results

Job `j20261104070517-376136`, work1556329, one normal Boat VM, pinned Bun1.3.10.
Each execution runs the complete unchanged11-case test program, no native
probe or app. Order alternates; the first pair runs before then after, the
second after then before, the third before then after.

| Pair | Before | After | After minus before | Before / after peak RSS |
|---|---:|---:|---:|---:|
| 1 | 8.34s | 5.98s | -2.36s | 73,556 /71,448KiB |
| 2 | 4.27s | 4.34s | +0.07s | 73,416 /71,344KiB |
| 3 | 4.31s | 4.22s | -0.09s | 73,432 /71,112KiB |
| Median | 4.31s | 4.34s | +0.03s between medians | — |

All66 original case executions passed, but timing is mixed. The first pair
has substantial warm/cold variation. There is no demonstrated useful
whole-suite speedup. The earlier13.04s is from another VM and must not be
compared with this candidate as a matched speedup.

Five alternating open-helper-only pairs (state inspection and close excluded):
before milliseconds108.710234,117.951818,99.258533,116.878929,102.183634;
after98.338489,112.230814,96.997021,165.997847,98.353514.
Medians108.710234ms versus98.353514ms, ranges99.258533–117.951818ms versus
96.997021–165.997847ms. These overlapping observations do not establish that
this helper accounts for the original13-second command's cost. No phase-level
attribution of that earlier command is claimed.

No numerical adoption threshold was registered in the frozen packet. A review
request to add one arrived after launch; neither the packet nor its RESULT.json
was changed. DECISION.json is explicitly a post-run interpretation. Correctness
qualification is separate from this decision; a passing scoped correctness
receipt does not establish a performance win or qualify full CI.

## Real SQLite controls

All controls in the one owned packet passed:

- Exact schema definitions and all table rows, including FTS shadow tables,
  match in five setup pairs. Real effect live PRAGMAs match, including WAL,
  synchronous=NORMAL, busy_timeout5000, page_size4096, user_version0,
  wal_autocheckpoint1000 and integrity_check=ok.
- All four original build modes match logical state, PRAGMAs, sidecar names
  and required WAL-empty behavior: persistent, empty-wal, wal-only and
  no-sidecars. Persistent WAL length legitimately changes486,192→238,992B;
  physical byte equality is not asserted. Other observed WAL sizes are0B,
  8,272B, and absent respectively. Every observed DB is229,376B; sizes are
  information, not the logical-state proof.
- An actual schema error produces ERR_ASSERTION containing
  `fail no such table: __missing_fixture_table`; an actual metadata trigger
  produces SQLiteError/SQLITE_CONSTRAINT_TRIGGER `fixture meta failure`.
  Both leave no partial tables after rollback, release the actual db slot,
  and close the actual handle, checked by a closed-handle error and reopen.
- Both unchanged original11-case suites detect the deliberately removed
  staged-sidecar sync in the real production-effect copy. Each fails the
  specific `WAL is durable before any current store is moved` case plus six
  copy/sync assertions. An unrelated failure does not count as detection.

All original runtime files and accepted sources are immutable. Only a separate
failure-injection copy is altered. No production store, durability mode,
Windows branch, source-review pin, law or proof changes. No host test runs.

## Identity, bounds and cost

Candidate source7e1d5ddbf80729ae14f94887076c09688b005c51, tree
10a565d6bea7195565106c6a4d7ce58c862832e0; reference0c63ed2629e3d888ae56766e21de606d9f1a651a.
Six-file source/transport hash+mode closures are in STARTED.json. Candidate
test SHA256e86eb84fb09cf6c48e40d7277b55e8e30693b44917f89436726f1eecfeabc699;
reference193c3a453ee3a0e105bef7fd58d7a20629223ddfc830aa440e14130e830a494b.
Runtime storeio.js remainsfc27b7c1ef83da3ad3b4ed4eec12237f7e776bf2582ffd20dc2d39b7acbb5f93.
Frozen Python harness e18e1f850eff5e6444dbf6b8b977795692719668f5a79280199cc3317cf9d55a;
native bench58218abf7a31b8cef6b1532b273cfd8ab73bb14b5f9c44546c3bdaf436a91cf8.
Final source/reference/harness/bench/Bun/Node identity fences all pass.

Actual worker cap3,699,998,720B, peak473,645,056B; controller536,870,912B,
peak10,399,744B. Combined caps4,236,870,912B <4GiB, both swap0, native bend
flock held and independently probed, all max/OOM counters zero. No Bend
installed or executed. Approved SHA-pinned normal daily runner only.

Whole remote command47.42s,53credit-s/$0.00053. Sandbox bx_vumm6wck deletion
verified. Together with the five-guard profile38credit-s/$0.00038, this
investigation cost91credit-s/$0.00091 before the eventual docs merge guard.
Full two-minute PR checks and low-memory complete CI/build gates remain open.
