# Message context sidebar source review

The changes to `thin-view.bend` and `thin-actor.bend` add source message IDs and folded display identities to the selected conversation's window. The new projection reads committed entries, fold context and the existing fold preference map. It introduces no draft-body read and does not change sidebar preparation, cache keys, image construction or command/session ownership.

`test/sidebar_draft_image_test.bend` passed on Boat job `j20261103161535-208155`: complete output with the retained sidebar equals output drawn afresh after draft changes. The source guard's two changed bridge hashes are updated after that qualification. Other reviewed inputs retain their existing hashes.
