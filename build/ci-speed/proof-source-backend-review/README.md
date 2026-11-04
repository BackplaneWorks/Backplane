# Original canonical shard 000: saved-result review

One approved default Boat experiment checked the original, unsliced 25-law
canonical shard, including `watch_copy_intent_none`. Its checker finished;
the enclosing experiment exited **1** because its trace verifier was wrong.
The original receipts are preserved unchanged in `EVIDENCE.tar.gz`.

The source checker is the exact opt-in PR856 postimage (commit
`fd932a32c3e2b1a305de7141c4beaec219443893`), including protected `bend.ts`,
`safe.ts` and Base identities. It uses check-only result discard, paged
storage, source-body replay and batch128. No D/lazy-prepare composition,
declaration slicing, law modification or weakened guard was used.

| Measurement | Before | This observation |
|---|---|---|
| Original canonical 000 checker | No matched baseline at this exact source/tool/cap | 669.804 s; 3,126,272 KiB GNU peak RSS (3.201 GB decimal) |
| Compiler cgroup | Historical native failures used different inputs/caps | 3,699,998,720 B peak and hard cap; swap 0; 1,675 max events, zero OOM/OOM-kill |
| Controller | No comparison | 471,326,720 B peak; 536,870,912 B hard cap, swap 0, zero max/OOM events |
| Typed false theorem | Required negative control | Rejected at typed body; 0.574 s including scope wrapper, 146,980 KiB peak RSS |
| Paid Boat job | One invocation | 692.33 s; 703 credit-s / $0.00703; sandbox deletion verified |

The two enforced hard caps sum to **4,236,869,632 B < 4 GiB**. Cgroup peak
includes disk-backed spill page cache; it is different from process RSS.
The compiler reached its cap and reclaimed memory; this observation does not
establish spare memory headroom or a speed improvement. The complete proof
suite, PR's two-minute target, hub/web emit, release and shared cache gate
remain **unqualified**. No cache pass was recorded or workflow wired.

## Why the original harness failed

The driver looked for `events:before-check`, which the frozen compiler never
emits. It also expected source spellings `Laws.x`, while imports resolve to
internal `LAWS:x` keys. Thus its collected proof-name set was empty.

The saved trace instead contains a matching `events:check:start`,
`events:validated` and `events:check:end` for the final root batch, events
33827 through 33925. That batch contains all 25 law keys and the unchanged
`dm_append_nil` helper. Frozen `modules.ts` extends batches through the last
event for a repeated key; protected `bend.ts` checks a filled definition's
body at that last event. `book_load` maps `./LAWS.bend as Laws` to namespace
`LAWS`; `parse_def` fills the resolved law. No fallback appears.

The raw compiler and unit command exit **1** with only the repository's
existing accepted notice: **2,574 definitions rely on unsafe or foreign
code**. This is accepted by the existing `bendok.sh` policy, not an
`All terms check` / proof-kernel-only success. The raw systemd result is
`success` because the scope ran to completion; it does not override the
command's nonzero exit. No timeout or OOM occurred. The enclosing packet
still reports exit1 and verifies all 822 source/tool inputs unchanged.

Seven modules are deferred, including the generated LAWS file; their events
are checked in the root's pending tail. Thus there are 368 loaded files but
361 module-check spans. The book reports 34,067 definitions and 33,925 events;
the difference of 142 is unclassified here. Coverage is specifically the
25 selected law fills and helper, not an assertion that every definition has
its own event. The isolated false theorem demonstrates error surfacing on a
small book; it was not inserted deep into the full shard. No stock-checker
run at these exact inputs/caps exists, so stock equivalence is not established.

## Offline reproduction

```
python3 -IB build/ci-speed/proof-source-backend-review/review.py
python3 -IB build/ci-speed/proof-source-backend-review/controls.py
```

These commands only inspect saved evidence. They never invoke Bend, change
the original result, record cache passes or launch paid work. `REVIEW.json`
is the additive audit. Ten controls cover missing validation, the original
wrong phase, missing theorem, wrong namespace/root, skipped prefix,
truncation, fallback, and distinct helper law/fill hashes.

`EVIDENCE.json` indexes raw artifact/source bytes; the bundle includes the
full trace, raw memory samples, execution/cap/time receipts, original failed
summary and harness, packet result/provenance, exact compiler postimages,
canonical originals and emitted shard. The audit compares both declaration
and fill bodies byte-for-byte to canonical originals. The hashes establish
reproducible evidence identities, not shared-cache signatures or independent
attestation of the remote host.

Work `w-t1793650867-1261555-1465396`, job `j20261103235057-263893`, sandbox
`bx_4nh74u77` (deleted). Canonical manifest
`37e080c1c51fcd2505c5ce77e9c8929bbac2fc3d4d4a359d120625b61c821891`;
packet `5572324b21fcda62051d5f64b2cc285391a2cb1e7fa7c358c1465d8e17723c07`.

The five longest module-check spans in the saved trace are server.bend
149.785 s, client.bend 71.299 s, view.bend 61.855 s, hub.bend 46.580 s and
refview.bend 31.120 s. These spans mix checking, paging and GC; they do not
separate parser, proof and emit costs. No emit ran in this experiment.
