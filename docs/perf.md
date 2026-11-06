# Performance

Every Backplane process measures itself all the time: a row per name in a
table held by `src/server/effects/perf.c`. A recorded value costs a hash
lookup and a few adds. Nothing is ever reset. A window of time is the
difference between two readings (`PF.Stats.since`), so the overlay,
`/debug/perf` and a trace never disturb each other.

## Looking

- **Overlay**: ctrl+shift+h in the window. It shows the last 3 to 6 s of
  the window's work: frame time, input to pixels, hub message to pixels,
  each frame step, ops per frame, the hub's share of the time, cpu, rss,
  and the heaviest hub and window messages. It refreshes with each frame
  drawn while it is up and asks for no frames itself (no repaint loop),
  so an idle window shows the last busy moment.
- **`GET /debug/perf`**: every row since the process started, as a table
  (spans in ms, heaviest first, then counts, then gauges). Also
  `/debug/perf.json` (spans in µs, with `kind`, `mean`, `p50`, `p90`,
  `p99`) and `/debug/log` (the last 512 log lines). They are gated like
  `/img`: the pairing token (from `<home>/token`) or a paired device's.
  `printf 'Authorization: Bearer %s\n' "$(cat ~/.backplane-bend/token)" | curl -s -H @- 127.0.0.1:3787/debug/perf`
- **Trace**: `BACKPLANE_TRACE=/tmp/t.json dist/backplane` writes every
  span, count and log line as a Chrome trace event. Open it in
  ui.perfetto.dev. Each area (ui, hub, boot, ...) gets its own row. Events
  reach the file whole, so a killed process leaves a file that still
  loads.
- **Request timing**: `BACKPLANE_TIMING=1` logs a `[bp-timing] {...}` line
  per request a connection sends: what it was (the WebSocket frame's
  method, or the HTTP request line), its size, the fibers waiting when it
  came (`runnable`), `in_to_out_ms`, from its first bytes in to the first
  bytes the hub writes back on that connection, and `cpu_ms`, the process CPU
  spent meanwhile (src/server/effects/loop.js).
  A connection's next write can be a broadcast rather than the answer: read
  it by the method.
- **Logs**: `BACKPLANE_LOG=error|warn|info|debug|trace` (default info),
  `BACKPLANE_LOG_FILE=<file>` to keep a copy. A line looks like
  `backplane 12:03:04 warn slow: ui.frame took 213.2 ms` (UTC).
- **Slow work**: any span longer than `BACKPLANE_SLOW_MS` (default 100) is
  logged as a warning. The check runs through `PS.Perf.lap`, and spans
  under 16 ms skip it without reading the environment.

## Names

`area.what`. Spans are µs, other values are plain numbers, and gauges keep
their last value.

| name | kind | what |
|---|---|---|
| `ui.frame` | span | a frame from memo to presented |
| `ui.memo`, `ui.layout`, `ui.raster`, `ui.present` | span | its steps (`ui.lightbox` in the lightbox) |
| `ui.plan`, `ui.draw`, `ui.fill` | span | `ui.raster`'s parts: which tiles to draw, drawing them in parallel, the tiles filled in |
| `ui.drawn` | value | tiles drawn in a frame |
| `ui.input` | span | a window event that changed something, to the frame showing it |
| `ui.hub` | span | a hub message that changed something, to the frame showing it |
| `ui.msg.<kind>` | span | handling one window message (`win`, `hub`, `frame`, `blink`, `board`, ...) |
| `ui.ops` | value | draw ops per frame |
| `ui.hud` | span | the overlay's own cost |
| `hub.<kind>` | span | handling one hub message (`rpc`, `agent_line`, `join`, `tick`, ...) |
| `hub.clients`, `hub.agents`, `hub.changes` | gauge | set each minute |
| `boot.read`, `boot.replay` | span | reading and replaying the event log |
| `proc.rss_kb`, `proc.peak_kb`, `proc.cpu_ms`, `proc.up_ms` | gauge | read at each take |

`ui.input` and `ui.hub` start after the message is handled, and only when
it left a frame to draw. They include the frame throttle (at most one
frame per 12 ms). Time a message spent queued before it was handled is not
in them.

The `ui.raster` / `ui.present` split rests on the pure tiles being
evaluated before the `lap` that follows them, as `test/native/type_bench`
assumes. If Bend defers any of it into `W.Win.present`, that time shows as
`ui.present`. `ui.frame` is right either way.

## Measuring something new

```python
import ../server/perf.bend as PS

do IO<X>:
  t0 : U32 <- PS.Perf.us()
  +v : T = heavy(...)                 # pure work bound before the lap
  t1 : U32 <- PS.Perf.lap("area.step", t0)   # records, warns when slow
  PS.Perf.add("area.items", n)        # a value per event
  PS.Perf.set("area.size", n)         # a gauge
  PS.Perf.mark("area.wait")           # start a span here ...
  PS.Perf.done("area.wait")           # ... end it anywhere later
  PS.Log.debug("area", "what happened")
```

`PS.Perf.span` records without the slow check and answers the time, so
steps chain: `t2 <- span("b", t1)`. Pure work has to be finished before
the next effect for its time to count. Bind it with `=` before the `lap`,
or pass something computed from it (the op count forces the layout in
`Shell.paint.go`).

Readings are text. `PF.Stats.parse` reads them and `PF.Stats.since`
subtracts an older one. `PF.Stat.pct` answers the top of a log2 bucket (an
upper bound within 2x, at most the max). `PF.Stats.table`, `PF.Stats.json`
and `PF.Hud.lines` format them.

## Pieces

- `src/server/effects/perf.c` (+ `perf.js`): the table (1024 names, a
  33-bucket log2 histogram each), marks, the trace writer and the log
  ring. Effects run on the event loop only, so it needs no lock.
- `src/server/perf.bend`: the effects, `Perf.lap`, `Log.*`.
- `src/core/perf.bend`: pure. Parsing, windows, percentiles, the report,
  the overlay text, log levels and lines. Laws `perf_*`.
- `src/app/nui.bend` `Hud.*`: the overlay's two readings, kept in the Ui
  scratch. `src/app/layout.bend` `Lay.perf.*`: drawing it.
- `test/perf_test.bend`, `test/native/hud_bench.bend`.
