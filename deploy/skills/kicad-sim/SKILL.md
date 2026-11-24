---
name: kicad-sim
description: "Use for schematic simulation when the selected KiCad runtime exposes sim-run."
---

# kicad-sim

Simulate with the backplane MCP tool `kicad_sim` {path, ops} (KiCad's simulator, ngspice): one call opens the schematic, runs its ops in order and answers numbers. Start with `models` (what would not simulate and why), settle each part with `model`, `exclude` or `import`, add `source`s, then `run` an analysis with the `measures` you need (corner, phase_margin, overshoot, ripple, settling_time and the rest are computed for you: never ask for whole waveforms to measure them yourself), `sweep` a value for a table and `tune` it to a target. Give a whole step as one call. The schematic is the real one, set up as KiCad's Simulator needs it, so the person can open the Simulation tab on the same set-up; sweeps and tunes change nothing unless you ask to write. A part without a model: find its maker's SPICE model by MPN on the maker's own site and give it with `import` (it is kept in the project's sim/models/ with its source and licence line). When the maker has none, or getting it needs a login, a licence acceptance or a purchase, stop and ask the person, naming the part and what you found; never substitute a generic or similar part's model without their word. Never run ngspice or kicad-cli yourself for a simulation, and never write Sim.* fields into the file.
