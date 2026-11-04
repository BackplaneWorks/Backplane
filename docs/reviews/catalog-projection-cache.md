# Catalog projection retention above 8,192 threads

The copied production catalog has 10,841 threads. SignalCache materializes a
signal for every thread, while IndexCache includes the catalog's thread rows.
Both previously rejected retention above 8,192 entries. SignalTextCache used
that ceiling plus its online marker. Consequently an otherwise unchanged
view rebuilt these full projections after crossing the catalog threshold.

The shared entry ceiling is now 16,384 (plus one online marker for text).
The scalar and text ceilings remain 512 KiB each, with the original per-key
and per-value limits. Oversized projections still render in full without
being retained. There is still one private slot per actor, guarded by the
same epoch, dependency revision and online visibility. Metadata values share
canonical immutable strings; no history or whole state is added to a cache.

Qualification: SMALL Boat work 1487635, job j20261104012435-1020998, Bend
2.0.36. Six 10,841-thread scalar/text checks, fifteen existing byte-budget
checks and six exact-body metadata checks passed. Peak RSS was 471,712 KiB;
wall time 7.90 s. Metadata qualification copied the production SIx type and
IndexCache/Size definitions with only the imported SIx namespace replaced;
its source/block hashes are recorded in metadata-scope.json. Laws shape and
i18n checks passed. The full client/render import closure exceeded the SMALL
limit in work 1485149; full-render parity was not rerun successfully. Full
root PROOF and a hub build were not run under the user's targeted-only limit.

This is a cache-reuse fix, not a measured installed-RSS improvement. Installed
945 failed its 30-minute memory gate. The private 945 soak also failed:
22.7 s to usable and a fitted RSS slope of +92,276 KiB/hour. Separate owned
before/after snapshots retained 223.1/223.7 MB of JS heap across forty metadata
changes. Neither result establishes the cause of all live memory growth.
A new exact packaged artifact still needs startup, CPU/delivery and unforced
memory qualification. Private stores and raw heap graphs remain outside git.
