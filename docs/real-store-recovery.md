# Private real-store startup and memory gate

`test/tools/real_store_recovery.ts` starts its own disposable loopback hub from
an owner-supplied SQLite backup and exact applied log prefix. It verifies the
packaged binary SHA and fixture log SHA. The archive contains only
`RECEIPT.json`, `store.db`, `events.jsonl`, and optional
`providers/accounts-v1/profiles.json` metadata. Vaults and provider credentials
are excluded. No existing home, live address or pairing input is supported.
Stand-ins, copy mode and an outbound network fence isolate the child.

The runner observes the actual listening announcement in stdout/stderr, sends
one hello, and opens a conversation and sidebar using the normal client
protocol. Usability includes both accepted views. Two clients then serialize
refreshes for 30 minutes. The unforced gate requires usability within15s,
complete5s RSS/HWM/swap samples, steady RSS at most1GiB after5min warmup,
no sampled swap, and RSS slope/end growth at most64MiB/hour and64MiB
respectively. Short smoke runs cannot pass the soak. No inspector, forced GC,
or heap snapshot is enabled. Reply bodies and private logs are deleted with
the temporary child home; results contain only numeric measurements/counts
and hashes. This isolates hub behavior, not real provider/network activity.

Run `scripts/real-store-ci.sh` on a private normal SMALL VM (<4GiB), with
`BACKPLANE_QUALIFIED_HUB` pointing to an immutable packaged service copy,
`BACKPLANE_QUALIFIED_HUB_SHA256` naming its receipt hash, and
`BACKPLANE_PRIVATE_REAL_STORE` pointing to the owner's private archive.
The named private CI workflow uses the `backplane-private-small` self-hosted
runner label; that runner and its staged inputs must be provisioned by the
owner. It builds and installs nothing and publishes no fixture/artifact.
The ordinary CI job tests the gate's early-exit, sample-loss, swap and growth
negative controls without accessing private data.

## November3 RC3 investigation

Read-only live observations showed anonymous/JSC memory dominating, rather
than file mappings. No live debugger, GC, signal or restart was used. Service
cgroup peaks also include child processes; they are not hub RSS alone.

An isolated copied store at seq1448789 contained10841 threads,246822 settings,
419731 entries and1448789 log events. On SMALL Boat,250 bounded10Hz samples
put124 in SQLite iteration. The startup recent-order query returned11110
thread ranks in5874.22ms. Its covering-index replacement returned the exact
same11110 rows in21.63ms, including history-only threads. This is a query
measurement, not an end-to-end startup speedup claim.

The first clean, unforced two-view RC3 smoke measured47.57s usable time,
915MiB final RSS,1212MiB VmHWM, zero sampled swap. It lasted60s, so its
30minute and steady-slope checks correctly failed. Earlier fixed-delay and
lost-stdout readiness attempts remain harness failures. An owned diagnostic
full GC reduced heap205.3MB to190.9MB while RSS stayed near971MB; that is
not a live leak attribution or unforced acceptance result.

The repair removes whole-heap diagnostics from `/hello`, bounds explicit
heap-stat sampling to one numeric pair per minute, uses covering recent-order
indexes, avoids copying accumulated changes/outs for no-op recovery steps,
and gates triage-key JSON parsing before unrelated setting suffixes reach it.
The full Hub-importing fixture exceeded the SMALL compiler limit; exact-body
scoped checks passed instead. Full hub build/root proof were deliberately
omitted under the user's build-memory constraints. Installed usable-time and
30minute memory acceptance remain required; the multi-GB incident is open.
