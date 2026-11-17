# kicad_sim: a simulation step in one call

`kicad_sim {path, ops, save?}` simulates a schematic with KiCad's own simulator (ngspice) and answers numbers. It is
`kicad_edit`'s sibling ([kicad-edit.md](kicad-edit.md)): the same lock, gate and save rules, one call per meaningful
step. The driver is `native/host/src/kicad/ksim.rs`, the MCP glue `ksim_tool.rs`; measurements and compaction are
`ksim_wave.rs`, values, the tune search, model files and failure lines `ksim_parts.rs`, the log `ksim_log.rs`.

It needs a Backplane KiCad with the capability `sim-run` (bp.13.2, the fork's `kiapi.backplane.sim`). On an older
KiCad the tool is not listed and the prompt does not mention it, so it costs an agent nothing; a call made anyway
answers "this KiCad has no simulation (it is 10.99.0-bp.12.1): kicad_sim needs Backplane KiCad bp.13.2 or newer".

The hub:

1. refuses a writing call while the person works in this KiCad, as `kicad_edit` does (`person_seen: true` repeats it);
2. opens the schematic if it is not open (never in the person's KiCad: that is theirs to open);
3. runs `ops` in order. A model, an exclusion or a saved analysis is KiCad's own commit (one undo step each); a
   source, a probe's label and a tune's written value are a nested `kicad_edit`;
4. saves what changed (through the save gate, committed to main like any save);
5. answers `{results: [one per op], saved, left}`. `left` is what still would not simulate after a model or an
   exclusion changed.

A run waits inside its op: `RunSimulation` answers `SIM_RUNNING` for a run that takes longer than a second and the
hub collects it with `GetSimulationResult`, a second at a time, up to the op's `timeout` (seconds, 120 by default,
3600 at most). `RunSimulation` and `StopSimulation` are a class of their own (`ipc::is_sim`), not reads:

| Where | What a simulation command does there |
|---|---|
| The document lock (`design.rs` `kicad_gate`) | Takes none: a run changes no document, whoever holds the schematic. |
| A review turn (`snapguard::review_allows`) | Allowed, like a check. |
| The person paused the agent in KiCad | `RunSimulation` waits for Resume, like a write (it may show in their Simulator); `StopSimulation` does not wait. |
| A hub on a copied log | Refused, like any command that is not a listed read. |
| KiCad busy with a task | Not asked again by the hub's read path; `ksim` waits it out itself. |
| `StopSimulation` | Only for a run the same thread started (the hub keeps each run's thread by project and run id); the person stops any run in KiCad. |

`GetSimulationResult` and the other `Get*` simulation commands are reads.

### Long runs

- **What a long run holds.** Its own project's KiCad, and nothing else of the hub: each poll is one short request, so
  other projects' calls go on, and the same project's other calls wait a second at most behind a poll.
- **The time limit always ends it.** KiCad stops the run itself at `timeout` and answers `SIM_TIMEOUT`. If it has not
  15 s later the hub sends `StopSimulation`, and 45 s later the op fails saying how long the run was in and how far.
  A poll that gets no answer is not waited for longer than 35 s. The failure says `stopped after 300 s at 42% of the
  run` and what to do: a longer `timeout`, or a shorter run.
- **An interrupted turn stops its run.** When the agent hangs up on a `kicad_sim` call (its turn was interrupted),
  the hub drops the call (`mcp.rs`), and a run that is out is stopped with `StopSimulation` (`ksim::Running`).
  `sim_e2e.py stop` checks it with the stand-in agent: the next turn's run is not refused as busy.
- **The server pool** (`docs/kicad-ipc.md`): two headless KiCads, three from 32 GB of memory, four from 64 GB. A
  server at work is never the one stopped to make room, so a long run is not lost to another project; with every
  server at work a further project waits for room, up to 120 s.

### Memory

A run is bounded in what the hub reads, not in what ngspice computes.

| Bound | Value | What it does |
|---|---|---|
| Points read of each vector | 200,000 (`max_points`: up to 2,000,000) | KiCad thins the rest (every Nth point). The answer's `thinned` says how many of how many were read; summaries and measures are of those, so a peak between them is missed. |
| Vectors read | the op's `signals` and what its measures name | Nothing else is sent to the hub. |
| A transient's currents and powers | not saved unless a signal or measure reads one (`I(...)`, `P(...)`, a `#branch`) | `options: {save_all_currents: true}` keeps them. |
| `tstart` on a `.tran` run | `{run: ".tran 1u 10m", tstart: "8m"}` | ngspice stores nothing before it: the way to measure the end of a long run. |
| A `.tran` line that says 20 million points or more ((stop - start) / maximum step) | refused before ngspice | Not checked with `.options interp`, which keeps the print step's points. |

Measured (`sim_e2e.py big`, a debug hub): a transient of 2,000,000 points with two signals, thinned to 200,000,
moves the hub's memory peak by 136 MB; the test's bound is 150 MB. The hub holds a reply's vectors once (the reply is
handed on, not copied). Memory scales with the points read: `max_points: 2000000` is ten times that.

Not bounded by the hub: KiCad's own memory for a run that is not windowed (ngspice keeps every computed point of
every saved vector until the run ends; the 6.7 GB of 17 Nov was a 1.9-million-point run with `.save all` on a
vendor model). `tstart`, the currents default and the 20-million refusal reduce it; a hard limit needs KiCad to stop
a run at a point count.

## Ops

Parts go by reference. A signal is what the Simulator calls it (`V(out)`, `I(R1)`), a net name, or ngspice's vector
name (`onoise_spectrum`); a root sheet's net works with or without its `/`. `V(a)/V(b)` and `V(a)-V(b)` combine two.

| Op | What it does |
|---|---|
| `{models: true}` | The parts that would not simulate, each with why, its MPN, and a model found on this machine (below). `"all"` or references: every part's model as a short row. |
| `{model: refs, device, type?, params?}` | A built-in device (`R`, `C`, `L`, `D`, `V`, `I`, `NPN`, `NMOS`, `SW` ...) with its parameters, written as KiCad's model dialog writes it. |
| `{model: refs, lib, name?, pins?}` | A `.subckt` or `.model` of a file (path from the project, or absolute). `pins: {pin number: node}`; without it the pins go by name when the symbol's pin names are the subcircuit's nodes (`pins_by: "name"`), else by order, and the answer then lists the nodes to check. |
| `{model: refs, raw: {primitive, model, library?}}`, `{model: refs, clear: true}` | Raw SPICE; back to no model. |
| `{exclude: ...}`, `{include: ...}` | References (with `*`), a group (`connectors`, `mounting_holes`, `test_points`, `fiducials`, `mechanical`, `jumpers`), `"no_model"` (every part of the first five groups that has no model: never an IC), or `{ref\|lib\|value: pattern}`. Answers the parts it took. |
| `{import: {file \| text, url, licence?, as?}, to?, name?, pins?}` | Keeps a fetched model in `sim/models/` (below), answers its definitions and their nodes, and assigns it to `to`. |
| `{source: {type, pos, neg?, value?, params?, ac?, current?, ref?, at?, bom?}}` | A `Simulation_SPICE` source symbol between two nets (`neg` is GND unless given), with its model. Types: dc, sin, pulse, pwl, exp, am, sffm, trnoise, trrandom. Out of the BOM and off the board unless `bom: true`. |
| `{probe: signals}`, `{probe: {pin: "R1.2", name}}` | Signals join the saved analysis' traces and this call's runs. A pin gets a net label first, so an unnamed net has a name to read. |
| `{analysis: ".ac dec 20 1 1meg", signals?, tab?}`, `{analysis: true}` | Saves the analysis with the design: what the person's Simulator tab opens with. `true` reads what is saved. |
| `{run: ".tran 1u 10m" \| true, set?, signals?, measures?, waves?, directives?, options?, tstart?, max_points?, retry?, netlist?, log?, timeout?, show?}` | Runs an analysis (`true`: the saved one, with its saved traces). Answers the operating-point values, the measures and one line per signal. |
| `{loopgain: "U1.2", run?, set?, directives?, waves?}` | A feedback loop's gain, cut at a pin in the netlist only (below). |
| `{sweep: "R1" \| "V1.dc" \| {param}, values \| from, to, steps?, log? \| series, run, measures, set?}` | One run per value; the measures as one table `{columns, rows}`. |
| `{tune: "R1", measure, target, from, to, series?, tol?, max_runs?, run, set?, write?}` | The value that brings a measure to its target. |
| `{stats: true \| {thread}}` | The log's totals (below). |

`set: {R1: "2k", "V1.dc": 3.3}` gives values for a run only (the fork's `overrides`): a part's value, or a parameter
after a dot. Sweeps and tunes use the same and change nothing. The schematic changes only by `model`, `exclude`,
`include`, `source`, `probe`, `analysis`, `import` with `to`, and `tune` with `write: true`.

### What a run answers

```json
{"run": {"analysis": "AC", "points": 121, "signals": {"V(out)": {"n": 121, "from": 1, "to": 1000000, "first_db": 0, "last_db": -55.96, "peak_db": 0, "peak_at": 1}},
         "measures": {"fc": 1591.2, "pm": 61.3}, "ms": 10}}
```

Vectors stay in the hub. `waves: true` (or a point count, 4 to 400) adds each named signal as columns of at most that
many points, `{x, y, of}` or `{f, db, deg, of}`: the first and last sample and the lowest and highest of each equal
part of the axis (equal in log f for a frequency axis), so no peak is lost (`ksim_wave::pick`). `netlist: true` adds
the netlist, `log: true` ngspice's console. With the person's KiCad holding the project a run is shown in its
Simulator tab (`show: false` leaves it); the runs of a sweep or a tune never are.

`directives` are netlist lines for the run (`.options`, `.ic`, `.nodeset`, `.include`, `.model`, `.param`); `options`
are KiCad's netlist options (`{save_all_currents: false}` for a vendor model whose internals have no branch currents).

A transient that does not converge is run again (`ksim::RETRY`), each attempt on its own, the cheap ones first:

1. gear integration (`.options method=gear`);
2. looser tolerances with the default integration (`.options itl4=500 gmin=1e-9 abstol=1e-8 vntol=1e-5 rshunt=1e9
   reltol=3e-3`): what a vendor's switching-regulator model converged with;
3. gear with the looser tolerances (`reltol=0.01`);
4. the second again with a maximum step no longer than the print step, last because it makes the run longer.

The first that converges is the answer, with `needed` saying what it took, so the next run can carry it as a
directive. Rules:

- An attempt that converged is never run again, however long it took.
- An attempt that took 20 s or more before it failed is the last: the answer says `no further attempt: that one took
  97 s, and each of the 4 left would take as long`.
- An attempt the run's own directives already are (they set every option of it) is skipped.
- A sweep's and a tune's later runs start with what their first run needed.
- When none converges the failure's last line is `tried without success:` with each attempt, how it ended and how
  long it took (`gear integration: no convergence at t=1.5e-05 (near /sw) after 0.044 s`).

`retry: false` leaves it at one run. The log's `retries` lists the attempts.

### Loop gain

`{loopgain: "U1.2"}` cuts the loop at a pin (an op-amp's inverting input, a regulator's FB pin) for its runs only: the
fork's `breaks` takes the pin off its net in the netlist and the hub puts a voltage source in the cut and a current
source into the pin, both zero at DC, so the operating point is the closed loop's and nothing is left in the
schematic. Three `.ac` runs (neither source, the voltage, the current; the first is what the circuit's own AC sources
put at the cut, taken out of the other two) give the loop gain by Tian's method (`ksim_wave::tian`: voltage and
current injection combined, right at any impedance level and whichever side of the cut drives). The answer is
`{at, crossover, phase_margin, gain_margin, dc_gain_db, runs}`; a margin that does not exist is `"none: why"`;
`waves` adds the curve `{f, db, deg}`. `run` gives another sweep than `.ac dec 50 1 100meg`.

### Measures

`measures: {name: spec}`. A spec is `"corner V(out)"`, `"rms V(out) from=1m to=2m"`, `{"corner": "V(out)", ...}`, or
the rest of an ngspice `meas` statement after its name (`"TRIG v(in) VAL=0.5 RISE=1 TARG v(out) VAL=0.5 RISE=1"`,
`"ac WHEN vdb(out)=-3"`). The hub's own, by name (`ksim_wave::MEASURES`, the one table the tool's description is made
from): at, cross, min, max, mean, final, pp, rms, ripple, ripple_rms, peak, overshoot, undershoot, rise_time,
fall_time, settling_time, frequency, period, duty, dc_gain, corner, bandwidth, center, q, unity_gain_freq,
phase_margin, gain_margin, gain_at, phase_at, noise_rms. `from` and `to` window any of them.

Samples are joined by straight lines (along log f on a frequency axis), a phase is unwrapped first, a mean and an RMS
weigh each sample by what it covers (ngspice's time steps are uneven). Gains are dB, phases degrees, overshoot and
duty percent. A corner is -3.0103 dB under the DC gain (a low-pass), the high-frequency gain (a high-pass) or the peak
(a band-pass: `{low, high}`); `ref` and `drop` say otherwise. A phase margin is 180 + the phase of the loop gain at
unity gain; `inverted: true` for a loop gain measured with the feedback's inversion in it. A measure that cannot be
taken is `"failed: why"` in its place ("the gain never falls through 0 dB between 1e0 and 1e3 Hz: sweep to a higher
frequency").

The tests hold them to closed forms: an RC's corner, rise time `tau ln 9` and settling `tau ln 50`; a second-order
step's overshoot `exp(-pi z / sqrt(1 - z^2))`; a series RLC's centre, bandwidth `R / (2 pi L)` and Q; the margins of
`K / (s (1 + s/p))` and `K / (1 + s/p)^3`; `kT/C`.

### Sweep and tune

A sweep steps a part's value, a parameter (`"V1.dc"`) or a `.param` (`{param: "gain"}`) through `values`, a range
(`from`, `to`, `steps`; log-spaced over a decade or more) or a series between two values (`series: "E24"`). A step
that fails is a row of nulls and a line in `failed`.

A tune runs both ends of `from`..`to`, then false position between the points that bracket the target (Illinois; along
log x, and log y while both are positive, so a power law is met in one step), until the measure is within `tol` (0.1 %)
or `max_runs` (16). With `series` the two series values around the result are run and the nearer one answered (`exact`
is the unrounded value). `write: true` puts the value on the schematic. A target outside what the range gives is an
error that says what the range gives.

## Failures

A failed simulation is its op's result, not a tool error: `{run: {failed, problems}}`, the ops after it are not run
(`skipped`), what was written before it is saved. A failed run's numbers are never answered (ngspice has some for a
floating node: gmin's, not the circuit's). `problems` are one line each with what to do next (`ksim_parts::problem_line`,
from the fork's `problems`, else from ngspice's console, else the status in words), eight at most:

| Problem | Line |
|---|---|
| no model | `missing model for U3: assign one (model) or exclude it (exclude)` |
| library not found | `model library of U1 not found (...): give its path with model (lib), or import the vendor's file` |
| model not in library | `the model named for Q2 is not in its library (BC847): name one the file has (import lists them), or give another file` |
| pin mismatch | `pins of U2 do not match its model's nodes: give the map with model (pins: {pin: node})` |
| bad value | `a value or parameter of R4 does not parse (...): set it with model (params), or correct the part's value` |
| floating node | `node N003 floats: nothing sets its DC level. Give it a path to ground (a resistor, a source), or exclude what hangs there` |
| singular matrix | `singular matrix at out: a loop of sources or inductors, or a node with no DC path. Add a small series resistance or a path to ground there` |
| no convergence (transient) | `no convergence at t=1.5e-05 (near /sw): shorten the maximum time step (the third value of .tran), give edges a rise time, or add directives: .options method=gear reltol=1e-3` |
| no convergence (operating point) | `no convergence: the DC operating point was not found. Set starting voltages (directives: .nodeset or .ic), or ramp the supplies in a .tran with uic` |
| no ground | `the circuit has no ground (node 0): put a GND power symbol on the return net` |
| not annotated | `annotate the schematic first: parts without a number in their reference do not simulate` |
| undefined model | `the netlist refers to an undefined model or subcircuit mbr340: a model file names it without defining it. Add its definition (directives: .include of the maker's file that has it, or a .model line), or import that file` |
| no code models | `ngspice's XSPICE code models are missing in this KiCad: parts that need them (digital, some vendor models) cannot be simulated here` |
| no AC source | `the input source has no AC value (v1): give it one (set: {"V1.ac": 1} for this run, or model params ac=1)` |
| another run | `a simulation of this project is running (the person's, or another agent's): this one was not started. Ask again when it ends` |

ngspice's console is in the answer only with `log: true`.

## Models

Where a model for a part without one is looked for, in order (`models` answers what it found as `found`, with where
it is from):

1. the symbol's own Sim.* fields (KiCad's; when they name a file or a model that is not there, the rest is tried for it);
2. the project's folder: `*.lib`, `*.mod`, `*.cir`, `*.sub`, `*.spice`, `*.sp`, `*.model`, `*.ckt` in it and under
   `sim/`, `models/`, `spice/`, `lib/` (`ksim_parts::scan`), matched by the part's MPN, value, library entry and
   Sim.Name (`name_fit`: the same name, or one starting with the other: LM358DR and LM358);
3. KiCad's bundled library and ngspice's built-ins: passives and sources are inferred by KiCad itself; ideal devices
   are a `model` with `device`;
4. the Backplane KiCad Library shipped with the runtime (`share/kicad/backplane-library`), scanned the same way;
5. the maker's site, by MPN. This step is the agent's: it finds the file and gives it with `import`.

`import` writes the file as it came to `sim/models/<name>` and `<name>.source.json` beside it: `url`, `licence` (the
source's licence line; "not stated by the source" when none was given), `fetched_at`, `sha256`, and the definitions.
The answer's `warnings` name what ngspice would misread in a file written for PSpice: a number with an ampere unit
stuck to it (`1Adc` is read as 1 atto) and a device naming a `.model` the file does not define. A web page saved in place of a model (a login or licence page), an encrypted model and a binary are refused in words
that say to ask the person.

The rule, in the tool's description, the prompt and `models`' answer (`ksim::VENDOR_RULE`): find the maker's model by
default; when the maker has none, or getting it needs a login, a licence acceptance or a purchase, stop and ask the
person, naming the part and what was found; never a generic or another part's model in its place without their word.

## The log

One JSON line per op in `<home>/logs/sim.jsonl` (`ksim_log.rs`), and one per turn that simulated:

| Field | |
|---|---|
| `at`, `thread`, `turn`, `path` | When (ms), who, the turn (the time it started), the schematic. |
| `op`, `ok`, `error` | The op; whether it could be done; why not. |
| `runs`, `analysis`, `status`, `points` | Simulations it ran (a sweep's and a tune's are many), the first one's analysis and points, the first status that is not `SIM_OK`. |
| `timings` | The fork's `netlist_ms`, `load_ms`, `run_ms`, `total_ms`, summed over the runs. |
| `ngspice_ms` | `load_ms + run_ms`. |
| `hub_ms` | The op's wall time in the hub. |
| `problems` | A failed run's lines. |
| `retries` | What a transient that did not converge was tried again with. |
| `lookups` | `models`: per part the names tried and what was found (`from`: project, library, kicad; none found is `[]`). `import`: `{stored, url, from: "vendor"}`. |
| `bytes` | The size of the op's answer as the agent got it. |
| `kind: "turn"`, `tokens`, `cost_nusd`, `context` | The turn's line, written at its end: the turn's tokens (input, cached and output) and cost from the provider's usage, joined to its ops by `thread` and `turn`. Null when the provider reported none. |

`{stats: true}` (or `{stats: {thread}}`) sums it: per op its count, runs, failures, times and answer bytes; the failed
statuses; where models came from; and per turn its actions, runs, bytes and tokens. A benchmark reads the file itself
or asks `stats`.

## Tests

`cargo test ksim`: the measurements against closed forms, the tune search, model files, failure lines, and every op
against a stand-in KiCad that solves an RC and a divider in closed form. Against a real headless KiCad:

```sh
bun test/kipc_bench/hub.mjs <backplane-hub> <runtime with sim-run> <port> WORK &
python3 test/kipc_bench/sim_e2e.py run WORK      # then: touch WORK/stop
```

`cargo test kicad::ipc`: three projects and room for two against a stand-in transport (a server with a run out is
never stopped to make room; a project is not started again beside its last process).

It generates five small schematics (an RC with a connector, a test point and an op-amp without a model; a divider; a
series RLC; two capacitors in series; a two-pole loop of two controlled sources), drives every op through the MCP tool as a stand-in agent's turn and checks the
numbers against closed form, the saved files, and the log's lines. Two more tasks are about load: `big` (two million
points: the answer, the `thinned` line, the hub's memory peak, `tstart`) and the crowd `slow`, `busya`, `busyb`
(three projects at once: a 40 s transient ends with its numbers while the other two are answered without a failure).
