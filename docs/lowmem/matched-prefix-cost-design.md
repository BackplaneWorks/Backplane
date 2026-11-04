# Matched same-module / prefix cost test: design only (lane 3). No job has been run.

Question: does the cost of checking the SAME module M grow with how much of the program was loaded
before it? Closure-size differences between different modules are NOT evidence of growth (they compare
different code); nothing here assumes or reports a growth law.

## Matched design
- Pick M from the real tree. Fix an ordered list of prefix modules P_k that M does not need (or that it
  imports in a fixed topological order): P_0 = {}, P_1 ⊂ P_2 ⊂ P_3 ... (nested, by prepending modules in a fixed,
  recorded order, imports never reordered between runs).
- Measure R_k = check of `import P_k...; import M` (entry file generated, `--check-only`) and S_k = check of
  `import P_k...` alone. Marginal cost of M after prefix k = R_k - S_k (wall and peak), differenced on identical
  inputs. The control is k = 0 (M alone) and R_k - S_k computed with M placed LAST in both.
- Decoys: the same prefixes with a byte-different but equal-size dependency to see whether the effect tracks size
  or content; and M placed first vs last with the same prefix to expose order effects (fill order matters).
- Repetitions: n >= 3 per cell in a randomised order, each in a fresh process; report every run, not a mean only.

## Binding (every run records)
exact source sha256 of every file in the entry's closure, the generated entry file bytes, the prefix list, the
`bend` binary sha256 and version, env (`BEND_NO_TELEMETRY=1`, stack and heap settings as in proof-shards
execute), the host/VM, and the git head. Inputs are materialised read-only; a hash is re-checked after the run.

## Observation (observation-only, with limits)
Under the existing admitted path (heavy-guard profile, native flock, own cgroup memory.max <= 3,700,000,000 and
swap 0): process wall time, `/usr/bin/time` peak RSS, cgroup `memory.peak`, `memory.events` (oom, oom_kill), exit
status and the bendok verdict. Phase cost only where an existing instrumented build already provides it, and only
if its output is shown byte-identical to the stock build's verdict/output on the same inputs; its overhead is
reported from paired stock vs instrumented runs. Otherwise phases are NOT claimed. A run that hits the cap is a
data point (killed at X), not discarded.

## Reporting
Raw table of every run (cell, repetition, wall, RSS, cgroup peak, oom, verdict), the differenced marginal costs
with their spread, and the reproducer command. Fit nothing until at least five prefix sizes exist; describe the
fit's inputs and refuse to name an exponent from fewer. No claim about quadratic or other growth from closure-size
differences between different modules.

## Safety and ownership
One campaign at a time under one admitted executor; a new heavy-guard profile only by explicit narrow contract
(exact argv) with the CI owner; no shared signature or kernel edits; no upstream publication from this lane.
