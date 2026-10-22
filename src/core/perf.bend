import Base
import ./json.bend as J

# Performance numbers, read and summed up (pure). src/server/effects/perf.js
# keeps one row per name: how many values, their sum, the largest, the last
# one and a log2 histogram (bucket 0 holds 0, bucket i holds [2^(i-1),
# 2^i)). Spans are microseconds. Perf.take answers the rows as text:
#   name \t kind \t n \t sum \t max \t last \t b0,b1,...
# Kind 0 is a stream of spans, kind 2 of other values (a count per frame,
# a size), kind 1 a gauge (only the last value means anything). Rows are never reset: a window of
# time is the difference of two readings (Stats.since), so the overlay,
# /debug/perf and a periodic dump never disturb each other.
#
# Names are "area.what": ui.* the window (ui.frame, ui.memo, ui.layout,
# ui.raster, ui.present, ui.input and ui.hub for input and hub messages to
# pixels, ui.msg.<kind> per message), hub.<kind> per hub message, boot.*,
# proc.* (read at each take). See docs/perf.md.

type Stat is Data:
  Stat{name: String, kind: U32, n: Nat, sum: Nat, max: Nat, last: Nat, bs: List<&2, Nat>}

# Reading
# -------

def Perf.nat(+s: String) -> Nat:
  Maybe.default(&2, Nat, Nat.read(s), 0n)

def Perf.buckets(xs: List<&2, String>) -> List<&2, Nat>:
  match xs:
    case Nil{}:
      Nil{}
    case +h <> t:
      Perf.nat(h) <> Perf.buckets(t)

def Stat.row(fs: List<&2, String>) -> Maybe<&2, Stat>:
  match fs:
    case +name <> +kind <> +n <> +sum <> +max <> +last <> +bs <> _:
      Some{Stat{name, U32.from_nat(Perf.nat(kind)), Perf.nat(n), Perf.nat(sum), Perf.nat(max), Perf.nat(last), Perf.buckets(String.split(bs, ','))}}
    case _:
      None{}

def Stats.keep(m: Maybe<&2, Stat>, rest: List<&2, Stat>) -> List<&2, Stat>:
  match m:
    case Some{s}:
      s <> rest
    case None{}:
      rest

def Stats.rows(ls: List<&2, String>) -> List<&2, Stat>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Stats.keep(Stat.row(String.split(l, '\t')), Stats.rows(t))

# every row of a reading (a malformed line is left out)
def Stats.parse(+text: String) -> List<&2, Stat>:
  Stats.rows(String.lines(text))

# Fields
# ------

def Stat.name(s: Stat) -> String:
  match s:
    case Stat{name, _, _, _, _, _, _}:
      name

def Stat.n(s: Stat) -> Nat:
  match s:
    case Stat{_, _, n, _, _, _, _}:
      n

def Stat.sum(s: Stat) -> Nat:
  match s:
    case Stat{_, _, _, sum, _, _, _}:
      sum

def Stat.max(s: Stat) -> Nat:
  match s:
    case Stat{_, _, _, _, max, _, _}:
      max

def Stat.last(s: Stat) -> Nat:
  match s:
    case Stat{_, _, _, _, _, last, _}:
      last

def Stat.gauge(s: Stat) -> Bool:
  match s:
    case Stat{_, kind, _, _, _, _, _}:
      U32.is_eq(kind, 1)

def Stat.span(s: Stat) -> Bool:
  match s:
    case Stat{_, kind, _, _, _, _, _}:
      U32.is_eq(kind, 0)

def Stat.none(+name: String) -> Stat:
  Stat{name, 0, 0n, 0n, 0n, 0n, Nil{}}

def Stats.hit(xs: List<&2, Stat>, +name: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case s <> _:
      String.eq(Stat.name(s), name)

def Stats.find.go(xs: List<&2, Stat>, hit: Bool, +name: String) -> Stat:
  match xs hit:
    case Nil{} _:
      Stat.none(name)
    case s <> _ True{}:
      s
    case _ <> +t False{}:
      Stats.find.go(t, Stats.hit(t, name), name)

# the row named name (an empty one when there is none); stops at the first
def Stats.find(+xs: List<&2, Stat>, +name: String) -> Stat:
  Stats.find.go(xs, Stats.hit(xs, name), name)

# Windows
# -------

def Perf.minus(+a: Nat, +b: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_ge(a, b), Nat.sub(a, b), 0n)

def Perf.bminus(xs: List<&2, Nat>, ys: List<&2, Nat>) -> List<&2, Nat>:
  match xs ys:
    case Nil{} _:
      Nil{}
    case +x <> t Nil{}:
      x <> Perf.bminus(t, Nil{})
    case +x <> t +y <> u:
      Perf.minus(x, y) <> Perf.bminus(t, u)

def Stat.less(cur: Stat, old: Stat) -> Stat:
  match cur old:
    case Stat{name, kind, +n, +sum, max, last, bs} Stat{_, _, +n0, +s0, _, _, bs0}:
      Stat{name, kind, Perf.minus(n, n0), Perf.minus(sum, s0), max, last, Perf.bminus(bs, bs0)}

def Stat.since.k(gauge: Bool, +cur: Stat, old: Stat) -> Stat:
  match gauge:
    case True{}:
      cur
    case False{}:
      Stat.less(cur, old)

# what happened between an older reading of the same row and this one; a
# gauge is its latest reading. The max is the row's max so far (a bound)
def Stat.since(+cur: Stat, old: Stat) -> Stat:
  Stat.since.k(Stat.gauge(cur), cur, old)

def Stats.since.go(cur: List<&2, Stat>, +old: List<&2, Stat>) -> List<&2, Stat>:
  match cur:
    case Nil{}:
      Nil{}
    case +s <> t:
      Stat.since(s, Stats.find(old, Stat.name(s))) <> Stats.since.go(t, old)

# every row of cur, less what old already counted
def Stats.since(cur: List<&2, Stat>, +old: List<&2, Stat>) -> List<&2, Stat>:
  Stats.since.go(cur, old)

# Summaries
# ---------

def Stat.mean(s: Stat) -> Nat:
  match s:
    case Stat{_, _, +n, sum, _, _, _}:
      Bool.pick(Nat, Nat.is_eq(n, 0n), 0n, Nat.div(sum, n))

# the largest value bucket i can hold
def Perf.top(+i: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_eq(i, 0n), 0n, Nat.sub(Nat.pow(2n, i), 1n))

def Perf.pct.go(bs: List<&2, Nat>, +want: Nat, +seen: Nat, +i: Nat) -> Nat:
  match bs:
    case Nil{}:
      Perf.top(Nat.sub(i, 1n))
    case +b <> t:
      Bool.pick(Nat, Nat.is_ge(Nat.add(seen, b), want), Perf.top(i), Perf.pct.go(t, want, Nat.add(seen, b), 1n+i))

def Perf.low(+a: Nat, +b: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_le(a, b), a, b)

# an upper bound on the value below which `per` thousandths of the values
# fall (the top of its bucket, at most the max); 0 with no values
def Stat.pct(s: Stat, +per: Nat) -> Nat:
  match s:
    case Stat{_, _, +n, _, +max, _, bs}:
      Bool.pick(Nat, Nat.is_eq(n, 0n), 0n,
        Perf.low(Perf.pct.go(bs, Perf.low(n, Nat.div(Nat.add(Nat.mul(n, per), 999n), 1000n)), 0n, 0n), max))

# microseconds as milliseconds with one decimal: 1234 -> "1.2"
def Perf.ms(+us: Nat) -> String:
  Nat.show(Nat.div(us, 1000n)) ++ "." ++ Nat.show(Nat.mod(Nat.div(us, 100n), 10n))

# a span over the slow threshold (ms) is worth a warning
def Perf.slow(+us: Nat, +ms: Nat) -> Bool:
  Nat.is_gt(us, Nat.mul(ms, 1000n))

def Perf.pad(+s: String, +w: Nat) -> String:
  s ++ String.repeat(" ", Perf.minus(w, String.length(s)))

def Perf.lpad(+s: String, +w: Nat) -> String:
  String.repeat(" ", Perf.minus(w, String.length(s))) ++ s

# Order
# -----

# the rows that took the most time first
def Stat.heavier(a: Stat, b: Stat) -> Bool:
  Nat.is_ge(Stat.sum(a), Stat.sum(b))

def Stats.by_time(+xs: List<&2, Stat>) -> List<&2, Stat>:
  List.sort(~Stat, ~Stat.heavier, xs)

def Stat.kind(s: Stat) -> U32:
  match s:
    case Stat{_, kind, _, _, _, _, _}:
      kind

def Stats.kind.k(same: Bool, +s: Stat, +rest: List<&2, Stat>) -> List<&2, Stat>:
  match same:
    case True{}:
      Bool.pick(List<&2, Stat>, Bool.and(Nat.is_eq(Stat.n(s), 0n), Bool.not(Stat.gauge(s))), rest, s <> rest)
    case False{}:
      rest

# the rows of one kind (0 spans, 1 gauges, 2 values); spans and values
# only when the window has some
def Stats.kind(xs: List<&2, Stat>, +k: U32) -> List<&2, Stat>:
  match xs:
    case Nil{}:
      Nil{}
    case +s <> t:
      Stats.kind.k(U32.is_eq(Stat.kind(s), k), s, Stats.kind(t, k))

def Stats.prefix.k(hit: Bool, +s: Stat, rest: List<&2, Stat>) -> List<&2, Stat>:
  match hit:
    case True{}:
      s <> rest
    case False{}:
      rest

# the rows whose names start with p (a match, not Bool.pick: that would
# walk the rest twice per row)
def Stats.prefix(xs: List<&2, Stat>, +p: String) -> List<&2, Stat>:
  match xs:
    case Nil{}:
      Nil{}
    case +s <> t:
      Stats.prefix.k(String.starts_with(Stat.name(s), p), s, Stats.prefix(t, p))

def Stats.total(xs: List<&2, Stat>) -> Nat:
  match xs:
    case Nil{}:
      0n
    case +s <> t:
      Nat.add(Stat.sum(s), Stats.total(t))

# The report
# ----------

def Perf.kind(+k: U32) -> String:
  Bool.pick(String, U32.is_eq(k, 0), "span", Bool.pick(String, U32.is_eq(k, 1), "gauge", "value"))

def Stats.order(+xs: List<&2, Stat>) -> List<&2, Stat>:
  List.concat(&2, Stat, [Stats.by_time(Stats.kind(xs, 0)), Stats.by_time(Stats.kind(xs, 2)), Stats.kind(xs, 1)])

def Stat.json(+s: Stat) -> J.Json:
  J.Json.obj([J.KV{"name", J.Str{Stat.name(s)}}, J.KV{"kind", J.Str{Perf.kind(Stat.kind(s))}}, J.KV{"n", J.Json.nat(Stat.n(s))},
    J.KV{"sum", J.Json.nat(Stat.sum(s))}, J.KV{"mean", J.Json.nat(Stat.mean(s))}, J.KV{"p50", J.Json.nat(Stat.pct(s, 500n))},
    J.KV{"p90", J.Json.nat(Stat.pct(s, 900n))}, J.KV{"p99", J.Json.nat(Stat.pct(s, 990n))}, J.KV{"max", J.Json.nat(Stat.max(s))}, J.KV{"last", J.Json.nat(Stat.last(s))}])

def Stats.json.go(xs: List<&2, Stat>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case +s <> t:
      Stat.json(s) <> Stats.json.go(t)

# every row: spans (in microseconds, the heaviest first), values, gauges
def Stats.json(+xs: List<&2, Stat>) -> J.Json:
  J.Json.obj([J.KV{"unit", J.Str{"us"}}, J.KV{"rows", J.Json.arr(Stats.json.go(Stats.order(xs)))}])

# a span in ms, any other value as it is
def Perf.fmt(span: Bool, +v: Nat) -> String:
  match span:
    case True{}:
      Perf.ms(v)
    case False{}:
      Nat.show(v)

def Stat.line(+s: Stat) -> String:
  Perf.pad(Stat.name(s), 28n) ++ Perf.lpad(Nat.show(Stat.n(s)), 9n) ++ Perf.lpad(Perf.fmt(Stat.span(s), Stat.sum(s)), 11n) ++ Perf.lpad(Perf.fmt(Stat.span(s), Stat.mean(s)), 9n)
    ++ Perf.lpad(Perf.fmt(Stat.span(s), Stat.pct(s, 500n)), 9n) ++ Perf.lpad(Perf.fmt(Stat.span(s), Stat.pct(s, 990n)), 9n) ++ Perf.lpad(Perf.fmt(Stat.span(s), Stat.max(s)), 9n)

def Stat.gline(+s: Stat) -> String:
  Perf.pad(Stat.name(s), 28n) ++ Perf.lpad(Nat.show(Stat.last(s)), 9n)

def Stats.lines(xs: List<&2, Stat>, +g: Bool) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +s <> t:
      Bool.pick(String, g, Stat.gline(s), Stat.line(s)) <> Stats.lines(t, g)

def Stats.head() -> String:
  Perf.pad("name", 28n) ++ Perf.lpad("n", 9n) ++ Perf.lpad("total", 11n) ++ Perf.lpad("mean", 9n) ++ Perf.lpad("p50<=", 9n) ++ Perf.lpad("p99<=", 9n) ++ Perf.lpad("max", 9n)

# the rows as a table (spans in ms, the heaviest first), then the gauges
def Stats.table(+xs: List<&2, Stat>) -> String:
  String.join(List.append(&2, String,
    Stats.head()
      <> Stats.lines(List.append(&2, Stat, Stats.by_time(Stats.kind(xs, 0)), Stats.by_time(Stats.kind(xs, 2))), False{}),
    "" <> Stats.lines(Stats.kind(xs, 1), True{})), "\n") ++ "\n"

# The overlay
# -----------
# ctrl+shift+h in the window: the last few seconds of the window's work,
# in the code font (so columns line up). `d` is the window, `cur` the
# latest reading, `ms` how long the window is.

def Hud.row(+label: String, +s: Stat) -> String:
  Perf.pad(label, 9n) ++ Perf.lpad(Nat.show(Stat.n(s)), 5n) ++ Perf.lpad(Perf.ms(Stat.mean(s)), 7n) ++ Perf.lpad(Perf.ms(Stat.pct(s, 500n)), 7n)
    ++ Perf.lpad(Perf.ms(Stat.pct(s, 990n)), 7n)

def Hud.top(k: Nat, xs: List<&2, Stat>) -> List<&2, String>:
  match k xs:
    case 0n _:
      Nil{}
    case _ Nil{}:
      Nil{}
    case 1n+j +s <> t:
      (Perf.pad(Stat.name(s), 20n) ++ Perf.lpad(Perf.ms(Stat.sum(s)), 8n) ++ Perf.lpad(Nat.show(Stat.n(s)), 6n)) <> Hud.top(j, t)

# share of the window (percent) that n microseconds are
def Hud.pct(+us: Nat, +ms: Nat) -> String:
  Nat.show(Nat.div(Nat.mul(us, 100n), Nat.mul(Bool.pick(Nat, Nat.is_eq(ms, 0n), 1n, ms), 1000n))) ++ "%"

def Hud.lines(+d: List<&2, Stat>, +ms: Nat) -> List<&2, String>:
  List.concat(&2, String, [
    ["last " ++ Perf.ms(ms) ++ " s    n   mean   p50<  p99<",
      Hud.row("frame", Stats.find(d, "ui.frame")),
      Hud.row("input>px", Stats.find(d, "ui.input")),
      Hud.row("hub>px", Stats.find(d, "ui.hub")),
      Hud.row(" memo", Stats.find(d, "ui.memo")),
      Hud.row(" layout", Stats.find(d, "ui.layout")),
      Hud.row(" raster", Stats.find(d, "ui.raster")),
      Hud.row(" present", Stats.find(d, "ui.present")),
      "ops/frame " ++ Nat.show(Stat.mean(Stats.find(d, "ui.ops"))) ++ "  hub busy " ++ Hud.pct(Stats.total(Stats.kind(Stats.prefix(d, "hub."), 0)), ms),
      "cpu " ++ Hud.pct(Nat.mul(Stat.sum(Stats.find(d, "proc.cpu_ms")), 1000n), ms) ++ "  rss " ++ Nat.show(Nat.div(Stat.last(Stats.find(d, "proc.rss_kb")), 1024n)) ++ " MB",
      "heaviest             ms total     n"],
    Hud.top(3n, Stats.by_time(Stats.kind(Stats.prefix(d, "hub."), 0))),
    Hud.top(3n, Stats.by_time(Stats.kind(Stats.prefix(d, "ui.msg."), 0)))])

# cpu time is a gauge that only grows: the overlay wants its growth
def Hud.cpu(d: List<&2, Stat>, +cur: List<&2, Stat>, +base: List<&2, Stat>) -> List<&2, Stat>:
  Stat{"proc.cpu_ms", 0, 1n, Perf.minus(Stat.last(Stats.find(cur, "proc.cpu_ms")), Stat.last(Stats.find(base, "proc.cpu_ms"))), 0n, 0n, Nil{}} <> d

def Hud.text.go(+cur: String, +base: String, +at: Nat, +now: Nat) -> String:
  String.join(Hud.lines(Hud.cpu(Stats.since(Stats.parse(cur), Stats.parse(base)), Stats.parse(cur), Stats.parse(base)), Perf.minus(now, at)), "\n")

def Hud.text.k(first: Bool, +cur: String, +base: String, +at: Nat, +now: Nat) -> String:
  match first:
    case True{}:
      "measuring (ctrl+shift+h hides this)"
    case False{}:
      Hud.text.go(cur, base, at, now)

# the overlay's text for a window: the older reading `base`, taken at
# `at` ms, against `cur` at `now`
def Hud.text(+cur: String, +base: String, +at: Nat, +now: Nat) -> String:
  Hud.text.k(String.is_empty(base), cur, base, at, now)

# the overlay starts a new window once the current one is this old (ms)
def Hud.span() -> Nat:
  3000n

# Logging
# -------
# BACKPLANE_LOG picks how much goes to stderr (and BACKPLANE_LOG_FILE):
# error, warn, info (the default), debug or trace.

def Log.level(+s: String) -> Nat:
  Bool.pick(Nat, String.eq(s, "error"), 0n, Bool.pick(Nat, String.eq(s, "warn"), 1n,
    Bool.pick(Nat, String.eq(s, "debug"), 3n, Bool.pick(Nat, String.eq(s, "trace"), 4n, 2n))))

def Log.name(l: Nat) -> String:
  match l:
    case 0n:
      "error"
    case 1n:
      "warn"
    case 2n:
      "info"
    case 3n:
      "debug"
    case _:
      "trace"

# a line at level l shows under threshold th; errors always do
def Log.on(l: Nat, +th: Nat) -> Bool:
  match l:
    case 0n:
      True{}
    case 1n+k:
      Nat.is_le(1n+k, th)

def Perf.two(+n: Nat) -> String:
  Bool.pick(String, Nat.is_lt(n, 10n), "0", "") ++ Nat.show(n)

# epoch seconds as the time of day (UTC): 3723 -> "01:02:03"
def Log.clock(+t: Nat) -> String:
  Perf.two(Nat.mod(Nat.div(t, 3600n), 24n)) ++ ":" ++ Perf.two(Nat.mod(Nat.div(t, 60n), 60n)) ++ ":" ++ Perf.two(Nat.mod(t, 60n))

def Log.line(+l: Nat, +t: Nat, +area: String, +msg: String) -> String:
  "backplane " ++ Log.clock(t) ++ " " ++ Log.name(l) ++ " " ++ area ++ ": " ++ msg
