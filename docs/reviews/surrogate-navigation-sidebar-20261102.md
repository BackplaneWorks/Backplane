# Surrogate and navigation sidebar review

Merged source `e37a3e66c86e7885174b1d00eb7482bea8964407` passed all 15 JSON cases (nine surrogate regressions) and all 24 full sidebar-output cases in work `w-t1793604413-1188217-1277028`. The entire 222-file executed input manifest was verified against this checkout. Those inputs remain unchanged by this fingerprint/documentation update. Compiler, Bun and all 66 compiler resources were pinned before execution and independently checked afterward.

Three fingerprints change:

- `src/core/json.bend`: UTF-16 escape units become valid supplementary characters; unmatched halves become U+FFFD. This pure lexer adds no Ui, session, info-map or draft lookup.
- `src/core/release.bend`: a pure formatter takes an explicit label, exit code and bounded caller-provided output. It adds no sidebar reader or draft lookup.
- `src/web/dom.bend`: Markdown cell splitting and trimming use tail recursion and restore input order. Nodes, caller inputs and reader authority are unchanged.

Roots, bridges, closure scanner, draft-slot filters, unknown-key handling, image keys, history and authority are unchanged. The input file set stays at 164. Full repository check/proof/default regressions and native-device qualification were not run and are not inferred.

The application must include the permanent surrogate fix before replacing the live crash workaround. Private escaped-status startup and actual installed STM32 entry/navigation acceptance belong to delivery, not this source review.
