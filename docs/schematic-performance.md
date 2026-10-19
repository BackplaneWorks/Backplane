# Schematic and history regression checks

This integrates Cameron Basham's PR [#64](https://github.com/i2cjak/Backplane/pull/64)
with the current localized shared client. The sheet reader evaluates the tail
once. The shared history bar reads its track once; web and phone bars share that
same track with their ticks and comparison menus. The phone avoids parsing a hidden bar. The native
bar uses the shared implementation; its scrubber still obtains its own track.

Large sheet tables use the existing tail-recursive line splitter instead of
Base's character-recursive splitter, preserving blank-line behavior. No persisted
state type or event meaning changes.

Run the focused cases and generated fixtures from the repository root:

```sh
bend test/hier_test.bend
bend test/hist_test.bend
mkdir -p build
bend src/core/hier.bend -o build/hier.mjs
node --stack-size=500 test/tools/hier_bench.mjs build/hier.mjs
bend test/tools/history_render.bend -o build/history-render.mjs
(ulimit -s 131072; node --stack-size=65536 test/tools/history_render.mjs build/history-render.mjs > build/history.json)
scripts/check.sh
```

The history runner covers 0, 3, 64, 256 and 1,024 synthetic CAD steps, English
and Japanese, live/first/last/diff/play/compare/schematic/closed states, and the
shared bar, web DOM, comparison menu and phone JSON. It hashes every field and leaf with an
iterative walk. The enlarged Node stack is only for exporting full client states
through Bend's generated test API; it is not evidence of phone stack acceptance.

For an exact before/after comparison, build the same fixture on main at
`30884966ef59c383581bcd1b7653f8f739cf009e`. Its wrapper needs these API substitutions
in `test/tools/history_render.bend`:

- `W.View.hist.of(l, U.Hs.track(u), u)` becomes
  `W.View.hist(l, U.Hs.bar.loc(l, u), u)`.
- `W.View.hist.menu(l, U.Hs.track(u), u)` becomes `W.View.hist.menu(l, u)`.
- `P.Hist.json(l, True{}, u)` becomes
  `P.Hist.json(l, U.Hs.bar.for.loc(l, u, True{}), u)`.

Run the current JS runner on that baseline module first, then pass its JSON as
argument three when running the candidate module. Every one of the 320 hashes
must match. Timing samples include the generated JS export marshalling overhead;
they compare identical inputs and APIs and are not device display latency.

`hier_bench.mjs` reads and round-trips the same labeled Unicode sheet rows at
increasing sizes, then checks empty rows. A baseline run can be limited to 20
rows with argument three: the old strict conditional doubles the tail work for
every row. The candidate goes to 1,024 rows by default.

On the generated sheet-table fixture, current-source main took 7,585 ms for
20 rows; the candidate took 2.4 ms for the identical 20 rows and 37.2 ms for
1,024 rows. Empty-row behavior and Unicode round trips matched. A separate
candidate run with `node --stack-size=500` completed 1,024 rows in 32.4 ms.
These are parser timings from Node 22.23.3 on this Linux host, not phone display
latency. The original PR's historical timings are not used as measurements.

All 320 baseline/candidate output hashes matched on the current source. At
1,024 steps in English, median shared-bar rendering fell from 107.2 to 23.0 ms,
web history from 120.0 to 23.5 ms, and phone history JSON from 679.7 to 27.0 ms.
The benchmark uses seven samples per case and includes generated API marshalling;
these results are from Node on an Intel Core i7-8650U Linux host. The comparison
uses the same fixture, language, navigation states and runner for both modules.
The full focused checks and affected app bundles remain separate acceptance
lanes; the enlarged test stack does not certify a phone's stack or frame budget.
