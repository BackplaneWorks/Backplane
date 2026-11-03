# Bounded conversation presentation and sidebar inputs

The presentation window is a read-only projection of the selected conversation.
`thin-body.bend` now clips quoted replies by the same body policy as other
messages and chooses a window capped at 32 rows and 32 KiB of encoded source
records. The existing issued body ledger and scoped cursors retain the complete
source. No persisted history is truncated.

The change adds no sidebar draft reader, no scratch-key filtering, no private
sidebar image-key change and no authority shortcut. The original unknown `@`
field policy, current-draft omission and metadata dependency fences remain.
`server.bend` composes its display fallback from issued source rows; it does not
rewrite canonical actor state or rerun an action. Oversized control values are
transmitted losslessly and adopted atomically under the original request ID.

Qualification: all 24 sidebar draft image cases passed on the exact changed
`thin-body.bend` bridge. The compact frame, legacy body continuation and issued
body authority checks also passed. The original source closure, scanner, and
all other reviewed dependency hashes are unchanged.

The matching build exposed an older native JSON counter hash fence. The full
`Json.bytes` section is byte-identical to commit `7d4d1fdfb` (section SHA256
`f80d1ea37f04d604a957566501313ba52f6434dbd8cb2c97a9c257d942ed1a2a`).
Only intervening lexer changes changed the module hash. Requalification against
the exact retained pure server emit passed 2,192 oracle comparisons, 607
overflow cases, 21 whole-tree comparisons and 11 negative build fences.
Main commit `0e4605666` already refreshed the exact module pin; this change carries no duplicate pin edit. Native counting and every drift fence remain.
