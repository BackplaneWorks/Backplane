# Active threads live-hub regression

`test/tools/active_threads_load.ts` starts a new headless hub with twenty
synthetic active threads, one idle foreground thread and, by default, **1,000
settled threads across forty additional projects**. `--metadata N` selects the
settled thread count (0–10,000); each project contains up to 25 settled threads
with realistic titles, completed turn state and explicit manual settlement, without conversation
history or pending work. These metadata participate in the hub's full sidebar
catalog even when the settled shelf is collapsed. It uses a temporary
hub home, OS HOME, config/cache directories, project, agents, lock directory,
token and ephemeral loopback port. It never connects to an installed hub or
loads a real event log. No real agents run; updates, peer discovery, telemetry,
Tailnet and skill sync are disabled. It needs Linux `/proc`, Bun and an already
built hub binary, with no browser or generated wire bundle.

or remote runner. It takes approximately forty seconds with default settings;
do not build or run it beside another heavy job. Keep the same test checkout
and options for both binaries:

```sh
bun test/tools/active_threads_load.ts --bin /path/to/baseline/backplane-serve --metadata 1000 --out /tmp/active-baseline.json
bun test/tools/active_threads_load.ts --bin /path/to/after/backplane-serve --metadata 1000 --out /tmp/active-after.json
```

Both commands assert the same limits. A failing baseline exits nonzero and
still writes its report; run the after command separately. Reports include
binary SHA-256, fixture/client source hashes, all parameters, host CPU model,
PID, CPU tick frequency, catalog counts, phase boundaries and diagnostics. Compare reports
with identical `fixtureSha256` and parameters on the same runner. Different
workload or client sources are visible in the hashes. `--keep` retains the
isolated home, producer receipts and hub stderr. Normal cleanup stops only the
owned hub and synthetic agents, and removes that run's temporary directory.
Pass the hub executable itself (or its Bun script), not a shell launcher that
leaves a separate hub child: the test verifies the captured process executable
before measuring CPU and rejects zero CPU measurements under offered load.
An installed executable may be copied to `/tmp` for the baseline; always pass
that copy to `--bin`. Every invocation still uses its newly generated isolated
home, never the installed hub's home. Keep the fixture checkout unchanged for
the later after run.

Three real `view-web` sockets select an active thread, the idle foreground
thread and the sidebar (`recent`, with `sidebar.view=active`). They use the
production `ThinViews` client, negotiate view deltas/sidebar regions, and send
coalesced `view.update` requests when notified. A legacy observer measures
actual hub delivery for all twenty streams. This is a transport and hub-drawn
presentation test; it does not measure browser DOM painting or input latency.

The default workload offers forty 96-byte text deltas per second **per
thread**, 800 deltas/s total. In the twenty-second mixed phase each producer
also finishes a message and emits a synthetic tool call/result every second.
The ten-second pure-streaming phase emits no commits, after a three-second
settling interval. Each delta carries its production phase as well as its
thread, sequence and emission timestamp. After settling, a bounded ten-second
barrier checks that all twenty delivered streams have crossed into the new
phase with at most two seconds of lag. Producers keep offering the full load
through the barrier; its elapsed time and per-thread acknowledgements are
recorded. A failed barrier fails the run, and measurements continue to expose
the backlog. Wrong-phase production or delivery also fails. This prevents old
mixed-message backlog from silently masquerading as pure streaming without
relaxing rate or latency requirements. Agents stay active through both phases. A quiet foreground
must receive zero other-thread live invalidations between watchdog metadata
writes during pure streaming. `Watchdog.line` writes `liveness.<thread>` every
five seconds; these legitimate commits may refresh the sidebar. The report
records their keys and timestamps and excludes windows from 100 ms before to
1,250 ms after each observed watchdog commit only for the isolation assertion.
At least 25% of the phase must remain uncontaminated, and all twenty streams
must deliver their minimum offered rate during that time; otherwise the test
fails. No metadata window exempts foreign text or legacy broadcasts, and all
notices, refresh requests and CPU remain in full-phase measurements and bounds.
Unexpected commits in the pure-streaming phase fail with diagnostics. Mixed-phase sidebar traffic
must remain live. Every phase asserts:

- Each thread actually produces and delivers at least 80% of its configured
  rate, with no gap over two seconds including both phase edges, and p99
  delivery lag at most two seconds. Zero streams and initial bursts followed
  by silence fail. Mixed phases also require committed events from every thread.
- Own-thread notices are at most 20/s; idle/sidebar notices at most 2/s,
  both averaged over the phase and in sliding 975-ms arrival windows (25 ms
  receive jitter allowance). These bounds count received notices before client
  coalescing, so client request suppression cannot conceal notification floods.
- In the mixed phase, completed own-thread content must advance at the existing
  minimum rate of one fresh marker per second, with at most a two-second gap
  (including phase edges) and p99 lag at most two seconds.
- Pure streaming follows the product's **static pending** contract: partial
  response text remains private until `MessagePosted`. The selected active
  thread must retain exactly one `@live:<thread>` Working/pending row at the
  phase start and in every accepted view reply during the phase. At least one
  successful phase reply is required; missing, wrong-thread, duplicate or
  invalid pending rows fail. A pending row alone cannot prove transport
  liveness: all twenty legacy-observed streams must still meet the unchanged
  production/delivery rate, two-second gap and lag bounds. Fresh partial text
  is not expected in `shown`, and appearing there fails this contract. The
  harness sends no extra polling requests and requires no per-delta repaint
  for an unchanged label; watchdog metadata can still trigger view replies.
- All thin views reject foreign text and legacy broadcast frames. Closed
  sockets, bad replies, undecodable frames and five-second request deadlines
  fail. Notice and CPU bounds apply to both phases without exceptions for
  pending presentation.
- Actual hub process CPU is at most **80% of one core**, per phase and over
  the measured phases combined. `/proc/<owned-pid>/stat` user + system ticks
  cover all threads of that process; child agents and the measuring client are
  excluded. The denominator is measured wall time, never the host's core count
  or the hub's own span telemetry. Warmup is excluded. This explicit regression
  budget leaves 20% of a core for foreground work at the default offered load;
  it is a target, not a claim that any unmeasured build passes.

The report breaks down received notices, events, frames and bytes per second
for each client and frame kind; it records view request rates, methods, bytes
and RTT, plus per-thread offered/delivered rates, bytes, events, gaps and lag.
Each phase includes full `/debug/perf.json` snapshots before and after timing,
plus monotonic count/sum differences: spans sorted by total milliseconds with
phase sample counts/means, value-counter count/sum deltas, and gauge last-value
changes. The console prints the eight heaviest spans. Cumulative p99/max values
remain only in the raw snapshots; they are not presented as phase percentiles.
Snapshot request timestamps bracket the CPU interval and can include the cheap
GET overhead; CPU assertions still use only the direct PID samples and measured
wall interval. Missing or malformed snapshots and regressing counters fail.
CPU failures do not stop later phases, allowing a slow baseline to produce a
useful comparison. Transport or workload failures invalidate the run.

`--help` lists workload overrides. `--max-cpu` selects an explicit budget
between 0 and 100% of one core; do not change it between baseline and after or
reduce offered load to obtain a pass. The minimum configured stream rate is
40 Hz to exercise the 20/s notice bound. No result passes by disabling agents.

Cheap local validation starts no hub:

```sh
bun test/tools/active_threads_load.ts --self-test
```

This checks the actual codec, CPU sample parsing, notice window calculation,
1,000 settled metadata across forty projects, zero/dead/burst/phase-edge liveness
guards and one synthetic producer's protocol
and receipts (about 300 ms of output, no hub), including the phase switch that
must stop committed messages/tool calls/results, plus synthetic span/count diffs.
It also checks both pending-row representations, rejects missing/wrong-thread/
duplicate pending rows and rejects a phase with no successful view replies.
It is not evidence that the live-hub CPU or rate limits pass.

The `active-threads-v3` report marks presentation checks as `completed-content`
or `static-pending`. Its raw production fixture and load parameters are unchanged
from v2; its fixture hash changes with these assertion/source changes. Freeze
this version and the same client dependencies for both comparison runs. Earlier
v2 pure-streaming `advancing=0` failures are not evidence that the static pending
view froze; the independent CPU, delivery, latency and isolation findings remain.

The v4 fixture measures each phase in a fresh function call, avoiding the stale
mixed-phase callback selection observed in a v3 report. Each phase also records
`hubMemoryKiB` from the owned hub PID's `/proc/status`: current RSS, lifetime
high-water RSS and swap. These are hub-process values, not the surrounding
runner's combined memory peak.
