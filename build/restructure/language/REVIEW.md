# Pure client language boundary

Base: `22d5e2202204371d3ad58f052f87cd4f89501b1f`.
Six real pure functions now live in the 27-line client-language.bend, importing
only Base and the existing i18n module. Five bodies are identical to the old
client policy; native_locale is identical to the old phone entry point after
qualifying those calls locally. Six original entry points keep their exact
signatures as transparent delegates to those same checked functions. Client
actions, phone exports, event types, field order and effects remain unchanged.

The source audit checked all six body moves, all six exact-signature/argument
delegates and their import targets, every other client/app declaration, every
original law statement and all other proofs. It also checked every type block
in the changed client file. The phone state key remains `0a8a7cc15ac90d48`;
all seven covered files' type inputs, the key script and compiler pin are kept.
No abstract interface, callback assumption or unchecked stand-in is introduced.

## Guarantee preservation

No original law is deleted. The six policy equations retain all quantifiers,
inputs and results, with only the target qualification changed. The old
U.Lang.resolve is definitionally the new function through its one-call delegate;
the new implementation is the old body, with its helpers copied identically.
Thus each checked property transfers to every original input, not a sample subset.

| Retained law | Original guarantee implied by the checked moved function and actual delegate |
|---|---|
| lang_default_auto | Empty setting with en_US.UTF-8 and preview=False resolves to en. |
| lang_default_unknown | The original garbage setting, OS string and preview=False resolve to en. |
| lang_gate | For every OS string, ja with preview=False resolves to en. |
| lang_gate_preview | For every OS string, ja with preview=True resolves to ja. |
| lang_auto_preview | Auto with ja_JP.UTF-8 and preview=True resolves to ja. |
| lang_override_en | For every OS string and preview boolean, explicit en resolves to en. |

The four native law equations keep their names and literal inputs/results.
Their import now targets the actual moved native_locale. The real phone entry
point delegates to it with the same three arguments. The new universal
native_locale_composition law equates that function to normalization followed
by the real resolution policy, for every setting, OS and preview string.
Each original proof explicitly instantiates this law; the concrete policy
reduces to its original result. The filled composition proof precedes its users.

| Retained native law | Instantiation and reduction |
|---|---|
| native_language_english | en / ja-JP / "1": normalize en, explicit override en. |
| native_language_preview | ja / en-US / "1": normalize ja, enabled preview gives ja. |
| native_language_system | auto / ja_JP.UTF-8 / "1": auto uses the Japanese OS tag. |
| native_language_fallback | auto / de-DE / "1": unsupported OS tag falls back to en. |

These implications use the actual production delegates and identical concrete
implementations. All ten original guarantees hold over their original domains.

## Measurements and validation

Before/after lexical closures, excluding Base, are bound by METRICS.json and
both source hash maps in current/. They are source reachability, not an
elaborated compiler graph or memory prediction.

| Property group | Imported implementation modules before/after | Paired declarations before/after |
|---|---:|---:|
| Four native language laws | 186 / 2 | 15 / 17 |
| Six language policy laws | 122 / 2 | 12 / 12 |
| New universal composition | — / 2 | — / 15 |

The two added native proof declarations express composition. The complete map's
largest law remains hubs_screen_unfocused_thread_null, increasing from 5,500 to
5,502 declarations through real delegates, with 117 reached modules and 177
imported modules. This change does not establish the global memory target.

Normal daily Boat j20261104045848-1173408 / work1541125 checked exact source
tree `cd5b9972c95fd2c5a6e00398d1509a9efb8328c8` on pinned Bend2.0.36.
It generated the full canonical3,196-law/128-shard partition, manifest
`121f0a785a98128c2bd1daf7d85fd4ae1ba05c01c4193280c8f34c595535071d`.
Only original shards123/124 were executed: 50 laws total, covering all11
requested laws. Both exited0 with ALL PROOFS CHECK. No slicer or cache was used.

| Original shard | Seconds | Peak RSS KiB | Cgroup peak bytes |
|---|---:|---:|---:|
| 123 | 13.803 | 2,894,204 | 2,950,627,328 |
| 124 | 0.335 | 137,096 | 121,458,688 |

Each native worker held the actual Bend flock and an own effective
3,699,998,720-byte cap with swap forbidden; swap-in/out counters stayed zero.
The separate512MiB/no-swap controller peaked at74,227,712 bytes with zero
max/OOM events. All35 Python tests, both law-shape checks, patch checks and
the body/delegate/type/law audit passed in a512MiB/no-swap process tree,
peak54,415,360 bytes. Static mapping peaked420,179,968 bytes under its own
512MiB/no-swap cap, with zero max/OOM events. The normal default sandbox was
deleted and verified gone; cost62credit-s/$0.00062. No local test/compiler ran.

Earlier attempts are preserved separately. One mapper attempt lacked baseline
dependencies and ran no native worker. The next native attempt passed123 but
failed124 because the composition fill followed its callers; this was corrected
without changing a law. The corrected source was freshly checked in both shards.
Neither earlier result is substituted for a current receipt.

This is affected-law and source-preservation qualification only. Full128-shard
join, remaining production code, app/runtime integration, build and installation
remain unqualified. There is no matched whole-gate before/after memory claim.
The frozen0.14.1 hotfix, its producer packet and its emit are separate; this
source PR neither enters that cut nor triggers a build/install/server update.

To replay the frozen audit, unpack current/SNAPSHOT.tar.gz at the repository
root before transporting the identity-fenced inputs to normal capped Boat.
The complete canonical manifest is preserved as current/MANIFEST.json.gz;
decompression reproduces the exact partition bytes. It and the raw worker
receipts are evidence, not a seeded cache.
