import Base
import ../core/json.bend as J
import ../core/model.bend as M
import ../core/uctx.bend as UC
import ../core/tz.bend as TZ
import ../core/sched.bend as SC
import ./db.bend as D

# The uberbot's infinite context: its reads (core/uctx.bend is the pure
# part; server.bend's Ucs.* and UctxJob.* run them)
# =====================================================================
# Everything comes from store.db: the thread's timeline (entry), its
# memory lines (ctx) and their full-text index (entry_fts). Nothing is
# kept between calls, so the hub holds no part of the tree.

def Ux.big() -> U32:
  4000000000

def Ux.q(+h: String, sql: String, ps: List<&2, D.DbVal>) -> IO(List<&2, List<&2, String>>):
  D.Db.query.t(h, sql, ps, Ux.big())

# an answer that is Db.query's error row
def Ux.failed(rs: List<&2, List<&2, String>>) -> Bool:
  match rs:
    case (+c <> _) <> _:
      String.starts_with(c, "\u{0}")
    case _:
      False{}

def Ux.why(rs: List<&2, List<&2, String>>) -> String:
  match rs:
    case (_ <> +w <> _) <> _:
      w
    case _:
      "?"

# the first column of the first row ("0" when none, or an error)
def Ux.first(rs: List<&2, List<&2, String>>) -> String:
  match rs:
    case (+c <> _) <> _:
      Bool.pick(String, String.starts_with(c, "\u{0}"), "0", c)
    case _:
      "0"

def Ux.json(xs: List<&2, J.Json>) -> D.DbVal:
  D.DbText{J.Json.show(J.Json.arr(xs))}

# where a new tree starts: the Uctx.back()-th last message, or the first
def Ux.start(+h: String, +th: String) -> IO(String):
  IO.bind(List<&2, List<&2, String>>, String, Ux.q(h, UC.Uctx.sql.start(), [D.DbText{th}, D.DbInt{Nat.show(Nat.sub(UC.Uctx.back(), 1n))}]), rs => IO.pure(String, Ux.first(rs)))

# The view
# --------

def Ux.view.k(bad: Bool, +h: String, +th: String, +start: String, rs: List<&2, List<&2, String>>) -> IO(String):
  match bad:
    case True{}:
      IO.pure(String, UC.Uctx.doc() ++ "\n(your memory could not be read now: " ++ Ux.why(rs) ++ ")")
    case False{}:
      do IO<String>:
        +ms : List<&2, UC.UMsg> = UC.Uctx.msgs(rs)
        lr : List<&2, List<&2, String>> <- Ux.q(h, UC.Uctx.sql.lined(), [D.DbText{th}])
        +lined : Map<&2, Bool> = UC.Uctx.lined(lr, Map.new(&2, Bool))
        +n : Nat = Nat.sub(List.length(&2, UC.UMsg, ms), UC.Uctx.own(ms))
        +vw : List<&2, UC.UNode> = UC.Uctx.view(n, UC.Uctx.fit(ms, 0n, lined))
        xr : List<&2, List<&2, String>> <- Ux.q(h, UC.Uctx.sql.lines(), [D.DbText{th}, Ux.json(UC.Uctx.want.lines(vw, lined))])
        lv : List<&2, List<&2, String>> <- Ux.q(h, UC.Uctx.sql.leaves(), [D.DbText{th}, D.DbInt{start}, Ux.json(UC.Uctx.want.leaves(vw, lined))])
        return UC.Uctx.block(vw, UC.Uctx.leaf.texts(lv, UC.Uctx.texts(xr, Map.new(&2, String))), n)

# the view a fresh turn of the uberbot starts with (its own new messages
# left out: they come whole)
def Ux.view(+h: String, +th: String, +start: String) -> IO(String):
  IO.bind(List<&2, List<&2, String>>, String, Ux.q(h, UC.Uctx.sql.msgs(), [D.DbText{th}, D.DbInt{start}]), rs => Ux.view.k(Ux.failed(rs), h, th, start, rs))

# Compaction sources
# ------------------

# rows (n, kind, role, text, at): the first message whole, with who wrote it
def Ux.whole(rs: List<&2, List<&2, String>>) -> String:
  match rs:
    case (_ <> +k <> +r <> +x <> _) <> _:
      UC.Uctx.who(k, r) ++ ": " ++ String.take(x, 24000n)
    case _:
      ""

def Ux.halves.leaves(+k: Nat, +i: Nat) -> List<&2, J.Json>:
  Bool.pick(List<&2, J.Json>, Nat.is_eq(k, 0n), [J.Num{Nat.show(Nat.mul(2n, i))}, J.Num{Nat.show(Nat.add(Nat.mul(2n, i), 1n))}], Nil{})

def Ux.pair(+k: Nat, +i: Nat, +texts: Map<&2, String>) -> String:
  UC.Uctx.flat(UC.Uctx.got(texts, UC.Uctx.key(k, Nat.mul(2n, i)))) ++ "\n" ++ UC.Uctx.flat(UC.Uctx.got(texts, UC.Uctx.key(k, Nat.add(Nat.mul(2n, i), 1n))))

# what node (l, i) compresses: message i whole, or its halves' lines
def Ux.source(l: Nat, +h: String, +th: String, +start: String, +i: Nat) -> IO(String):
  match l:
    case 0n:
      IO.bind(List<&2, List<&2, String>>, String, Ux.q(h, UC.Uctx.sql.leaves(), [D.DbText{th}, D.DbInt{start}, Ux.json([J.Num{Nat.show(i)}])]), rs => IO.pure(String, Ux.whole(rs)))
    case 1n+k:
      do IO<String>:
        xr : List<&2, List<&2, String>> <- Ux.q(h, UC.Uctx.sql.lines(), [D.DbText{th}, Ux.json([J.Str{UC.Uctx.key(k, Nat.mul(2n, i))}, J.Str{UC.Uctx.key(k, Nat.add(Nat.mul(2n, i), 1n))}])])
        lv : List<&2, List<&2, String>> <- Ux.q(h, UC.Uctx.sql.leaves(), [D.DbText{th}, D.DbInt{start}, Ux.json(Ux.halves.leaves(k, i))])
        return Ux.pair(k, i, UC.Uctx.leaf.texts(lv, UC.Uctx.texts(xr, Map.new(&2, String))))

# Zoom and search
# ---------------

def Ux.date(+st: M.State, +at: String) -> String:
  TZ.Tz.show(SC.Sched.person(st, 0n, True{}), UC.Uctx.num(at, 0n))

def Ux.zoom.one(rs: List<&2, List<&2, String>>, +st: M.State, +id: Nat, +at: Nat) -> String:
  match rs:
    case (_ <> +k <> +r <> +x <> +d <> _) <> _:
      UC.Uctx.whole(id, UC.Uctx.who(k, r), Ux.date(st, d), x, at)
    case _:
      "No message " ++ Nat.show(id) ++ "."

def Ux.zoom.k(ok: Bool, +n1: Bool, +h: String, +st: M.State, +th: String, +start: String, +id: Nat, +n: Nat, +at: Nat, +count: Nat) -> IO(String):
  match ok n1:
    case False{} _:
      IO.pure(String, UC.Uctx.no.part(id, n, count))
    case True{} True{}:
      IO.bind(List<&2, List<&2, String>>, String, Ux.q(h, UC.Uctx.sql.leaves(), [D.DbText{th}, D.DbInt{start}, Ux.json([J.Num{Nat.show(id)}])]), rs => IO.pure(String, Ux.zoom.one(rs, st, id, at)))
    case True{} False{}:
      do IO<String>:
        xr : List<&2, List<&2, String>> <- Ux.q(h, UC.Uctx.sql.lines(), [D.DbText{th}, Ux.json(UC.Uctx.halves.keys(id, n))])
        lv : List<&2, List<&2, String>> <- Ux.q(h, UC.Uctx.sql.leaves(), [D.DbText{th}, D.DbInt{start}, Ux.json(Bool.pick(List<&2, J.Json>, Nat.is_eq(n, 2n), [J.Num{Nat.show(id)}, J.Num{Nat.show(Nat.add(id, 1n))}], Nil{}))])
        return UC.Uctx.halves(id, n, UC.Uctx.leaf.texts(lv, UC.Uctx.texts(xr, Map.new(&2, String))))

# memory_zoom: line id+n opened, or message id whole (a page from at)
def Ux.zoom(+h: String, +st: M.State, +th: String, +start: String, +id: Nat, +n: Nat, +at: Nat) -> IO(String):
  do IO<String>:
    rs : List<&2, List<&2, String>> <- Ux.q(h, UC.Uctx.sql.msgs(), [D.DbText{th}, D.DbInt{start}])
    +count : Nat = List.length(&2, UC.UMsg, UC.Uctx.msgs(rs))
    Ux.zoom.k(UC.Uctx.part.ok(id, n, count), Nat.is_eq(n, 1n), h, st, th, start, id, n, at, count)

def Ux.hits(rs: List<&2, List<&2, String>>, +st: M.State) -> List<&2, String>:
  match rs:
    case (+n <> +k <> +r <> +x <> +d <> _) <> t:
      UC.Uctx.hit(n, UC.Uctx.who(k, r), x, Ux.date(st, d)) <> Ux.hits(t, st)
    case _ <> t:
      Ux.hits(t, st)
    case Nil{}:
      Nil{}

def Ux.found(rs: List<&2, List<&2, String>>, +st: M.State) -> String:
  Bool.pick(String, Ux.failed(rs), "The search failed: " ++ Ux.why(rs),
    Bool.pick(String, List.is_empty(&2, List<&2, String>, rs), "Nothing found.", String.join(Ux.hits(rs, st), "\n")))

def Ux.search.k(none: Bool, +h: String, +st: M.State, +th: String, +start: String, +q: String) -> IO(String):
  match none:
    case True{}:
      IO.pure(String, "Give some words to find.")
    case False{}:
      IO.bind(List<&2, List<&2, String>>, String, Ux.q(h, UC.Uctx.sql.search(), [D.DbText{q}, D.DbText{th}, D.DbInt{start}]), rs => IO.pure(String, Ux.found(rs, st)))

# memory_search: the thread's messages with these words, newest first
def Ux.search(+h: String, +st: M.State, +th: String, +start: String, +words: String) -> IO(String):
  Ux.search.k(String.is_empty(UC.Uctx.fts(words)), h, st, th, start, UC.Uctx.fts(words))
