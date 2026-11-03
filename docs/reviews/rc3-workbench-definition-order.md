# RC3 workbench definition order and source pin

Frozen product `c7e32b94b324ce81a3e138801509a5debb8c162a` failed its server-entry check because `View.solid.loc` used `View.solid.workbench` before that definition was filled. Bend requires called definitions to precede their callers.

The source repair already landed in `fc92fc968`, merged normally through PR #859 (`b7aaff1e4872215c1df11d688059c77030b40a58`), and is present in baseline `784422f3aceb741ff27db2cc31d64b6113b0fdce`. The change in `src/web/view.bend` is exactly a move of the unchanged five-line `View.solid.loc` block below the workbench. Removing that block makes the old and corrected files byte-identical. No viewer definition body changes. Other timeline changes in PR #859 are separate existing main content.

This follow-up refreshes only the stale `src/web/view.bend` dependency pin from `68e37010e56361a21f3d79a072430b0a088ab000345db22ed16eee13488b5fec` to `7ced34096836655e4a058704fd45c16d0ca7b02e916e2392e7fec995a0bb8ce8`. The 171-file closure, sidebar readers, filter, keys, guard, scanner, authority and unknown-`@` retention are unchanged. Independent exact-byte approval is preserved verbatim in [rc3-viewer-order-pin.json](rc3-viewer-order-pin.json), SHA256 `a123279d29a138569380c5bbdfff1225524d700cdb5268fe5a235626fd12f40b`.

The dependency guard and whitespace checks pass. The full-output 24-case fixture is UNRUN. One official `src/server/main.bend --check-only` is PENDING on normal LARGE work `w-t1793685091-1352063-1438949`; no PASS is inferred. It performs no web-entry check, application emission or full proof. No new XLARGE, install or publication is authorized by this pin refresh. RC3 must be refrozen to a source containing the repair and this pin update.
