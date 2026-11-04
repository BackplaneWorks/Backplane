# Compiler-oriented restructuring

Baseline: `a4a14b372` (2026-11-03). This is an incremental source/proof effort,
coordinated with the installer `t1792701529-415584`, compiler lead
`t1793604413-1188217`, and CI owner `t1793650867-1261555`. No compiler, release
gate, live installation, or qualification policy changes are part of this plan.

## Measured dependency map

Reproduce with:

```
python3 scripts/dependency-map.py --out build/restructure/current.json
```

Add `--graph` to include the indexed declaration adjacency lists and module
import graph. Every report also binds its measured modules by SHA256; this source
identity is descriptive provenance, not a proof-cache key.

The default roots are `PROOF.bend` and `src/server/server.bend`. This uses the
strict reader and lexical resolver from #871, excludes compiler/Base definitions,
and conservatively includes local-name collisions. These are static source
closures, not a compiler elaboration trace, memory predictions, or proof evidence.
The graph contains **43,547 declarations, 144,473 edges, 417 modules, 3,165 laws**.
Largest SCC has 7 declarations. Mapping took 10.67 s and 406,312 KiB maximum RSS
with `/usr/bin/time -v`; no Bend compiler was run. `BASELINE.json` records the
rankings and every module's size/import fan-in. Full rankings are reproducible
with the command above.

| Definition | Reachable declarations | Modules |
|---|---:|---:|
| server.Main.boot | 25,401 | 335 |
| server.Main.opened | 25,015 | 332 |
| server.Main.listened | 24,067 | 329 |
| server.Main.serve | 24,061 | 329 |
| server.Main.headless | 23,494 | 326 |
| server.Hub.loop | 23,374 | 322 |
| server.Hub.handle | 23,312 | 322 |

The source has **no law directly naming Main.boot or Hub.handle**. Treat those
as large runtime roots, not as existing law names. The largest paired law/proof
closures are different:

| Law | Reachable declarations | Modules | Imported-module union* |
|---|---:|---:|---:|
| hubs_screen_unfocused_thread_null | 5,500 | 116 | 176 |
| hubs_unpaired_quiet | 5,216 | 101 | 179 |
| held_empty_passes_press | 4,351 | 97 | 155 |
| held_empty_passes_release | 4,351 | 97 | 155 |
| held_swallowed_without_tour | 4,351 | 97 | 155 |
| menu_press_first | 4,245 | 96 | 155 |
| menu_right_press_first | 4,245 | 96 | 155 |
| watch_copy_launch_none/check_none/stop_none | 6 each | 3 | 359 |
| watch_copy_intent_none | 5 | 3 | 359 |

*Import union follows all imports of reached implementation modules, excluding
the omnibus LAWS/PROOF imports. The compiler checks unused definitions too;
small declaration reachability can still load hundreds of unrelated modules.
The #871 manifest is the execution authority, not this estimate.

| Module | Lines | Declarations | Direct importers | Transitive importers | Import closure |
|---|---:|---:|---:|---:|---:|
| core/msgs | 23,975 | 3,597 | 37 | 126 | 2 |
| server/server | 19,973 | 2,725 | 2 | 2 | 359 |
| core/client | 17,554 | 2,986 | 63 | 88 | 122 |
| core/hub | 11,170 | 1,684 | 20 | 56 | 83 |
| app/layout | 7,853 | 1,048 | 9 | 9 | 169 |
| web/view | 5,803 | 894 | 5 | 32 | 157 |
| core/model | 3,956 | 597 | 159 | 217 | 11 |
| app/nui | 3,749 | 597 | 9 | 11 | 155 |

Fan-in is among these roots' modules, not every repository test. Small widely
shared modules include core/json (217 direct / 299 transitive importers),
core/bot (53 / 220), and core/mail (35 / 220). Their low import closures mean
they should stay foundational; do not move feature behavior into them.

## Small merge units

1. Land this measurement script, its light graph tests, and the numeric plan.
   It changes no Bend source, law, checker, gate, or runtime artifact.
2. Extract the real Watch script/argv planner from server/server to a Base-only
   feature module. Retarget production call sites and the same four laws.
   Preserve every script byte and argument order. Add the generic copy gate
   law plus thin composition laws; explain why they imply each original law.
   This provides a small control with a 359-module baseline and very few actual
   declaration dependencies.
3. Remove the other law-facing leaves from server/server in separate PRs:
   pairing messages/QR, desktop connection metadata, adoption-copy selection,
   authentication cache, then writer-credit and log-count configuration. Keep
   configuration and message types separate from effects and actor dispatch.
   Narrow feature modules must never import server/server as a back edge.
4. Split core/client by feature with the shared Ui data in a foundational type
   module: viewer state, project/sidebar menus, composer/model picker, onboarding,
   and actions. Extract app/nui input gates and mobile/hubs focused-screen
   selection before restating their large laws. Preserve persisted type meaning,
   JS export identities/marshalling and every client's shared decisions.
5. Split core/hub deciders into feature modules: thread lifecycle, provider/model
   switching, project lifecycle, deploy/locks, mail/bots, and command dispatch.
   Separate pure deciders from IO in server/server; then move IO feature actors
   behind shared message/config types, keeping the dispatch and boot sequence thin.
   Move localization catalogs out of the broadly imported msgs module by domain
   only after measuring catalog fan-in; file length alone is not the priority.
6. Re-measure the complete law set and residual declaration coverage. Record the
   largest law closure and every shard's memory, OOM status and verdict. Continue
   splitting failing boundaries rather than interpreting successful samples as
   complete qualification.

## Guarantee preservation

Never remove an original law before its replacements and implication argument
exist. Prefer retaining its name and exact quantified property, changing only
the module qualification to the actual moved implementation. For compositional
restatements, record in each PR: the original statement, each new quantified
statement, the real production call site, and the derivation of the original
from the conjunction of replacements. Thin composition must use the checked
small functions; a generic theorem alone does not establish that the runtime
uses them. No unverified interface, callback assumption, unsafe stand-in, law
deletion, weaker quantifier, or altered effects order counts as a solution.

Moving a file does not shrink Main.boot's semantic closure: it still genuinely
starts the whole server. A stock compiler that checks every imported body may
still load the whole system for the residual runtime check. This effort must
report that limit honestly and coordinate with the compiler lead's separate
compilation work; narrower laws do not justify dropping residual code coverage.

## Memory protocol and release coordination

Keep the canonical `proof-shards.py` planner/verification and #871's experimental
`proof-slice.py` distinct. The slicer is **unqualified and unwired**, not a
substitute for the canonical proof gate. The historical sliced result in
`docs/lowmem/proof-slicing-negative-result.md` was **local**, through
`/tmp/ps/capped_run.sh` and `batch.sh`, with a 3.7e9-byte/no-swap scope and batch
flock. It did not use Boat, a 512 MiB controller, or heavy-guard admission:
299 passing shards, 104 OOM kills after 4 bisections, highest passing peak
3.55 GB, incomplete overall. These are diagnostic numbers, not Boat qualification
or a fresh baseline for a4a14b372. The admitted Boat run 1465396/#877 instead
checked **unsliced canonical shard000**.

Merged #854's guard accepts canonical `proof-shards.py run` under the worker's
own <=3.7e9-byte/no-swap cgroup. It does not admit arbitrary declaration-sliced
work. No diagnostic sliced run may be relabeled as an admitted shard.

Fresh before/after runs use normal-size Boat, explicit cgroup MemoryMax
**3,700,000,000 bytes** (stricter than 3.7 GiB, kernel rounds to 3,699,998,720),
MemorySwapMax=0; read limits from inside each worker. Never use xlarge or infer
compliance from RSS alone. Preserve frozen source/manifest/compiler hashes,
worker verdict, peak RSS, cgroup peak and OOM counters, swap counters, wall time,
and exact coverage. A failure/refusal is recorded as such. No full-gate claim
until all law and residual source coverage is verified. At most one local heavy
job, held under the shared named lock; prefer budgeted Boat jobs.

Before large moves merge, confirm the installer is not mid-cut. Run the relevant
tests and laws on exact PR inputs, the regular merge gate, and pre-merge law shape
check; merge regular PRs immediately when green. Coordinate development server
updates with the installer so RC artifacts and live state remain undisturbed.

## Qualification status

Dependency mapping and three graph tests pass. Fresh attempt
`w-t1793755214-1485269-1487534` was launched once, preserved without duplication:
`scripts/boat-job.sh --size default --idle 0 --timeout 600`, canonical 128-shard
generation, then `systemd-run --user --scope --unit=restructure-before-000
-p MemoryMax=3700000000 -p MemorySwapMax=0 -p OOMPolicy=continue --
python3 scripts/heavy-guard.py proof-shard -- python3 scripts/proof-shards.py run
... --shard 000`. The source has no runtime/law changes. This launcher does
**not** separately cap a 512 MiB controller.

Result: normal Boat job `j20261104012426-1018805`, Bend 2.0.36, unsliced canonical
shard000, **failed**: compiler exit -9 (SIGKILL), 28.4315 s, peak RSS 3,624,840 KiB.
The worker recorded actual Bend flock held, effective memory.max 3,699,998,720 B,
aggregate cgroup peak 3,699,998,720 B, and unchanged zero swap-in/out counters.
This VM had swap provisioned, but the worker's own scope forbids using it.
All manifest source hashes were verified against the unchanged local sources.
`BEFORE-BOAT.json` preserves the launcher, compiler/manifest identities, guard
identity and raw worker receipt. No kernel OOM counter was captured: SIGKILL at
the cap is not being presented as an independently confirmed kernel OOM event.
The post-exit systemctl query returned default success/infinity/[not set] fields;
those do not describe the completed worker and must not be used as verdict or
peak evidence. The sandbox was deleted and verified gone (81 credit-s/$0.00081).

Runtime extraction, compositional law proofs, fresh after measurements, and
all-shard qualification remained pending at this first PR. Subsequent milestones:

| PR | Boundary | Before/after implementation import closure | Native affected-law result |
|---|---|---|---|
| #890 | Watch planning | 359 to 1 | canonical shards098/124 passed; highest RSS171,836 KiB |
| #894 | Pairing/desktop/adoption | 359 to 5/2/15 | canonical shards077/105/121 passed; highest RSS431,604 KiB |
| #896 | Private Wire and writer credit | 359 to 12 | canonical shard088 passed; latest RSS822,064 KiB |
| #905 | Authentication cache | 367 to 12 | canonical shard088 passed; RSS325,804 KiB |
| #912 | Six shared pure language functions | native186/policy122 to2 | canonical123/124 passed; highest RSS2,894,204 KiB |
| Catalog lookup leaf | Exact lookup/metadata bodies separated from messages | msgs3,597 to1,368 definitions;23,975 to9,580 lines | canonical052/091 passed existing verdicts; highest RSS686,064 KiB |

Each change retains every original law and adds concrete compositional laws;
boundary-specific REVIEW.md files state the implication arguments and scope.
All listed after runs use normal Boat, native guarded workers capped at
3.7e9 bytes/no swap, and separate 512 MiB/no-swap controllers. Successful
foreign/unsafe-only verdicts are distinguished from exit-zero proof checks.
Auth's source audit and 35 Python checks also ran entirely on capped Boat.
The latest user constraint prohibits **all local compiler, proof and test runs**,
including light tests. RC4 remains frozen and the installer holds builds/installs.

Auth's paired original-law closure grows from49 to51 due to explicit proof
composition; this is reported alongside its reduced import cost. The largest
law was5,500 declarations at the Auth milestone. The language leaf's real
delegates increase it to5,502, while its own law imports shrink to2 modules.
Full before/after per-shard qualification,
residual runtime coverage, and the whole-project memory target remain open.

The configuration/log-count proposal remains preserved and unmerged after an
unsuccessful canonical attempt. Its positive mechanical audit does not replace
native qualification. Catalog lookup was freshly qualified on top of the merged
language leaf: all3,597 bodies preserved across35 modules,47 Python tests and31
generator cases passed, plus45 laws in two original native shards. The largest
global law remains5,502. Ordinary message imports load2,229 fewer unrelated
definitions; all new lookup/metadata modules are at most447 lines.
The strict canonical generator
still permits only1..128 shards; no one-law partition, generator expansion,
declaration slicing, cap/guard relaxation or cache/gate wiring was introduced.
