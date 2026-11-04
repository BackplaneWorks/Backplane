# Explicit approved-runner qualification

The required normal daily runner predates `--include-file`. The premerge
controller now accepts an explicitly reviewed absolute runner path, SHA256
and `exclude` transport, with no fallback or budget/guard changes. It executes
a private byte snapshot, rechecks the original after the job, and requires
the actual owned upload manifest to contain only the exact pushed worker.

Initial qualification: work1529849 / Boat`j20261104041114-793634`, exact head
`7127f7f5c7df788b5f93e5283a454cd7bf5119e4`, base
`5cbf389a8ba80084625f55314e5e6e688cce0df3`, actual merged tree
`ade5669174895e2b05bd8f81e9065d34cea73750`. All26 controls passed0.254s,
mandatory BASE-pinned merged shape passed, and the full owned controller
completed0. The intentional guard update used the exact-head acknowledgement
and returned `passed-reviewed-guard-change`, never plain `passed`.

Worker9.14s,26,240KiB process peak RSS; actual cgroup peak41,091,072B under
536,870,912B, swap0, all memory events0. Job16credit-s/$0.00016; default
sandbox deleted verified. End-to-end controller time was not isolated; this
is not a full-PR latency or build/proof benchmark.

The selected runner and executed snapshot both have SHA256
`7e3f2d4ffad2d86f0a1bea580ae52e95fde9706016a236b291be98cfba98b3df`.
This is the coordinator-approved normal daily runner, independently confirmed
by lane4's byte audit and partial boundary review, not a full runner audit.
It is neither the retired xlarge nor box100 runner. No source replacement,
size exception, account/budget/start-limit change or live setting was used.
The owned actual manifest contains exactly one worker entry, SHA256
`e03408c59458188da25ae189d8727afbfddbf16c67f562604dc2cd0544e865f7`.
Raw result, control output, controller receipt, manifest and provenance are
preserved here. Two toy job headers in controls.txt belong to the deliberate
ambiguous-identity unit fixture; they were never actual paid executions.

The controller receipt is local and unauthenticated, not an independent
execution attestation. Candidate-dispatcher trust still requires reviewing
the closed guard-source set; trusted-base `--head` mode remains unimplemented.
No compiler, law/proof, app/build/install/release or full CI qualification is
implied. No local tests, shape checks or merge-tree ran. The later receipt-only
commit must pass its own fresh Boat guard before normal merge.
