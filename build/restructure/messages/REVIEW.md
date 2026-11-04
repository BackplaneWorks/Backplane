# Catalog lookup boundary

Base: `70c642b71ef713386858a66fd584c5a4dc89fed8` (merged language leaf #912).
Separate ordinary message functions from catalog lookup and metadata. The
generator still emits the same bodies, keys, literals, parameters, locale
branches, plural/select behavior and sorted binary search tree.

The 1,368 translated functions keep their original module and names.
Lookup retains its exact tree nodes in a root and 32 balanced subtree modules;
metadata has its own module. Every new lookup/metadata file is at most 447 lines.
The phone's real App.t changes only its import target, retaining the MSG alias,
body and signature. The existing i18n test imports lookup/metadata explicitly.
The generator's monolithic fixture API and native output generation remain intact.

## Guarantee preservation

Every original law statement and proof body is unchanged. The mechanical audit
establishes a bijection from all 3,597 original declarations to the 35 generated
modules: no additions, losses or duplicates, and every body/signature is identical
after qualifying references to the corresponding moved declaration. The1,368
ordinary translation functions do not even change their namespace.

This is a renaming of the same concrete program graph. Base/i18n references,
literal values and argument positions stay fixed. Each old evaluation step has
the corresponding new step; the lookup tree and metadata recursion have the
same branches, edges and base cases. Therefore all original functions have the
same results for all original inputs, not only the checked lookup examples.
The real App.t computes the same locale, calls the corresponding lookup with
the same key, then applies the unchanged t.k fallback. Its result is consequently
unchanged for every hub state and key. All other app declarations are identical.

Six additional laws check the real moved functions, without replacing an old law:

| Added law | Checked guarantee |
|---|---|
| catalog_lookup_common_cancel | For either locale, lookup of common.cancel equals its actual translated function. |
| catalog_lookup_parameterized_absent | For either locale, files.changed is excluded because it needs arguments. |
| catalog_lookup_unknown_absent | For either locale, the original lookup returns None for the stated unknown key. |
| catalog_lookup_root | For every locale and key, public lookup is its actual checked tree root. |
| catalog_metadata_complete | Both locale catalogs have no missing translations. |
| catalog_keys_nonempty | The actual generated key list is nonempty. |

The metadata laws also keep its complete module under canonical checking.
No callback assumption, unchecked signature, stand-in or omitted original
guarantee is introduced. All real references in changed caller bodies were
audited after namespace normalization. Source inventory found no direct JS/TS
consumer of the relocated API; the phone's App.t export remains unchanged.

## Measured scope

Before/after reports in current/ use lexical source reachability, excluding Base,
and bind every measured file by SHA256. They are not compiler elaboration traces.

| Measured msgs.bend | Before | After |
|---|---:|---:|
| Lines | 23,975 | 9,580 |
| Bytes | 700,068 | 254,777 |
| Declarations | 3,597 | 1,368 |
| Direct importers among measured roots | 36 | 69 |
| Transitive importers | 124 | 157 |
| Imported-module closure | 2 | 2 |

The additional importers are real lookup subtrees sharing the translation
functions. Ordinary message callers now load 2,229 fewer unrelated definitions
from this module. Full lookup still needs its complete tree: its new laws reach
3,304 paired declarations/35 implementation modules. Metadata laws reach 6/27
declarations and2 imported modules. The global largest law stays 5,502; this is
a partial source-boundary improvement, not whole-project qualification.

Normal daily Boat j20261104064043-78558 / work 1545925 checked exact staged tree
`035a30a7d917a2e0a01e7256c36d97dc9fe2a496` on stock pinned Bend 2.0.36.
It generated the complete canonical 3,202-law/128-shard partition, manifest
`ed28d32b86bf1841a068efd6f2d26bcad481b5ce3efc70fd63e3a654d109378c`.
Only original shards 052/091 ran:45 laws total, covering all 11 requested laws.
No slicer, cached pass or partition/gate/guard policy change was used.

| Shard | Verdict | Seconds | Peak RSS KiB | Cgroup peak bytes |
|---|---|---:|---:|---:|
| 052 | Existing unsafe/foreign-only policy accepted native exit1 (86 existing definitions); no typed proof failure | 1.782 | 686,064 | 685,035,520 |
| 091 | Native exit0, ALL PROOFS CHECK | 2.079 | 375,528 | 366,895,104 |

Both held the actual native Bend flock and own effective3,699,998,720-byte cap,
with swap forbidden and swap-in/out counters unchanged at zero. The separate
512 MiB/no-swap controller peaked 74,153,984 bytes, zero max/OOM events.

The whole source audit,47 Python tests,31 generator self-test cases, generated
output freshness/completeness check, both law-shape checks and patch checks
passed in a512 MiB/no-swap scope, peak 49,438,720 bytes. Swift/Kotlin and iOS
permission outputs are unchanged. The phone state key is0a8a7cc15ac90d48, with
unchanged covered type inputs and compiler pin. Static mapping had its own
512 MiB/no-swap scope, peak 420,438,016 bytes and zero max/OOM events.
The default sandbox was deleted and verified gone; cost 48 credit-s/$0.00048.
No local test/compiler or app build/install ran.

The earlier catalog candidate's failed shard002 is preserved under attempt-1/;
it terminated without a final receipt, so no worker peak or OOM cause is inferred.
It is not a matched before-memory baseline or substituted receipt. The language
dependency it exposed was independently fixed and merged in #912 before this
fresh catalog run. The original full proof/runtime/build gate and matched full
before/after shard memory remain open; successful samples do not qualify them.

The installer explicitly permits independently qualified main-only catalog
merges through normal fresh Boat premerge. The frozen0.14.1 hotfix source,
tooling, producer packet and emit remain separate, with no build/install trigger.

current/MANIFEST.json.gz preserves the complete canonical coverage manifest.
Unpack current/SNAPSHOT.tar.gz at the repo root to restore the identity-fenced
audit replay inputs before transporting to normal capped Boat. Receipts are
evidence and are never promoted into a proof cache.
