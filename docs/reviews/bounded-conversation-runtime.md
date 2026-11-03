# Conversation refusal reproduction

An isolated copy of Bub's conversation contained 18,409 message/activity rows
at the captured boundary. The test also created a bot conversation with 55,228
rows (three copies plus one oversized Unicode/Markdown entry). No live history
was modified. Eighty live projects exercised oversized fixed chrome.

The installed build refused the initial view with `LIMIT`:

- logical reply: 452,997 JSON bytes
- mandatory fitting budget: 229,376 bytes (224 KiB)
- sidebar: 357,052 bytes
- timeline: 1,216 bytes
- shown tree: 451,441 bytes
- session: 500 bytes; window: 694 bytes; commands: 2 bytes

This was `fixed-view-bytes`, not the actor-capacity rejection or an entry-count
limit. The independent transport cap is 262,144 bytes (256 KiB).

The matching build on main `2ec3c0edd` passed the real Chromium regression on
trusted `h@backplane` (work `1339858`): source content painted for both histories,
older paging worked, thread and bot navigation worked, and no status bar or
`LIMIT` appeared. The real copy painted seven rows; the pathological triple
painted one bounded row. All 53 focused Bend assertions passed, including all
24 sidebar draft-image cases, before the two changed bridge hashes were pinned.

The public source transfer covered 3,038 files (manifest SHA256
`a14e32936703ea7143134b5b2e6834b38b9ec37f1f8cc8f27e4eab18432e6b67`).
The approved Bend hash was `b2b183bddc3c3fa4bc45d99f6a6bdbb192ebc616e6e697777f49e74c545b1643`;
Bun was `1.3.10+30e609e08`, hash
`795b37d736a88d746b0e8bb5fcde92bf29bd0f9ac6a1dd1a950fd37440cf5822`.
All 66 compiler resources matched manifest
`28ad003e5ab3a3128d984a3fea418003d554d353debfd7c0ed69e26d1f42002d`.
The final sidebar manifest was
`a7d174051cda30a238ad9817120bf9182cc9c1d6aa6874e1376de559c8cf9c4e`.

`scripts/check.sh proof` exited zero in 470 seconds on the same source with
nine new conversation laws. Bend reported only the existing 2,579 unsafe/foreign
definitions; the unchanged `bendok` verdict accepted it. This is not a literal
“All terms check” output. The additional final producer-byte theorem remains a
separate pending composition check; the checked source clipping, lossless
control split, row cardinality/nonempty, floor shape/text, successful envelope
and actual-cursor emission laws do not claim arbitrary history issuance.

Additional focused checks passed: 214 JavaScript tests including exact control
state/effect reassembly, no partial adoption, request correlation, restoration
of canonical entry identities, recovery timer fencing and serial paging.
