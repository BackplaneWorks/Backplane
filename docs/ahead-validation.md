# Ahead-of-time UI validation prototype

The required pre-push command is:

```sh
python3 "$snapshot/scripts/ui-validate.py" \
  --snapshot "$snapshot" --base "$base" --head "$head" \
  --changed-paths0 "$paths" --report "$report" --budget-ms 3000
```

Run from the caller's repository (read-only Git object access). `snapshot`
is an immutable exported checkout directory containing the exact pushed
head and needs no `.git`. `paths` is NUL terminated, includes deletions and both rename endpoints.
The command independently verifies the changed set and every required input
against the head's Git blobs. Exit 0 means the scoped required checks passed;
1 means failure; 3 means incomplete, including cold cache, timeout, missing
dependency, mismatched snapshot or corrupted receipt. It never builds or
waits for a lock. `UI_VALIDATE_CACHE` chooses the cache directory.
Reports and caches must be outside the snapshot; bytecode writes are disabled.

Static coverage always runs first. It checks literal `D.On` and `Dom.button`
bindings through the reachable dispatcher, explicit local clipboard gesture
ownership, syntactically inert indexed handlers, and the relative image URL
and authenticated `/img` route. Computed bindings are inventoried as unresolved.
This is a conservative source pass, **not** proof that every conditionally
reachable handler performs a useful effect or every remote image loads.
Unresolved computed bindings make the static approach and overall command
incomplete (exit 3), even with a valid simulator receipt. The currently
measured prototype therefore cannot supply an unqualified green pre-push
gate; its literal contracts can be consumed as scoped advisory findings.

Prepare a cold cache explicitly in CI, on Boat or under Backplane's `bend`
lock, with a 1 GiB shell stack:

```sh
ulimit -s 1048576
python3 scripts/ahead-validation/prepare.py --cache-dir /absolute/cache/path
```

Preparation emits the production `Ui.act`/`Ui.recv_act`/`View.page` functions,
runs 483 generated transitions, state/view invariants, 104 valid/structurally
truncated JSON/CBOR cases, negative controls, transport replay and trace
replay. It checks the complete input identity before and after. An emit by
itself cannot produce a receipt. Required source, validator spec, transitive
helpers (conservatively all production Bend/JS/CSS), compiler binary and
resources, named libraries, actual Node/Python binaries and relevant runtime
environment are bound to the receipt. Module bytes, results and receipt bytes
are checksummed. Any changed input invalidates reuse. This cache protects
against stale/corrupt results, not a hostile writer who can forge a cache and
the validator itself.

An explicit `prepare.py --module FILE --emission-receipt FILE --cache-dir DIR`
can adopt a Boat emit whose source and actual compiler identity match. It
reruns the simulator/transport/trace tests on the local runtime and writes a
new local receipt. A remote runtime receipt is never treated as a local pass.

Run independent historical negative controls with
`python3 scripts/ahead-validation/history.py --emit`, also under `bend` or on
Boat. It exports real buggy source revisions into `build/ahead-validation/`
and emits each there; the checkout and installed application are untouched.
The simulator's deletion shrinker produces a 1-minimal synthetic sequence;
it does not claim a globally shortest trace or exhaustive state exploration.
Canonical change frames require exactly-once delivery. Duplicate and
out-of-order **view replies** are exercised at the actual `ThinHost` boundary.

The exploratory `--malformed` driver option is a separate failing discovery
lane. It found one invalid Unicode-scalar throw and two malformed JSON
acceptances on the original snapshot. They are owned by the parser/security
thread; they are not baseline allowances and no all-input parser-totality
pass is claimed. The ordinary finite Unicode regression corpus passes.

Layout uses pinned Yoga 3.2.1 plus actual CSS selectors/cascade, with a bounded
adapter (`tinycss2` 1.5.1, `cssselect2` 0.9.0 and local Pillow/Noto fonts).
Install the JavaScript dependency with
`npm ci --prefix scripts/ahead-validation --ignore-scripts`. For each saved
production composer tree and width 320/390/430/1280, run `css-boxes.py` and
`layout.mjs`. `calibrate.mjs` compares the same saved synthetic tree/CSS in
Chromium without starting Backplane. Grid, pseudo elements, text shaping,
intrinsic media, actual container discovery and JS-managed presentation
remain limitations. Unsupported geometry and clipping are advisory; never
promote this adapter to a universal layout gate from a few height matches.

Trace schema version 1 is **owned by this prototype**, not inherited or
qualified from the stopped diagnostics thread. `makeReplayTrace` is an
opt-in decoded-hub-RPC capture hook, currently unconnected to installed hub
dispatch. Its ring contains at most 256 events/64 KiB, 128 session-local
numeric aliases, and clamped relative timings. No text, raw identifier,
filename, URL or content hash is exported. Unknown fields are rejected at
replay, and the disabled hook records nothing. `trace.mjs` accepts exported
fixtures and drives production pure functions with synthetic text, preserving
distinct aliased identities. Scroll/page geometry is explicitly unsupported
by this pure fold and covered separately in transport tests. Timings are
recorded but replay does not sleep. Live installation, authenticated export,
complete actor-state capture and automatic CI fixture intake remain required
before claiming real bug reports become regressions automatically.

The reviewed aggregate diagnostics contract is `d92814f7eef950dfe5faea041b1abbe78480eb8e`
(`#679`/`#734`), documented in `docs/diagnostics-backend.md`.
`diagnostics-adapter.mjs` validates its closed metadata, omits private exit
rows and unknown fields, and reports **zero replayable UI actions**. A join
or measured span is not evidence of reconnect, selection or a button press.

Invariant alignment with t1793671361-1312951 uses the audit's uncommitted
snapshot based on `92ddc7e46`, not an assumed merged theorem: navigation is
`ui_navigation_total` (`Ui.select.nav`); draft observations correspond to
`ui_view_error_preserves_all_drafts`/`ui_state_adoption_preserves_all_drafts`
(`Ui.error_put`/`Ui.adopt.raw`); unread corresponds to
`ui_bottom_has_no_unread_pill`/`ui_seen_boundary_has_no_unread_pill` and existing
`hold_shown_*`. The simulator exercises emitted transitions and sampled
views, not those universal proofs. The audit's `Cr.drained.k` duplicate and
commuting credit counters do not prove arbitrary event replay commutes.
Canonical pending outbox bounds/echo do not prove the JS optimistic lifecycle;
entry-ID cardinality/order does not prove text retention or byte capacity.

Measured findings and runtimes are delivered in
`build/ahead-validation/REPORT.md`. The report must keep misses, unresolved
coverage, exploratory failures and uninstalled hooks explicit.
