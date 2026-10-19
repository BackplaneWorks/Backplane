# Perf bench: the window and hub under load

`test/tools/perf_load.ts` runs the native app (hub and window in one
process) on a copy of a real event log, with stand-in agents streaming, and
measures it phase by phase against the targets. Failed targets or missing evidence
exit nonzero by default. `--diagnostic` can collect a failing experiment without a
nonzero timing exit, but every such report records `accepted: false`; invalid
workloads still fail. The former `--fail-on-target` flag remains compatible and
is no longer required. Keep one immutable log
snapshot for a baseline/change comparison; each report records its byte
count and SHA-256 so different inputs are visible:

```sh
cp ~/.backplane-bend/events.jsonl /tmp/backplane-perf-snapshot.jsonl
bun test/tools/perf_load.ts --bin build/backplane --log /tmp/backplane-perf-snapshot.jsonl --out /tmp/perf-result.json --diagnostic
```

| target | metric |
|---|---|
| frame p99 ≤ 8 ms | `ui.frame` |
| no frame > 16 ms while streaming | `ui.frame` in every measured streaming phase |
| typing ≤ 16 ms | `ui.input` in `type` and `normal` (also shown: key>frame) |
| key handling through frame ≤ 16 ms | `ui.msg.win` + `ui.input` in `type` and `normal` |
| thread open ≤ 50 ms | Return in the palette to the frame showing the thread |
| agent line ≤ 2 ms | `hub.agent_line` p99 |

## Running it

```sh
scripts/build-app.sh src/app/main.bend build/backplane      # inside bp-admit with explicit memory/cores
cc -O2 test/tools/xpoke.c -o build/xpoke -lX11
bun test/tools/perf_load.ts --bin build/backplane --out /tmp/perf.json --diagnostic
```

Options: `--bin` (any branch's build), `--log` (default
`~/.backplane-bend/events.jsonl`, only read), `--port` (3799), `--display`
(`:77`; an Xvfb is started there if none runs, from `/tmp/xvfb-root`,
`/usr/bin` or `~/.local/bin`, see "Xvfb without root" below), `--agents`
(10), `--secs` (20 per phase), `--phases` (any of
`normal,idle,bg,fg,type,open,plane`), `--out` (raw JSON: every phase's
stats, top 25 spans, both perf.json readings), `--shots DIR` (a screenshot
at the end of each phase), `--keep` (keep the temp folder), `--hold` (set up,
start streaming and wait for `<tmp>/stop`, to look around by hand).
One run takes about 4 minutes.

Strict mode exits nonzero when a selected phase misses a target or has no
samples for a required metric. It additionally requires the build receipt,
raw evidence, complete required scenarios, and `--keep`. Use `--diagnostic`
for before/after experiments; those reports never grant acceptance.
Neither historical processing-only typing metric includes time spent waiting
before the window handles an event; these are processing measurements,
not complete input-to-pixels latency. The Status plane is selected by
its sidebar coordinates; include `--shots` when checking UI coverage.

The copied log's provider preference is overridden with the deterministic
stand-in protocol, including bot threads. Before timing begins, every load
thread must deliver parsed streaming deltas. Each streaming phase reports
per-thread delta counts and fails the workload if a thread falls below one
delivered update a second and now also records the longest receive gap
(at most two seconds, including both phase edges). Every local bot in
every posted room must respond during a normal-use run. Zero or dead
streams, unexpected provider processes and failed input commands make a
run invalid regardless of timing targets. `bun test/tools/perf_standin_test.ts`
checks the stand-in's JSON, deltas, subagent and shell events without a hub.
`bun test/tools/perf_guard_test.ts` rejects bursts followed by dead streams,
phase-edge gaps and partial/missing thread-open samples.

Thread opens must be observed through the native client's existing
`desk.look` selection reports, with ten complete timing windows in the
open phase. The same report can establish the uberbot selection, but not
which tab it rendered. Status-tab coverage therefore remains explicitly
unverified and cannot pass strict mode; screenshots remain necessary for
that visual check.

## What it measures

The app runs with `BACKPLANE_TRACE`, so every span is in a trace with its
exact duration. Each phase is bounded by two `/debug/perf.json` readings;
their `proc.up_ms` is the trace's clock, so a phase's numbers are exactly the
spans inside it (percentiles are exact, not log2 buckets). cpu% is the whole
app process (hub and window) from `/proc`; hub% is the time in `hub.*`
spans over the wall time (how busy the hub's loop was); rss is the most seen.

key>frame adds the `ui.msg.win` that handled a key to its `ui.input` (which
starts after the handling, docs/perf.md). Neither has the time an event
waited in the queue before it was handled.

## Phases (run in this order, `normal` printed first)

- `idle`: the window on the longest real thread, nothing streaming: the
  caret's blink and nothing else.
- `bg`: 10 threads stream at once (6 with a subagent writing its own lines
  and a background shell, each with a progress line every 100 ms); the person
  posts in every room that has bots every 4 s, so bots wake and stream; the
  window stays on the long thread (nothing on screen changes).
- `fg`: the window on a streaming thread, its Subagents panel at work.
- `type`: 60 keys typed into that thread's composer, 8 a second, then
  deleted.
- `open`: 10 real threads (the next longest) opened one after another:
  ctrl+k, the title, Return. Only the Return is timed.
- `plane`: the uberbot's Status plane (its row at the top of the sidebar),
  wheel over it.
- `normal`: everything at once, as in a day's use: agents, subagents and
  bots streaming, typing in a streaming thread, scrolling it, opening the
  long thread and scrolling up, typing there, the Status plane, back to a
  streaming thread and typing.

The stand-in agent is a shell script speaking Claude's stream-json (3 text
deltas per 100 ms, every 3 s a finished paragraph, a Bash call and its
result). Threads are found by title through the palette: the long thread and
the 10 to open get unique titles (`zqlong`, `zqopena`..) appended to the copy.

## Safety

- The log is copied. Dropped: `DeviceRegistered`/`DeviceDropped`, hooks,
  peers, wakes, routines, locks, watched work, and push/voice/deploy/google
  settings. Delegated tasks still open are closed, `restart.continue` is off.
- Every path under the user's home is rewritten into the temp folder, so no
  thread's folder is a real one; `git`, `gh`, `claude` (the stand-in),
  `codex`, `grok`, `muse`, `notify-send`... on PATH are the bench's own. No
  repository, no live home, no phone is touched.
- HOME, the lock dir and the deploy locks are in the temp folder; the D-Bus
  address is a dead one.
- It refuses port 3787 and the installed binary, aborts if a child agent is
  not the stand-in, and stops only the PIDs it started (the app, and Xvfb if
  it started one). The stand-ins end when their hub is gone.

## Xvfb without root

```sh
mkdir -p /tmp/xvfb-deb && cd /tmp/xvfb-deb && apt-get download xvfb && dpkg -x xvfb_*.deb /tmp/xvfb-root
```

## Not covered

The web client: it needs the web bundle and a Chromium; not timed here.

Mailroom is an opt-in `--phases mail` workload. It files 5,000 conversations
and 50,000 bodies across six synthetic bots through `mail.send`, with
`mail.on` enabled only in the temporary home. Filing finishes and a list
plus a body read are verified before the timed phase. The phase scrolls
40 rows, types `needle`, opens ten conversations, then sends two mails per
second while the uberbot's Chat tab is selected. `--mail-convs N` and
`--mail-mails N` permit explicitly smaller smoke runs; the report records
those counts, so a smoke run cannot be mistaken for the full fixture.

The JSON includes `mail_read_rpc_ms` (real body-read roundtrips) separately
from `opens_ms` (native input processing through its frame). Native tab
coverage still needs the screenshots: the existing `desk.look` report
identifies the bot's thread but does not expose its tab or open mail.
The Mailroom pane reuses PW with `Mm.mail` as its key. Bot panes bypass
MainM, so the existing `ui.layout.mm.hit` metric does not measure that
pane cache; do not claim Mailroom cache hit coverage from that metric.


## Exact-candidate gate (implementation in progress; no FPS acceptance)

Acceptance runs now require `--build-receipt`, `--evidence`, and `--keep`.
`--diagnostic` can collect incomplete/failing measurements but never authorizes
merge or release. Existing frame p99 8 ms, streaming frame max 16 ms, input
16 ms, opens 50 ms and agent-line p99 2 ms budgets are unchanged.

The supported local path is `scripts/perf-build.ts` (one source-frozen native
build/receipt), `scripts/perf-native-gate.sh` (real ten-stream workload and raw
verification), then `scripts/perf-merge.sh` while holding `merge-main`. The last
command checks i2cjak identity, repository, clean/pinned PR head/base and exact
artifact evidence. Its proposed integration is fast-forward-only: the actual base
must be an ancestor of the measured head, and an atomic remote lease rejects a
concurrent base update. This preserves the exact measured commit/tree rather than
manufacturing an unmeasured merge result. This change in integration procedure is
still under coordinator review; it has been tested only against isolated local Git
repositories. A direct raw Git/GitHub merge remains outside this wrapper;
repository protection/trusted check enforcement is still required. This is not
branch protection, and no live main update has used this draft wrapper.
It currently accepts only Linux window evidence and refuses changed phone,
browser or Windows-renderer paths; it cannot authorize those clients on a
Linux-only result. Their platform-specific native gates remain separate work.

`dev.sh` Linux stage/install, including deploy_main and `--force`, calls the
same native gate. It fails before building if the immutable fixture/Xvfb
configuration is missing, and after building if evidence is missing or fails.
The acceptance build currently bypasses caches to establish a fresh binding;
ordinary developer builds retain their caches. Reusing a prebuilt acceptance
artifact will need validated source/compiler/C/binary provenance, not relabelling
an old binary. Build flags/private bake inputs are recorded only by hashes.
The receipt also authenticates a clean committed tree with the configured builder's
Ed25519 signature. Verification rehashes emitted C, the compiler, Bend dependency
key, compile environment and private bake inputs; merely asserting hashes in JSON
is insufficient. `BACKPLANE_PERF_TRUST_KEY` and `BACKPLANE_PERF_SIGNING_KEY` are
absolute host key paths outside the candidate checkout. No production keys have
been provisioned by this work. A protected signer/verifier and isolation from
candidate execution remain required; paths outside a checkout alone do not provide
that isolation. Pull-request runners must never receive builder signing keys.

Raw spans are checked against the retained trace. Candidate commit, source,
compiler, binary, fixture, harness, host, coverage and run age must match. Required
scenarios are continuous scrolling, input and thread switching during ten sustained
streams over long history, plus rendered feature coverage. Callback/timestamp
counts alone are insufficient: each demand must correlate to the actual drawn
content revision/digest and relevant displayed-region pixels, then to a boundary
acknowledgement for that frame. X-server acknowledgement is distinct from compositor
presentation and physical scanout. Xvfb is a 60 Hz reference workload, not a physical
refresh-rate measurement; simulator/layout results do not certify an iOS device.
Mandatory coverage is fixed by policy even if a caller requests fewer features.
An independently expected demand names its surface and region; presented labels
must match that demand. Timeline pixels cannot also certify rooms, bots, Status
or subagents. Coalesced input credits only features in the actually displayed
prefix, not an earlier feature absent from that prefix. Coverage must lie within
its actual streaming phase window and meet that phase's finite response budget.
Invalid optional content also fails the phase; it cannot borrow valid coverage
elsewhere or use zero/missing refresh metadata to obtain an infinite budget.

The proposed CI Linux build invokes the same native gate through
`BACKPLANE_PERF_ENFORCE=1`; versioned Linux release builds also require it even
when that variable is unset or zero. Configuration is checked before compilation;
the strict verdict precedes packaging/upload. The workflows retain a separate
regression check for malformed/stale evidence; that check does not replace the
native workload. Native evidence includes its raw trace with relative paths so
it survives artifact upload. `dev.sh` revalidates copied packages before requesting
deployment or performing a direct install.
After the completed build, `perf-package.ts dist` signs a manifest of the installed
payload: both binaries, web files, shipped helpers, licenses, desktop entry and icons.
`perf-package.ts seal` repeats strict verification, rejects any subsequent dist
change and reads all installed members from tar, deb and AppImage without executing
them. File contents, complete member sets, regular-file types and read/execute bits
must match. AppImage and deb staging/launcher metadata are compared too; extra deb
maintainer scripts fail. The AppImage runtime prefix remains a packaging-tool input
frozen by the final package hash, not independently measured GUI code or a verified
third-party runtime source. The publish
job authenticates that attestation, candidate commit/tree, original measurement
age and actual downloaded package bytes before publishing. This covers the Linux
native artifacts; it does not establish native performance acceptance for macOS,
Android or iOS. Synthetic archive/signature tests are dispatch and integrity tests,
not measured Backplane workload evidence.
Compiler provenance includes inherited CPATH, LIBRARY_PATH, C/C++/Objective-C include
paths and COMPILER_PATH. Verification must use the build's effective compiler-search
environment, including the X11 paths `build.sh` prepends when BACKPLANE_X11 is set.

Current limitations are explicit: the real drawn-content/ROI observation producer
is not wired yet, so strict runs fail closed on that missing evidence even if their
CPU span summaries pass. These workflow/build changes are unmerged. Hosted Actions
are unavailable in the current project workflow and hosted runners have no configured
fixed-host fixture; the new preflight refuses this configuration rather than producing
a green build-only result. Main currently has no required performance status, and
deferred hub swap revalidation and a protected trusted verifier are still pending.
The supported local gate is the available validation path, not an Actions result.
There is no physical iOS device
currently visible to the Mac. No whole-app FPS fix, green complete workload, deployed
gate, or physical-device performance acceptance is claimed by the helper tests.

The revision contract additionally carries the independently expected relevant
pixel-region SHA256 for each cumulative input prefix. The independent X11
readback tool hashes canonical RGB pixels; matching only a candidate-supplied
counter, semantic tag, changing caret/sidebar, or a different pixel region is
insufficient. A single correctly rendered cumulative prefix may satisfy coalesced
inputs, but each input retains its own injection-to-visible deadline. Credited
content frames, callback samples and physical refresh are separate quantities.
The current readback tool has compiled; live pixel/oracle integration is pending.
