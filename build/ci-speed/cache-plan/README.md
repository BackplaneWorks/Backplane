# Validate cached plans at consumption

Previously, `run-check` returned success immediately for an empty miss list.
A saved plan could therefore pass after a transitive input changed, a module
was added, or a cached pass disappeared. Mixed plans re-keyed only their misses,
so a hit changed while another module ran was ignored. The shell also returned
success directly on all-hit; it now invokes the same validator first.

Plans now record schema 1, the complete module list, keys and forced misses.
Old/partial/malformed plans fail closed and must be regenerated. Every success
path re-reads the tool identity, complete Git module inventory, all current keys
and authenticated entries for all claimed hits. Miss execution retains its
existing heavy guard, admission and flock requirements, and performs the same
full validation again before any recording or success. A deleted/corrupt hit
entry requires a new plan; it is never silently trusted or run without admission.
`--all` can force extra work, and uncacheable units cannot become hits.

The recorded pass remains attached to the exact validated content key. A later
edit changes that key rather than transferring the pass to the new content.
These local filesystem checks do not lock source files: a real CI producer must
still execute an immutable, identity-bound snapshot. Local MAC trust remains
the same-user boundary; no shared/CI cache trust is introduced.

## Evidence

[MEASURE.json](MEASURE.json) compares the old and new helper on a four-module
synthetic repository with a mocked toolchain and test-only authenticated pass
fixtures. It executes no Bend/theorems and records no real proof pass.

| Plan consumed | Before | After |
|---|---|---|
| Unchanged all-hit | Success | Success; no Bend/heavy guard |
| Transitive dependency changed after plan | Success (incorrect) | Refused |
| New module after plan | Success (incorrect) | Refused |
| Authenticated entry removed after plan | Success (incorrect) | Refused |
| Unchanged all-hit consumption, one observation each | 0.020ms | 0.722ms |

The complete comparison takes 0.07s /28,244KiB peak RSS ([GNU time](MEASURE.time)),
cost $0. These are fixture validation costs, not repository check or PR timings.
There are two complete key passes for warm success (plan and consume), three
when misses run (plan, before execution, after execution). Full-repository
cold/warm timing and production gate qualification remain pending.

Thirteen new tests exercise stale warm/mixed plans, tool changes, module
addition/removal, removed/corrupt cache entries, forged skips, malformed plans,
uncacheable units, `--all`, genuine all-hit without Bend/admission, and shell
failure propagation. The combined cache/closure/planner/guard suites pass
111/111 in 2.117s ([summary](TESTS.txt)). Lane 4 independently reviewed the fix,
passed 70 cache/closure/snapshot controls and checked shell syntax.

Reproduce the small comparison:

```sh
/usr/bin/time -v python3 -IB build/ci-speed/cache-plan/measure.py \
  --before 169f31fd5 --out /tmp/cache-plan-measure.json
```

Canonical laws/proofs and compiler code are unchanged. No full proof,
Boat/compiler/app/package/install/release job ran. Cache helpers remain opt-in;
this neither enables a workflow nor qualifies the sliced proof gate or release.
