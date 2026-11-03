# Message context sidebar source review

The changes to `thin-view.bend` and `thin-actor.bend` add source message IDs and folded display identities to the selected conversation's window. The new projection reads committed entries, fold context and the existing fold preference map. It introduces no draft-body read and does not change sidebar preparation, cache keys, image construction or command/session ownership.

`test/sidebar_draft_image_test.bend` passed on Boat job `j20261103161535-208155`: complete output with the retained sidebar equals output drawn afresh after draft changes. The source guard's two changed bridge hashes are updated after that qualification. Other reviewed inputs retain their existing hashes.

The store-backed follow-up reads at most two committed message identities and
32 source-to-run identities into private render files. SQLite retains bodies;
the projection still reads no composer draft. `thinq.bend` adds only fixed,
parameterized metadata queries. A definition-order correction in `view.bend`
moves the existing 3D helper above its caller without changing its body.

Boat job `j20261103210003-3049212` passed the stored-context regression, the
SQL checks, the context IO module check, and the complete sidebar output
comparison on these sources. The additional query, view bridge, and reordered
web renderer hashes are updated after that qualification.

The final follow-up applies the same stored run identity to web, native-window
and phone fold roles and to explicit body admission. It preserves the context
files when constructing a bounded render state. Only fold preferences are read
from the existing info map; ordinary composer drafts remain excluded from the
private sidebar image. The new pure message-context module is now in the
reviewed import closure.

Boat job `j20261103211522-3213449` passed the canonical identity, render-state
retention, explicit expansion, SQL and complete sidebar output comparisons.
The corresponding reviewed hashes and the new closure member are updated
after that qualification.

Final acceptance: Boat job `j20261103213009-3380607` passed desktop and
1,228-tool bot entry, with visible text and one explicitly expandable drawer.
Job `j20261103213635-3448305` passed the 390 px entry/expansion case, the
full `scripts/check.sh` type/proof gate and all eight required regressions.
The server, web client and shared phone bridge were built from those sources.
