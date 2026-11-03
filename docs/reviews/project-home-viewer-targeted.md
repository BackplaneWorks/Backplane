# Project viewer targeted qualification

The user prioritized these changes for the next combined redeploy and explicitly
authorized targeted checks on normal-size Boat workers below 4 GB, immediate
regular PR/normal merge, and no full compile or full proof. This is a scoped
source acceptance; the combined artifact still has to emit and start before
installation.

The independent sidebar review is preserved verbatim in
[project-home-viewer-sidebar.json](project-home-viewer-sidebar.json), SHA256
`fd0a6b05594f5fe283be4dc8fefe48701602dd064afe095b5d7c66254589243d`.
Only its four reviewed dependency hashes were manually refreshed. The 171-file
closure, guard, scanner, roots, bridges, draft filter, cache keys, authority and
unknown-`@` retention remain unchanged. The reviewer checked that 32 sidebar
and source-cache definitions remain unchanged.

Passed on official Bend 2.0.36, normal Boat, hard cgroup limit 3,799,998,464
bytes and swap disabled:

- 52 focused JavaScript tests: project file reads, composer, outbox, viewer
  intent, whole-sheet fit and camera retention, pane resizing and persistence.
- Two focused handler-inventory checks, including removal of the expand handler
  as a negative control.
- Nine small Bend policy assertions against the real contents projection.
- `src/core/project-home.bend --check-only`.
- Law/proof structural shape and pixel-difference metric negative controls.

Producer receipt:
`/home/h/.local/state/backplane/boat/jobs/j20261103195023-2113266/artifacts/report.json`.
An earlier run caught a report-timestamp fixture race; it now sets explicit
older/newer times and checks both stale and current reports.

The broader `src/web/project-home.bend --check-only` scope ended 143 without a
Bend diagnostic. That module check is **UNQUALIFIED**, consistent with a capped
scope OOM; neither success nor a source error is inferred. A separate diagnostic
retry was stopped for immediate source handoff and is not a pass.

The 24-case full-output sidebar fixture is **UNRUN on this changed source**.
There is no inherited PASS. Full proof, remaining module compilation, emitted
browser/pixel comparison, native compilation and screenshots are deferred.
The real-renderer comparison test is included for subsequent artifact
qualification; its metric unit checks do not establish renderer parity.

The installer receives the merged source identity and these limits. Installing
or starting a combined build requires its separate authorization and an actual
successful emission/startup receipt; this source review supplies neither.

Baseline advanced by normally merging memory PR #850 (`8021937a45f0fc4a3aa18dde560833cacf9dbb75`). None of the four reviewed module bytes changed; their exact source bindings and narrow review remain valid. No acceptance of the combined emitted artifact is inferred.
