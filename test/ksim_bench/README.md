# The simulation token benchmark

What a simulation task costs an agent in Backplane: tokens, tool calls, seconds, simulations, for a fixed set of tasks whose
answer is a number checked against a reference and whose schematic must be left set up for simulation.
`docs/kicad-sim-bench.md` has the tasks, the references and the numbers; this is how to run it. It is the sibling of
`test/kipc_bench` and uses its harness as it is: `hub.mjs`, `turn.mjs`, the `claude` wrapper, and `bench.py`'s turn,
fresh copy and token count (loaded from that file, not copied).

Real model turns: the real `claude` (Fable 5.2, medium effort) on a hub with a temp home, on fresh copies of generated
fixtures. Only with the person's go (AGENTS.md).

## Pieces

- `mkfix.py RUNTIME OUT`: the fixtures, written by `schgen.py` from the runtime's own symbol libraries and upgraded by
  its `kicad-cli`: `lcfilt`, `preamp`, `ldo`, `buck`, `buck_mp`, `vsense`, `aafilter`. Each is a board's schematic
  (connectors, mounting holes, a test point, footprints, ERC clean) with no `Sim.*` field and no source. Beside each,
  `<fixture>.solved`: the same set up for simulation, which the references are simulated from. `models/` holds the two
  macro-models written for the benchmark (BP321 op-amp, BP758 regulator; not any manufacturer's); the buck's model is
  the manufacturer's, downloaded (`VENDOR` in `mkfix.py`: URL and sha256).
- `tasks.json`: the tasks: a prompt a person could write, the fixture, the reference numbers with their tolerances, what
  the saved schematic is held to, and how the reference was derived.
- `refs.py`: every reference derived twice: from the component values alone (`mna.py`, a small circuit solver, or a
  formula) and by ngspice on the netlist KiCad's exporter writes from the solved fixture. `tools/ngs.py` drives the
  runtime's `libngspice` (the runtime ships no `ngspice` program).
- `checks.py`: the pass check: the answer's `RESULT` line within tolerance, and the saved schematic still the board's
  and now simulating (see its head).
- `osim.py`, `oracles/<task>.py`: each task done by hand over raw `kicad_ipc` (no model), on a runtime with the
  simulation commands; `bench.py e2e` runs them through the hub when one is up and holds them to the check.
- `bench.py`: `ref`, `e2e`, `run`, `table`.

## A run

```sh
RT=<the KiCad runtime>; W=$BP_TMP/ksim-bench; mkdir -p $W/a
python3 test/ksim_bench/mkfix.py $RT $W/fixtures
echo "{\"fixtures\":\"$W/fixtures\",\"cli\":\"$RT/bin/kicad-cli\"}" > $W/a/bench.json
python3 test/ksim_bench/bench.py ref $W/a          # both derivations against tasks.json; no hub
python3 test/ksim_bench/bench.py e2e $W/a          # the checks pass the solved fixtures and fail the untouched ones
bun test/kipc_bench/hub.mjs native/host/target/fast/backplane-hub $RT 3871 $W/a &   # touch $W/a/stop ends it
python3 test/ksim_bench/bench.py run $W/a base --runs 3 --par 2
python3 test/ksim_bench/bench.py table $W/a
```

The variant knobs are `hub.mjs`'s: `KIPC_BENCH_DISALLOW` (tools kept from the agent), `KIPC_BENCH_PROMPT` (text
replaced in the hub's prompt), `KIPC_BENCH_NOMCP`, `KIPC_BENCH_MODEL`, `KIPC_BENCH_LOGIN`.

Measured per run: what `test/kipc_bench` measures (context tokens, output tokens, tool calls, wall seconds, cost, pass)
plus simulations, ngspice seconds and model lookups from the hub's `<home>/logs/sim.jsonl` when it exists; before it
does, `simulations` counts the shell commands that ran ngspice and the `RunSimulation` calls. A run the account's limit
or an API error cut short is marked `cut`, left out of the table and counted under it.
