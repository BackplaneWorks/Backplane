# The simulation token benchmark

What a simulation task costs an agent in Backplane, measured with real turns (`test/ksim_bench`, whose README says how
to run it): eight tasks a person could ask for, each on a fresh copy of a generated KiCad project that is a board's
schematic and not yet a simulation (connectors, mounting holes, a test point, footprints, ERC clean; no `Sim.*` field,
no source). A run passes when its answer's numbers are within tolerance of the reference **and** the saved schematic
is still the board's and now simulates. It is the sibling of [the KiCad-IPC benchmark](kicad-ipc-bench.md) and uses its
harness.

The baseline is in "Result": a hub from main with raw `kicad_ipc`, `kicad_edit`, file edits and the shell; no
`kicad_sim`, no simulation skill. Each variant of ops and skills goes beside it.

## Tasks

Every prompt ends with the same request: set the schematic itself up for simulation and save it (every part that
simulates has what it needs, parts that do not simulate are left out, what is added only for simulation stays out of
the BOM and off the board, the circuit otherwise unchanged), and end with a line `RESULT key=<number> ...`.

| task | fixture | what is asked | reference | tolerance |
|---|---|---|---|---|
| `lc_corner` | `lcfilt`: a class-D output filter, 22 uH into 680 nF with a 15 ohm + 220 nF Zobel | -3 dB frequency and peaking with 8 ohm on the output | 51,793 Hz; 3.249 dB | 2 %; 0.15 dB |
| `opamp_gain_pm` | `preamp`: a single-supply non-inverting stage (12k / 10k, 4.7 nF on the output), op-amp BP321 whose model is in the project's `models/`, not assigned | gain at 1 kHz and the loop's phase margin | 2.19995 V/V; 40.11 deg | 1 %; 3 deg |
| `ldo_transient` | `ldo`: an adjustable regulator BP758 (model in `models/`), 5 V to 3.3 V, 10 uF out | a 10 mA to 200 mA load step in 1 us: the dip, the time to stay within 5 mV, and the loop's phase margin at 200 mA | 83.77 mV; 38.2 us; 32.55 deg | 5 %; 10 %; 3 deg |
| `buck_ripple` | `buck`: a TPS54331 converter, 12 V to 3.3 V, 6.8 uH, 220 uF with 60 mohm (said in the prompt) | output ripple at 2 A with the manufacturer's model, which the agent is to get | 41 mV | 15 % |
| `buck_blocked` | `buck_mp`: a converter around an MP1470 | the same question; the maker has no SPICE model of the part | the agent stops and asks | no number, no model put in its place |
| `divider_tol` | `vsense`: 180k / 22k from a 24 V bus with 220k to 3.3 V, tolerances in each resistor's `Tolerance` field | the lowest and highest ADC voltage over the tolerances | 2.624514 V; 2.716016 V | 0.2 % |
| `tune_corner` | `aafilter`: a two-stage RC low-pass whose second capacitor is a placeholder | the E24 value that puts the corner nearest 4 kHz, written into the schematic | 3.9 nF; 4,079.8 Hz | 1 %; 1.5 %; the schematic's value |
| `noise` | `preamp` | output noise, 20 Hz to 20 kHz, input shorted | 3.867 uV RMS | 3 % |

A start-up transient or an S-parameter task was not added: the eight already cover AC, transient, operating point with
overrides, noise, a loop gain, a tuned value, a model from the project, a model from the manufacturer and a model that
cannot be had.

## References

Each is derived twice (`bench.py ref`), and the two must agree with `tasks.json` to a quarter of the pass tolerance:

- from the component values alone: a formula, or the circuit's equations solved by `mna.py` (a 90-line nodal solver);
  no KiCad and no ngspice in the path;
- by ngspice (the runtime's own `libngspice`, driven by `tools/ngs.py`) on the SPICE netlist KiCad's exporter writes
  from the solved fixture, so the fixture, its `Sim.*` fields and the exporter are all in the path.

| task | from the values | ngspice on KiCad's netlist |
|---|---|---|
| `lc_corner` | 51,792.9 Hz; 3.24911 dB (closed form) | 51,792.9; 3.24898 |
| `opamp_gain_pm` | 2.19995; 40.1059 deg (return ratio of the op-amp's transconductance) | 2.19995; 40.1059 (loop opened at the inverting input, 1 GH / 1 GF) |
| `ldo_transient` | margin 32.5457 deg (the loop linearised by hand at 200 mA) | 83.7651 mV; 38.225 us; 32.5485 deg |
| `buck_ripple` | 40.6 mV (textbook: ripple current times ESR, plus the capacitor's charge ripple, at the datasheet's 570 kHz) | 43.54 mV at 571.4 kHz, 3.312 V |
| `divider_tol` | 2.624514; 2.716016 (closed form at the eight corners) | the same to 12 digits |
| `tune_corner` | 3.9 nF, 4,079.78 Hz (next best 4.3 nF, 3,859 Hz) | 4,079.79; 3,859.42 |
| `noise` | 3.8669 uV (each 4kTR and 8 nV/rtHz through its transfer, integrated) | 3.86697 |

The two with no closed form are reference simulations, and their tolerance comes from how far the number moves:

- `ldo_transient`: 83.68 to 83.77 mV and 38.1 to 38.25 us from a 10 ns to a 1 us step limit, with `reltol` 1e-5 and
  with Gear. The ringing's extremes are -76.1, +26.1, -9.95, +3.64, -1.35 mV about the new value, so a 5 mV band is
  left for good between the third and the fourth (a 10 mV band would sit on the third: 25.5 or 34.2 us by the step).
- `buck_ripple`: 42.4 to 44.9 mV from a 20 ns to a 2 us step limit; 43.1 over 10 ms; 43.6 with another Schottky model
  and 47.4 with ngspice's default diode. That is the peak-to-peak over the last millisecond, which holds a few mV of
  the model's cycle-to-cycle wander in ngspice; one cycle's ripple is 38.3 to 38.9 mV in every run. Both answer the
  prompt, so the reference is 41 mV with 15 % (34.9 to 47.2 mV): both readings pass, ngspice's default diode does not.
  It was 43.5 mV with 12 % for the baseline and was recentred after it (2026-11-17); the baseline's three runs
  (39.0, 39.8, 40.4 mV) pass under both, and the prompt is unchanged.

The two op-amp and regulator models (`test/ksim_bench/models/BP321.lib`, `BP758.lib`) were written for the benchmark
and say so; they are no manufacturer's part, so their numbers can be derived from first principles and nothing of a
manufacturer's is kept in the repository.

### The manufacturer's model (`buck_ripple`)

Texas Instruments' "TPS54331 Unencrypted PSpice Transient Model Package (Rev. A)",
<https://www.ti.com/lit/zip/SLVM659>, linked from <https://www.ti.com/product/TPS54331>: a plain download, no sign-in
and no licence click (checked 2026-11-17 with a bare HTTP client; `mkfix.py` keeps the URL and the file's sha256 and
says when the file has changed). KiCad's library has no TPS54331 symbol; the fixture uses `Regulator_Switching:TPS54233`,
the same pinout, with the value TPS54331. What getting it to run in ngspice takes, all of which the agent has to find:

- it is a PSpice model: ngspice needs its PSpice compatibility (KiCad's default for a project with no workbook);
- its library names a diode model `MBR340` that it does not define (PSpice's own library has it);
- it does not get through ngspice's default tolerances ("time step too small" within 0.1 ms): it runs with Gear and
  `reltol` 0.01 (also with `abstol` 1e-7, `vntol` 1e-4, `itl4` 500), not with either alone, not at `reltol` 0.005;
- with KiCad's current probes in the netlist (`.probe alli`, the simulator's default) it stops at 56 us even so:
  `RunSimulation` needs `options.save_all_currents = false`;
- the capacitor's 60 mohm is the ripple, and a KiCad capacitor has no series resistance: it takes a subcircuit on C2
  (a resistor drawn in series would change the board).

Parts tried and not used: TI's LM2596-5.0 model (<https://www.ti.com/lit/zip/SNVMA61>) loads and runs with ngspice's
defaults but is wrong there without any error: `I_U2_I1 ... DC 1Adc` is read as 1 attoampere, the oscillator's reset
is cut short, and it switches at 470 to 640 kHz instead of 150, with a ripple between 10 and 25 mV by the step limit.
The TPS562201 model did not get past a few microseconds with any setting tried.

### The model that cannot be had (`buck_blocked`)

The brief was a part whose only model needs a sign-in. That could not be confirmed from this machine: Renesas, ROHM and
ST answer both a bare HTTP client and Backplane's browser with a bot wall, and Richtek's page showed no model. What was
confirmed (2026-11-17, in Backplane's browser): Monolithic Power Systems' MP1470 page offers only an "MPSmart Model"
for its own simulator, which starts downloading without a sign-in; there is no SPICE model. So the variant as built is
"no usable model exists", which is a stop-and-ask case all the same. Its check: no number in the `RESULT` line, the
answer says why (a sign-in, or that it could not get or find the model), and U1 has no model assigned. To make it a
true sign-in case, name a part in `mkfix.py`'s `buck_mp` whose model page a person has seen ask for a sign-in.

## The check

`checks.py`, written before any run. The answer: the last `RESULT` line, each number within tolerance. The design:

- every part the fixture had is there, with its symbol, its value (the tuned capacitor: the reference value), its
  BOM, board and do-not-populate flags, and its connections (the pins of the fixture's parts are joined as before,
  whatever the nets are called): a loop opened or a resistor drawn in series for a measurement and left there fails;
- every part that was added is out of the BOM and off the board;
- KiCad's own SPICE netlist of the saved schematic (`kicad-cli sch export netlist --format spice`) has no part without
  a model, leaves out the connectors, the test point and the mounting holes, has every other part, has a source, and
  names the task's model on the right nodes (each port of the subcircuit on the node its pin is on);
- that netlist loads in ngspice, and for the linear tasks finds its operating point.

`bench.py e2e` proves the check both ways for every task: the task done by hand passes, a number 25 % off fails, the
untouched fixture fails (16 to 20 findings).

## The tasks by hand over raw `kicad_ipc`

`oracles/<task>.py`: each task done with no model through a hub from main (015bb7ed5; 4496e22d2 before the history rewrite of 2026-11-17) on the fork lane's runtime
(`sim-run`, e311092569): parts left out of simulation and models assigned with `AssignSimulationModel`, sources and
loads placed with `CreateItems` (out of the BOM and off the board), saved, run with `RunSimulation`, and held to the
same check. All seven pass, and their numbers are the references' (the eighth, `buck_blocked`, has nothing to do).
What it took, which the baseline agent meets too:

- a loop gain cannot be had from `RunSimulation`: its overrides change values, not connections. The two margins are
  taken from `GetSimulationNetlist`'s netlist, the loop opened in the text and ngspice run on it from the shell;
- a symbol without pins (a mounting hole) is not a symbol to the simulation commands (`AssignSimulationModel`: "the
  schematic has no symbol H1"; `GetSimulationModels` does not list it), while `kicad-cli`'s SPICE export writes it as
  a part without a model (`H1 __H1`): it is left out with an ordinary `UpdateItems`;
- the hub on main refuses the runtime as staged: its `manifest.json` says 10.99.0-bp.13.1 and the program says
  bp.13.2 ("this KiCad is ..., not the 10.99.0-bp.13.1 Backplane installed"). The runs here use a folder of links to
  the runtime with that one line of the manifest corrected;
- a hub serves two projects' KiCad at a time, and a project whose folder was deleted keeps its place.

## What is measured

As the IPC benchmark: context tokens (input, cache writes and cache reads over the turn's requests), output tokens,
tool calls, wall seconds, the CLI's cost, pass. And per run the web fetches (the CLI's fetch and search, the hub's browser, a shell command that asks a web server), the simulations, ngspice's seconds and the model lookups,
from the hub's `<home>/logs/sim.jsonl` once the hub writes it; until then `simulations` counts the `RunSimulation`
calls and the shell commands that ran ngspice. A run cut short by the account's limit or an API error is marked,
left out of the table and counted under it.

## Result

### Baseline

Raw use: the hub as on main at 015bb7ed5 (4496e22d2 before the history rewrite of 2026-11-17; no `kicad_sim`, no simulation skill; `kicad_ipc` with the fork's simulation
commands, `kicad_edit`, the CLI's file and shell tools, the web), Backplane KiCad 10.99.0-bp.13.2 from the fork at
9a500f789a (`sim-run`). `claude-fable-5-2`, medium effort, 3 runs a task, medians, a 30 minute turn cap (2026-11-17).
No run was cut short. The 24 runs cost $50.65.

| task | passes | context tokens | output tokens | tool calls | wall s | cost $ | simulations | web fetches |
|---|---|---|---|---|---|---|---|---|
| lc_corner | 3/3 | 1,016,016 | 8,767 | 18 | 257 | 1.61 | 3 | 0 |
| opamp_gain_pm | 3/3 | 891,461 | 10,332 | 14 | 198 | 1.58 | 3 | 0 |
| ldo_transient | 3/3 | 1,229,526 | 11,729 | 20 | 199 | 1.83 | 4 | 0 |
| buck_ripple | 3/3 | 5,542,282 | 30,046 | 59 | 860 | 4.99 | 8 | 6 |
| buck_blocked | 3/3 | 1,472,142 | 10,351 | 27 | 316 | 1.92 | 1 | 4 |
| divider_tol | 3/3 | 842,052 | 7,360 | 14 | 140 | 1.37 | 2 | 0 |
| tune_corner | 3/3 | 699,576 | 6,382 | 12 | 128 | 1.31 | 1 | 0 |
| noise | 3/3 | 893,201 | 8,345 | 15 | 228 | 1.67 | 3 | 0 |
| **all (sum of medians)** | 24/24 | 12,586,256 | 93,312 | 179 | 2,324 | 16.27 | 25 | 10 |

Simulations are the `RunSimulation` calls plus the shell commands that ran ngspice (the hub has no simulation log
yet, so no ngspice seconds and no model lookups). No run failed, so there is no list of reasons. One pilot run of
`buck_blocked` was first scored a failure by the check, which then asked for the word sign-in; the agent had stopped
and asked, with no number and no model in the part's place, and the run is scored again with the check as it is now.

What the runs did (tool calls over the three runs of a task):

| task | shell | `kicad_edit` | `kicad_ipc` simulation commands | other |
|---|---|---|---|---|
| lc_corner | 26 | 23 | 0 | 2 |
| opamp_gain_pm | 29 | 19 | 13 (7 AssignSimulationModel, 3 RunSimulation) | 6 |
| ldo_transient | 27 | 26 | 9 (5 AssignSimulationModel, 1 RunSimulation) | 6 |
| buck_ripple | 89 | 27 | 23 (13 AssignSimulationModel, 3 RunSimulation, 3 GetSimulationResult) | 16 |
| buck_blocked | 38 | 14 | 0 | 37 (the browser 20, web search and fetch 6) |
| divider_tol | 19 | 21 | 0 | 1 |
| tune_corner | 18 | 17 | 0 | 2 |
| noise | 28 | 22 | 11 (4 AssignSimulationModel, 3 RunSimulation) | 5 |

- The pass rate is not the room to climb: every run passed. The price is: 0.55 to 7.3 million context tokens a
  task, 10 to 65 tool calls, two to sixteen minutes.
- The shell does most of the simulating. Four tasks never used a simulation command: the agent wrote a driver for the
  runtime's `libngspice` (as `tools/ngs.py` is) and ran KiCad's exported netlist. Where the simulation commands were
  used it was for the model (`AssignSimulationModel`) more than for the run (`RunSimulation`: 10 calls in 24 runs).
- `kicad_edit` carries the schematic work in every task: 14 to 27 calls a task over three runs, for the sources, the
  load, the flags and the exclusions.
- `buck_ripple` is a third of the whole: finding the solver options the manufacturer's model needs ("Try four solver
  option sets in parallel"), the undefined `MBR340`, the capacitor's series resistance as a subcircuit.
- `buck_blocked`: all three stopped and asked, after 25 to 34 calls spent finding out that the maker's only model is
  encrypted for its own simulator.

`buck_ripple`'s reference was recentred after these runs (see "References"): the three runs answered 39.0, 39.8 and
40.4 mV, the first of them the ripple of one cycle, and pass under the old band and the new.

### `kicad_sim` against raw use, on Antigravity `gemini-4-neon`

The same eight tasks on one model with and without the tool (2026-11-19 and 20): Antigravity's `gemini-4-neon` (the
CLI's medium level) through the person's own `agy` sign-in (`KIPC_BENCH_PROVIDER=antigravity
KIPC_BENCH_THREAD_MODEL=gemini-4-neon`, the `agy` wrapper of `test/kipc_bench`), released Backplane KiCad
10.99.0-bp.13.2 (edfc943200), 3 runs a task, medians, a 30 minute turn cap.

- **sim**: a hub from main with `kicad_sim` (15 runs on d9ecbb5e6d, 9 on 335f784238, which differs in three lines
  of `ksim_tool.rs`).
- **base**: a hub from main at 37a271bec, before `kicad_sim`: `kicad_ipc`, `kicad_edit`, the CLI's file and shell
  tools, the web.

| task | context tok sim | base | output tok sim | base | tool calls sim | base | wall s sim | base | passes sim | base |
|---|---|---|---|---|---|---|---|---|---|---|
| lc_corner | 355,767 | 1,963,090 | 8,563 | 21,231 | 13 | 41 | 199 | 324 | 3/3 | 3/3 |
| opamp_gain_pm | 235,029 | 967,381 | 7,208 | 19,378 | 10 | 30 | 120 | 319 | 3/3 | 3/3 |
| ldo_transient | 604,525 | 1,463,589 | 16,725 | 24,605 | 19 | 30 | 258 | 268 | 3/3 | 3/3 |
| buck_ripple | 862,233 | 3,140,602 | 18,179 | 40,611 | 28 | 52 | 507 | 793 | 3/3 | 3/3 |
| buck_blocked | 97,473 | 255,195 | 3,465 | 3,887 | 8 | 16 | 64 | 75 | 3/3 | 3/3 |
| divider_tol | 230,551 | 1,252,019 | 5,906 | 14,130 | 7 | 26 | 74 | 197 | 3/3 | 3/3 |
| tune_corner | 168,177 | 1,671,559 | 4,540 | 16,446 | 7 | 32 | 79 | 272 | 3/3 | 3/3 |
| noise | 554,279 | 1,368,111 | 10,287 | 20,878 | 22 | 33 | 203 | 281 | 3/3 | 3/3 |
| **all (sum of medians)** | 3,108,034 | 12,081,546 | 74,873 | 161,166 | 114 | 260 | 1,503 | 2,529 | 24/24 | 24/24 |

With the tool: 74 % fewer context tokens, 54 % fewer output tokens, 56 % fewer tool calls, 41 % less time. Over the
24 runs the shell commands fell from 345 to 77 and the `kicad_edit` calls from 147 to 55; `kicad_sim` was called 129
times. From the hub's simulation log (sim only; base has none): 27 simulations, 173 s of ngspice (all but 0.3 s
`buck_ripple`'s) and 67 model lookups, as sums of the tasks' medians.

What these numbers are and are not:

- Not comparable with the baseline above, which is `claude-fable-5-2`: this is one model against itself. Context is
  the input tokens not read from the cache plus those read from it, summed over the turn's requests, as `agy`'s
  `result` line gives them. No cost: the sign-in is not billed by the token (the 48 runs and the discarded ones took
  the account's weekly Gemini allowance from 89 % to 85 % left).
- Antigravity does not hold the MCP tools' definitions in its context: it reads a tool's definition from a file
  when it wants it (`~/.gemini/antigravity-cli/mcp/backplane/<tool>.json`), which is part of why its context is
  smaller than the Claude CLI's on the same task.
- Runs left out and made again: nine sim runs (`noise` 2 and all of round 3) ran with no Backplane MCP server
  ("server name backplane failed to load"), because the hub's program was a main-builder build that was removed
  while the benchmark ran; they passed all the same, through the shell, at the base's cost (the sum of medians with
  them in was 4.2 million context tokens). Two base turns that were out when their hub was killed (a restart of the
  person's Backplane ends the benchmark's jobs) were dropped and the round made again. None of the 48 runs in the
  table has a failed MCP load. Run the benchmark from a copy of the hub that nothing removes.
- It ran on the desktop, where the sign-in is; nothing was compiled for it. `agy` runs with the person's HOME: the
  conversations are kept in their `~/.gemini`, their `pcbparts` MCP server is in the agent's reach, and the tool
  definitions Antigravity keeps there are rewritten by whichever hub ran last.
