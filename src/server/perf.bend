import Base
import ../core/perf.bend as PF
import ./host.bend as H

# Measuring and logging, for any computation in the process (the hub, the
# window, a connection). effects/perf.js keeps the numbers; core/perf.bend
# reads them. See docs/perf.md.

# microseconds since the first reading (wraps after 71 minutes; spans are
# wrapping differences, so they stay right)
def Perf.us() -> IO(U32):
  import "./effects/perf.js"

# record now - t0 under name; answers now, the next span's t0
def Perf.span(name: String, t0: U32) -> IO(U32):
  import "./effects/perf.js"

# record a value (a count, a size) under name
def Perf.add(name: String, v: U32) -> IO(Unit):
  import "./effects/perf.js"

# a gauge: only its last value means anything
def Perf.set(name: String, v: U32) -> IO(Unit):
  import "./effects/perf.js"

# start a span under name, unless one is already started
def Perf.mark(name: String) -> IO(Unit):
  import "./effects/perf.js"

# end the span mark started (nothing when none is)
def Perf.done(name: String) -> IO(Unit):
  import "./effects/perf.js"

# every row, as PF.Stats.parse reads them
def Perf.take() -> IO(String):
  import "./effects/perf.js"

# one log line to stderr, BACKPLANE_LOG_FILE, the trace and the ring
def Perf.emit(line: String) -> IO(Unit):
  import "./effects/perf.js"

# the last 512 lines emitted, oldest first
def Perf.logs() -> IO(String):
  import "./effects/perf.js"

def Perf.env.or(r: Result<&1, &1, U32 & String, String>, +d: String) -> String:
  match r:
    case Done{v}:
      v
    case Fail{_}:
      d

def Perf.env(+name: String, +d: String) -> IO(String):
  IO.bind(Result<&1, &1, U32 & String, String>, String, IO.get_env(name), r => IO.pure(String, Perf.env.or(r, d)))

# Logging
# -------

def Log.go(on: Bool, +l: Nat, +area: String, +msg: String) -> IO(Unit):
  match on:
    case True{}:
      IO.bind(U32, Unit, H.Clock.epoch(), t => Perf.emit(PF.Log.line(l, U32.to_nat(t), area, msg)))
    case False{}:
      IO.pure(Unit, Unit{})

# a line at level l (0 error .. 4 trace), if BACKPLANE_LOG lets it through
def Log.at(+l: Nat, +area: String, +msg: String) -> IO(Unit):
  IO.bind(String, Unit, Perf.env("BACKPLANE_LOG", "info"), v => Log.go(PF.Log.on(l, PF.Log.level(v)), l, area, msg))

def Log.error(+area: String, +msg: String) -> IO(Unit):
  Log.at(0n, area, msg)

def Log.warn(+area: String, +msg: String) -> IO(Unit):
  Log.at(1n, area, msg)

def Log.info(+area: String, +msg: String) -> IO(Unit):
  Log.at(2n, area, msg)

def Log.debug(+area: String, +msg: String) -> IO(Unit):
  Log.at(3n, area, msg)

# Slow work
# ---------
# A span longer than BACKPLANE_SLOW_MS (default 100) is logged as a
# warning. Spans under 16 ms are never slow, so the common case reads no
# environment.

def Slow.over(slow: Bool, +name: String, +us: Nat) -> IO(Unit):
  match slow:
    case True{}:
      Log.warn("slow", name ++ " took " ++ PF.Perf.ms(us) ++ " ms")
    case False{}:
      IO.pure(Unit, Unit{})

def Slow.check(big: Bool, +name: String, +us: Nat) -> IO(Unit):
  match big:
    case True{}:
      IO.bind(String, Unit, Perf.env("BACKPLANE_SLOW_MS", "100"), v => Slow.over(PF.Perf.slow(us, PF.Perf.nat(v)), name, us))
    case False{}:
      IO.pure(Unit, Unit{})

# end a span begun at t0 and warn when it was slow; answers now
def Perf.lap(+name: String, +t0: U32) -> IO(U32):
  do IO<U32>:
    +t1 : U32 <- Perf.span(name, t0)
    Slow.check(U32.is_gt((t1 - t0 : U32), 16000), name, U32.to_nat((t1 - t0 : U32)))
    return t1
