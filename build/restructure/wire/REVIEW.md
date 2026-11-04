# Private writer message boundary

Base: `e3b7d62136db73bb7c143288ee9c465d3ef7a79f` (merged server leaves #894).
Move the concrete Wire data definition, Wire.charged and Wire.text.charge into
server/wire.bend. The type, signatures and both function bodies are identical.
All nine variants and their fields remain: WText, WCharged, WPong, WBin, WClose,
WState, WItems, WJson and WTerminal. The module imports the original model and
JSON types, so payload types are unchanged too.

The actor and both native producer/consumer source files import this same
concrete definition. Normalizing only qualifications reconstructs every
remaining server declaration and every declaration in the five changed caller
files. No effects, argument ordering, frame formatting or wire credit amount
changes. The credit test no longer imports the whole server for pure policy.

## Guarantee preservation

The two original law statements retain their names, quantifiers, equations
and targets, with only the module qualification changed. All other original
law statements are identical after namespace normalization. The native-kept
proof still handles all nine constructors, with unchanged case bodies.

Relocating a private data type gives a bijection E from each old constructor
and its fields to the same new constructor and fields. Its inverse changes
only that qualification. The charging function's identical case table
commutes with E, since it inspects the same boolean/constructor and returns
the same variant/fields. Every real producer and consumer is updated together.

| Retained original law | Implication argument |
|---|---|
| mail_credit_carried | For every text and charge, the checked new function maps False/WText{text} to WCharged{text,charge}. Transporting through E and its inverse gives exactly the original offered-credit property on the original domain. |
| mail_credit_native_kept | The retained checked law quantifies over every value of the same nine-variant domain and every charge. It returns the input unchanged when native=True. For any original value w, instantiate it with E(w), then apply the inverse bijection; this yields exactly the original universal identity, with no variant or payload omitted. |

Four additional laws cover recharging already charged text, JSON and terminal
messages, and the original text-length accounting. Their proofs use the actual
production functions. No original guarantee is weakened or deleted; no abstract
replacement type, unchecked signature or stand-in implementation is introduced.

Wire is a private in-process channel value. JSON protocols and persisted event
types are unchanged. The phone state key is `0a8a7cc15ac90d48`; every source
input it covers and the key script/compiler pin are unchanged from the base.
The relocated Wire type is outside that covered graph (`PRESERVATION.json`).

## Measured proof scope

Reverse lexical declaration reachability selects exactly six affected laws.
Both original credit laws now import 12 implementation modules rather than
359, excluding Base. Their paired static declaration closure is 40. Text
accounting reaches three declarations. The global largest law remains 5,500;
the Main.boot semantic closure is unchanged by this relocation.

The first native result below is historical. The current resolved-source
result and Boat-only PR checks follow it.

Normal Boat job `j20261104023950-39517`, work `1501777`, stock Bend 2.0.36
SHA256 `02089dc0eed0fd5fd6d73c74cc9cffcb2c6638dd3b02ae83f7b8cc57fbe381ba`,
generated the full canonical 3,177-law/128-shard partition, identity
`402d57c5778207791ddbdd32f8ab4bca07bb94ab85e2962ab6b848d8a552ba67`.
It executed original native shard088, with all six affected laws and 25 laws
total. No slicer, template proof book or cached pass was used.

The shard passed the unchanged unsafe/foreign-only verdict (native exit 1;
100 existing host/performance/desktop-broker definitions listed, no failing
typed proof). Worker time 2.300 s, peak RSS 770,652 KiB, cgroup peak
772,591,616 bytes; effective cap 3,699,998,720 bytes, actual Bend flock held,
zero swap activity. The separate controller had cap 536,870,912 bytes, swap 0,
peak 71,008,256 bytes and zero OOM/max events. The sandbox was deleted and
verified gone, cost $0.00021. Raw output is preserved in 088.json.

This is affected-law evidence and a source/type-shape migration audit. Native
sender/receiver integration tests, full application build, all-shard join and
the whole-project memory target remain unqualified. RC4 stays frozen under
the install/build hold. After the new user rule, every further compiler/proof
or test run, even light validation, runs only on capped, no-swap normal Boat.

## Resolved merge and Boat-only PR checks

Current main's browser-cache change added a disconnect cleanup call beside a
Wire constructor reference. The merge retains `WIRE.WClose{}` and
`CST.Cast.drop(client)`, as well as the cleanup call on the other disconnect
path. Those hooks retain their original source-audit-only qualification; this
does not claim a full server/runtime compile or integration test.

Boat job `j20261104025314-148480`, work `1505990`, checked the resolved staged
source tree `f054751bb65068ac49b59c2f57e3cdbf992a6d78` against main
`2f2c1ab54a9d3015675aed38b855261037521a9e`. It generated the complete current
3,180-law/128-shard canonical partition, identity
`7eb8eb0b2c5f40022802a8a566e7c8ff06c695b9ec2af10bc9e38f9d53766ea4`.
Affected original shard088 passed the unchanged foreign/unsafe-only verdict:
1.994 s, peak RSS 824,352 KiB, cgroup peak 826,986,496 bytes, actual native
flock, effective cap 3,699,998,720 bytes, no swap activity. All 459 canonical
source file hashes still match the checked bytes after receipt-only changes.

The same Boat job then passed all 35 Python tests (3 mapper, 14 canonical-shard,
18 slicer), canonical and frozen merged-law shape checks, and source patch
whitespace/applicability. The entire test process tree had a 536,870,912-byte
cgroup limit, swap 0; peak 73,486,336 bytes, zero OOM/max events. No local test
or compiler ran after the hard rule. Raw native and check receipts are in
`merged/`; replay inputs for the light checks are archived there rather than
adding duplicate full books to the source tree. The sandbox was deleted and
verified gone, cost $0.00026. These checks do not close the full project gate.
