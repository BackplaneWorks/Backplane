# Native JSON memo retention budget

The native server parse memo previously retained4096 small texts and8 large
texts with no input/expanded-value byte limit. It now preserves those FIFO
count ceilings while sharing a16MiB accounting budget. Inputs above262144
UTF16 units and entries above2MiB charged size parse uncached. The accounting
counts owned input storage plus conservatively charged parsed objects/strings;
it is not a claim that allocator RSS is exactly16MiB. Eligible input is copied
through UTF16 before parsing, preserving raw lone units while preventing a
small cached substring from pinning a larger input backing allocation.

No parse is rejected or truncated. Misses/oversized inputs return the unchanged
parser result. Hits retain immutable value identity, no hit promotion occurs,
and no exceptions are cached. Only numeric cache statistics are exposed to
owned diagnostics. No Bend/module, codec, protocol or sidebar-reader changes.

SMALL qualification1468484 / j20261104000646-377328 passed7 actual compiled
parser oracles plus the native helper suite; peak301080KiB. The original
assertion helper failed on deep lists; its replacement compares iteratively,
without relaxing exact constructor/field/value order checks.

SMALL1468864 / j20261104000846-393042 passed the actual server transform and
missing/duplicate/changed-definition negative fences, with web output pure.
The retained public RC4 raw emission was used; no new Bend emission or service
binary was built. Peak422828KiB,21.18s.

Two separate processes parsed eight0.67–0.82MB frames containing actual copied
store entries. After full diagnostic GC, retained heap growth was14494088B
before versus232034B after. Parse time was142.60ms versus161.99ms. RSS after
collection was203.75MB before versus209.42MB after: no RSS speedup/drop is
claimed. The new memo retained no oversized frames; ordinary model/small
record cache hits remain covered by the exact oracle tests.

This closes an unbounded-cache ownership hole, not the multi-GB incident.
The copied-store snapshot had221MB of node-shallow data and four nonempty
State histories with shared tails; it did not reproduce live multi-GB strong
retention. Only704 entries/668765 text characters arrived in the copied store
since seq1445046 (3744 total log lines/2424004bytes), insufficient to explain
GB growth by new payload volume alone. Startup, active-production allocation,
steady RSS and the unforced30minute installed gate remain open.
