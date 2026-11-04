# One check, two emits: parity passed, no reliable speedup

Do not change the default test runner from this result. This is a bounded
experiment, not an alternative check, proof, workflow, build or release gate.
The full two-minute PR / under-4-GiB goal remains unachieved.

`scripts/test.sh` loads a test twice when it has both `def main` and a matching
JavaScript driver: first `bend TEST`, then `bend TEST -o library.mjs`, then
`bun DRIVER library.mjs`. Stock Bend 2.0.36 accepts two output paths, sharing
one checked book while emitting each form independently. The candidate emits
the `.mjs` first and executable `.js` second, runs the executable with a Bun
launcher that preserves `IO.args`, then runs the original driver.

The experiment changes no compiler, production test, proof, law, admission,
cache pass, workflow or runner. Compiler-memory ownership stays with the
build-times lead. Both actual `clipath_test` IO assertions and its existing
`test/tools/clipath.mjs` driver execute; neither is replaced with a stand-in.
The existing driver has its own isolated POSIX/Cygwin fixtures; this does not
qualify native Windows behavior.

## Complete measurement

Normal Boat job `j20261104044326-1045417`, work
`w-t1793650867-1261555-1536983`, sandbox `bx_zsqbzfk4` (deleted, verified).
Frozen public source **1e34e40673417e8388ce23c15795e5de9ecd214a**, tree
**575db83698781f0c3f54ab32d772afde62308494**; this is a specific historical
commit, not a claim about current main. Test SHA256:
`16e9b5bb59afa3f649e03979e9d760cf07dcbbb8c65b5773355b987319e2bf21`.
Harness SHA256:
`80cd9aca86f51804bb4bc93e221be9d0d891683f647340ac179bbb7f9a94d401`.

| Complete test (compile, IO, driver), seconds | Baseline | Candidate |
|---|---:|---:|
| Pair 1, baseline then candidate | 0.617478 | 0.483873 |
| Pair 2, candidate then baseline | 0.875436 | 0.838844 |
| Pair 3, baseline then candidate | 0.812110 | 1.182783 |
| Median | 0.812110 | 0.838844 |
| Largest command peak RSS, KiB | 173,712 | 194,876 |

This is one shared-host VM, three pairs and a small test. Timings include the
Python guard shim/process launches and parent timeout wait, so short-command
polling and scheduling noise are material. The mixed result does not establish
a speedup; its median is 3.3% slower and peak command RSS is 12.2% higher.
Removing one source check does not guarantee a faster small test because an
additional executable emission and Bun process are required.

The worker's actual cgroup peak was **473,776,128 B**, cap **3,699,998,720 B**;
controller cap **536,870,912 B**. Combined hard caps **4,236,870,912 B**, below
4 GiB. Both swap caps were zero, all recorded max/OOM events zero. Native
`/tmp/bp-bend.lock` was held and independently probed. The worker peak includes
tool setup and is distinct from individual compiler command RSS. Outer GNU
time measures the controller, not the inner compiler. Whole remote command:
42.60s; **60 credit-s / $0.00060**. Source push 2s; setup 1s; sandbox start 2s.

Official Bend archive pin:
`2.0.36 02089dc0eed0fd5fd6d73c74cc9cffcb2c6638dd3b02ae83f7b8cc57fbe381ba`.
Actual executable SHA256:
`b2b183bddc3c3fa4bc45d99f6a6bdbb192ebc616e6e697777f49e74c545b1643`.
Bun 1.3.10 baseline executable SHA256:
`795b37d736a88d746b0e8bb5fcde92bf29bd0f9ac6a1dd1a950fd37440cf5822`.
The official archive independently supplies the expected complete Base/effect
inventory and file modes. Python, Git, GNU time and systemd identities are in
`pilot-v4/HOST-TOOLS.json`. All source, harness, library, fixture and tool
identity checks passed after execution. No experimental compiler was used.

Approved normal daily runner:
`/home/h/Documents/i2cjak/bp-coord/bin/boat-job-day-20261103.sh`, SHA256
`7e3f2d4ffad2d86f0a1bea580ae52e95fde9706016a236b291be98cfba98b3df`.
One SHA-bound bootstrap file was transported with exclude filtering; public
source was fetched blobless on Boat. No runner setting, guard, budget or size
exception was used. All four sandboxes were deleted after their owned jobs.

## Parity and rejection evidence

The final run preflights all generated fixtures before timing and collects
unexpected exits together. The deliberately invalid U32/String fixture fails;
all five positive fixtures compile. Native Bend checks every program.

- All three real-test library emits match exactly, SHA256
  `05d0df32153f9b03382459bb3a2a30f3bcfe9a7aa584f6aec3f6c7290642b080`.
- The executable emitted second matches a separate stock executable emit,
  SHA256 `14c4165da7ef3571a65894881a34e979a2ec05236a74e2742d44cbf9eb29c10c`.
- Both real assertion paths match stdout, stderr and exit separately.
- `IO.args` preserves the original source path, Unicode, spaces, empty values
  and flag-like arguments. The launcher inserts a `--` delimiter before the
  program arguments; it does not claim raw process executable identity parity.
- Actual stdin, environment and cwd-relative file reads match, with over 1 MiB
  output flushed at exit. A 50,000-call structurally decreasing non-tail
  recursion matches; this is not a stack-boundary qualification.
- An `ok` followed by `FAIL`, prior output followed by exit 7, a throwing driver
  and an actual type error fail. A type error creates neither output artifact.
- The same `require_bytes` acceptance function rejects a deliberately corrupted
  executable. This is a live fence control, not merely two different digests.
- Verdicts use the exact replay block extracted from the frozen production
  `scripts/test.sh`; only cost display is stubbed. It neither runs the full
  script nor marks verified/cache passes. Its source and excerpt hashes are
  recorded in `PRODUCTION-VERDICT.json`.

Twelve current-source test files match the runner's IO-main-plus-driver branch;
`ELIGIBLE.json` is inventory only. No other eligible test ran. Raw
`process.execPath` / `process.argv` metadata, other tests and deeper stack
boundaries remain unqualified. In particular this does not justify global
promotion or a claim that a full test job fits under 4 GiB.

## Failed attempts, preserved without promotion

| Job suffix | Outcome | Whole command | Paid cost |
|---|---|---:|---:|
| `j20261104043509-986353` | Three real pairs passed; later argv fixture had invalid bind syntax; missing final identity fence | 16.32s | $0.00024 |
| `j20261104044013-1019828` | Preflight argv fixture incorrectly used reusable String.join on affine IO.args; no timed pairs | 12.24s | $0.00018 |
| `j20261104044153-1032459` | Preflight depth fixture matched a computed value; no timed pairs; final identities unchanged | 12.62s | $0.00020 |
| `j20261104044326-1045417` | Complete corrected scoped parity/control run passed; mixed timings, no demonstrated speedup | 42.60s | $0.00060 |
| Total | 122 credit-s | | **$0.00122** |

The first three are not successes. Their frozen harnesses, raw failure logs,
commands and available identities are retained under `pilot-v1` through
`pilot-v3`. The first result's attractive partial timing is not the qualified
benchmark and must not be substituted for the complete result. Complete raw
artifacts remain in each owned Boat job directory; committed JSON contains
per-command stdout/stderr hashes, exact inputs and resource receipts.
