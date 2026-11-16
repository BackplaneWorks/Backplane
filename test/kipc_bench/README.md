# The KiCad-IPC token benchmark

What an agent's KiCad edit costs: tokens, tool calls and seconds, for a fixed set of edits whose result is checked
item for item. `docs/kicad-ipc-bench.md` has the numbers; this is how to run it.

Real model turns: it runs the real `claude` (Fable 5.2 by default) on a hub with a temp home, on copies of the KiCad
runtime's own demo boards. Never on a person's project, and only with their go (AGENTS.md, "Real model turns").

## Pieces

- `mkfix.sh RUNTIME OUT`: the fixtures, from the runtime's demos, upgraded to its file format: `stickhub` (94 footprints,
  DRC clean), `stickhub_drc` (the same with one stray track: 8 DRC findings), `pic_programmer` (two sheets).
- `tasks.json`: the eight edits, each a prompt a person could write (no API hints) on a fixture.
- `oracles/<task>.py`: the reference edit of each task, over raw `kicad_ipc` (no model).
- `norm.py`: a project folder as comparable items (footprints, tracks, vias, zones with fill area, net classes, rules,
  symbols and fields on every sheet, wires, labels) plus what DRC and ERC say by kind. Ids and text positions are left
  out. A run passes when its folder's items equal the reference's.
- `hub.mjs HUB RUNTIME PORT WORK`: a hub with a temp home, its own HOME/XDG for KiCad, and `claude` (this folder's
  wrapper) as its only agent. `KIPC_BENCH_LOGIN` is the Claude config folder to sign in with (default `~/.claude`);
  `KIPC_BENCH_DISALLOW` keeps tools from the agent; `KIPC_BENCH_PROMPT` is a JSON file of `[old, new]` replacements in the
  hub's system prompt (or `["re", pattern, new]` for text that holds a run's paths), so a wording is measured before it
  is built in; `KIPC_BENCH_NOMCP=1` takes every MCP server away (the raw file variant: `prompts/rawfile.json`, only the
  CLI's file and shell tools); `KIPC_BENCH_MODEL` picks the model.
- `grok`: the same for Grok (`KIPC_BENCH_PROVIDER=grok KIPC_BENCH_THREAD_MODEL=grok-4.7` makes the run's thread a Grok thread): the real Grok
  CLI over ACP, its lines copied to `WORK/out/<run>.jsonl`; `bench.py` reads tokens and tool calls from them and prices them at the
  provider's list price (`PRICE`). `KIPC_BENCH_SEAL=1` closes a finished run's folder (Grok searches the disk).
- `claude`: starts the real CLI with the hub's own arguments (its prompt, MCP tools, plugins), the model and effort
  fixed, and copies its stream to `WORK/out/<run>.jsonl`. A turn that begins `ORACLE <script>` runs that script with the
  hub's MCP url instead (no model); `HOLD <file>` holds a turn open to call tools by hand (`mcp.py`).
- Design tasks: `mkfix2.py CLI FIXTURES HUBBLE_DIR OUT` builds their fixtures (and `OUT/<fixture>.base.json`, what DRC and
  ERC said before), `tasks2.json` the tasks (`KIPC_BENCH_TASKS=.../tasks2.json`), `checks.py` the rules each is held to
  (no one reference: DRC/ERC nothing new, the netlist, the spec), `kx.py` what KiCad's CLI says (DRC, ERC, netlist, pads).
- `tasks2.json`, `checks.py`, `kx.py`, `mkfix2.py CLI FIXTURES HUBBLE_DIR OUT`: the design tasks (a schematic group, routing, placement, a
  footprint exchange, a real 8-layer board), their rule checks (DRC and ERC with nothing new, the netlist, the spec) and their fixtures;
  `KIPC_BENCH_TASKS=.../tasks2.json` runs `bench.py run`, `e2e` and `table` on them (`e2e`: `edit_e2e.py`'s `DESIGN`, the fewest calls).
- `bench.py`: `ref` (the references), `e2e` (each task as one `kicad_edit` call must equal the reference), `run`
  (real turns; a line per run in `WORK/results.jsonl`), `table`.

## A run

```sh
RT=~/.local/share/backplane/kicad/<version>; W=$BP_TMP/kipc-bench; mkdir -p $W/a
sh test/kipc_bench/mkfix.sh $RT $W/fixtures
echo "{\"fixtures\":\"$W/fixtures\",\"cli\":\"$RT/bin/kicad-cli\"}" > $W/a/bench.json
bun test/kipc_bench/hub.mjs native/host/target/fast/backplane-hub $RT 3861 $W/a &   # touch $W/a/stop ends it
python3 test/kipc_bench/bench.py ref $W/a          # the references (about a minute)
python3 test/kipc_bench/bench.py e2e $W/a          # kicad_edit against real KiCad, no model
python3 test/kipc_bench/bench.py run $W/a mine --runs 3 --par 2
python3 test/kipc_bench/bench.py table $W/a
```

A hub runs two KiCad servers at most, so `--par 2`; a second hub on another port doubles that. Every run is a fresh
copy of its fixture in `WORK/runs/<variant>-<task>-<n>` and a project and thread of its own.

What is measured, from the CLI's own `result` line: context tokens (input + cache writes + cache reads, summed over the
turn's requests: what the model read), output tokens, tool calls, wall seconds and the CLI's cost figure. A turn's
context is roughly its requests times the 40 thousand tokens every request carries, so the count of requests is what
moves it.
