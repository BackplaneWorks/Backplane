# PR #932 follow-up for independent re-review

Release status: **blocked**. These receipts support functional changes after
the exact-c25c9959 postmerge REQUEST CHANGES. They are not release acceptance,
a full-hub latency result, or permission to merge this follow-up.

The tested source is based on merged #932, commit
`397cb47edeb9a5eb4f3eabf44b3be7d56b936591`. The accompanying source-sha256.json
matches the passing Boat manifest for every listed source file. Receipt-only
files were added afterward. The frozen follow-up commit is identified in the
review handoff and PR, avoiding a self-referential commit hash here.

## Changes

- The CAD effect awaits filesystem promises for directory creation, staging,
  FreeCAD command reads and recursive cleanup. Terminal state follows cleanup;
  Retry and terminal Dismiss cannot race it. Cancelling a reservation before
  setup creates no files. Active cleanup stays visible and accepts Cancel.
- The hub reserves a new attempt and publishes `starting` before launching
  platform/setup work. Retry replaces the old terminal snapshot immediately.
  The consumer rejects obsolete captured snapshots and all further messages
  after a terminal result or dismissal. Per-phase byte counts remain monotonic.
- All controls carry the attempt token. Active Dismiss is rejected; terminal
  Dismiss removes the effect record and clears visible snapshot/error fields,
  retaining only a generation tombstone. A new attempt gets a new token.
  Active snapshots remain in Cfg and Hub.info across view changes and joins.
  CAD notices no longer write a persistent hidden setting.
- Every error goes through one producer clamp: at most 2,048 escaped UTF-8
  JSON bytes, with lone surrogates normalized. Each producer state is asserted
  at most 4,096 UTF-8 bytes. The consumer reconstructs known fields only and
  clips errors to 256 scalar values. The CAD-specific two-tool progress
  envelope is capped at 16,384 UTF-8 bytes **after** existing Info.json masking.
  This does not bound unrelated data in ordinary join/info frames.
- Sampling is at most **4 Hz per tool**, potentially **8 Hz together**;
  immediate reservation/dismissal control transitions are separate. Unchanged
  sampled counters/phases are suppressed. This is not a global 4 Hz claim.

## Disposition of the inherited Vault boundary

Info.json masking is preserved. Its inherited VaultFx.values / bp_vault_api
freshness path can still synchronously stat api.env once per second and read
it on change. No zero-sync-progress-tick claim is made. This separately owned
boundary, Hub actor queueing, actual broadcast fan-out, full application
reconnect/rendering, and packaged KiCad/FreeCAD installs remain unqualified.
The coordinator and independent reviewer received this limitation explicitly.
No author latency measurement, full build or deployment was performed.

## Functional receipt

Boat job `j20261104161844-3666589`, default sandbox `bx_hzneqaeh`, exit 0.
Kernel memory.max `3699998720`, memory.swap.max `0`, peak `704577536` bytes,
no OOM events. Sandbox deletion was verified by the Boat runner.
Command: `scripts/test-cad-install.sh --focused`.
Bend 2.0.36 was SHA256 verified; the tool bootstrap pins Bun 1.3.10.
The Node VM fixtures are functional tests, not real-Bun latency evidence.
The emitted Bend ready/error/cancel fixture uses the production async loop
under Bun. API check-only reports that foreign effect declarations are
unproven; the repository's existing bend_verdict accepts that specific report
as a type-check result. It is not proof of foreign JavaScript behavior.

Passing coverage includes slow Content-Length transfer, speed/ETA, unknown
length, HTTP error/Retry, stalled headers, checksum failure, download Cancel,
captured child cancellation, early reservation Cancel, terminal Dismiss,
stale controls, escaped error/state/envelope limits, async filesystem lifetime,
and a deterministic cleanup-await gate. Five handler checks, platform-script
regressions, core assertions, and byte-identical scoped law/config/UI/handoff
declarations pass. The handoff test injects a captured A snapshot after B is
accepted at the real Cfg consumer; it is not a full Hub transport simulation.
See individual logs and results.json for exact commands and verdicts.

## Canonical proof remains incomplete

Required current-source `bend PROOF.bend` was attempted separately on Boat,
job `j20261104162112-3690577`, sandbox `bx_ycyxfvya`. The kernel killed it
with exit -9 at memory.max `3699998720`, swap 0, oom_kill increment 1.
The collector exited 1; sandbox deletion was verified. This is **not a pass**.
The new law and existing feature laws passed only their byte-identical scoped
check. No law was weakened or deleted. Full proof/build acceptance remains open.

Earlier failed functional packets are retained in their own job directories:
`j20261104161016-3585752` (unsupported literal),
`j20261104161327-3616653` (missing counter type), and
`j20261104161523-3637991` (fixture expected terminal before async cleanup).
Historical receipts outside this folder describe earlier source and have not
been relabeled as current-source passes.

## Effect API for the independent adapter

All arguments are String. `reserve(home, tool, attempt) -> IO(Bool)` uses
the registered True/False tags. Attempts are positive decimal tokens.
`start(home, tool, attempt, spec) -> IO(String)` requires the reservation.
`next(home, tool, attempt) -> IO(String)` parks via the production
io_park_promise continuation bridge. `cancel(home, tool, attempt)` and
`dismiss(home, tool, attempt)` return IO(Unit).
`frame(masked) -> IO(String)` receives already-masked progress JSON.
The independent reviewer owns the real-Bun component probe and its protocol;
no component result may qualify the excluded full-hub boundaries above.
