import Base
import ./check.bend as C
import ../src/core/storedb.bend as S
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/thin.bend as T
import ../src/core/thinq.bend as Q
import ../src/core/thin-wire.bend as W
import ../src/server/thin-view.bend as V
import ../src/server/thinstore.bend as TS
import ../src/server/db.bend as D

# The thin views from the store (server/thinstore.bend over core/thinq.bend)
# answer exactly what the whole in-memory index (T.Index.of) answers: every
# page of every thread at every anchor, every message read, every search
# page with its cursor chain, and the recent threads. The store is made by
# S1's pure materializer (S.Mat) from a generated log and written to a
# SQLite file through D.Db.

# A log
# -----

def th.of.k(+r: Nat, +i: Nat) -> String:
  match r:
    case 0n: "ta"
    case 1n: "tb"
    case _: "tc"

def th.of(+i: Nat) -> String:
  th.of.k(Nat.mod(i, 3n), i)

def th.other.k(+r: Nat, +i: Nat) -> String:
  match r:
    case 0n: "tb"
    case 1n: "tc"
    case _: "ta"

def th.other(+i: Nat) -> String:
  th.other.k(Nat.mod(i, 3n), i)

def word.if(on: Bool, w: String) -> String:
  Bool.pick(String, on, w ++ " ", "")

def text.of(+i: Nat) -> String:
  word.if(Nat.is_eq(Nat.mod(i, 2n), 1n), "Alpha") ++ word.if(Nat.is_eq(Nat.mod(i, 3n), 0n), "beta") ++ word.if(Nat.is_eq(Nat.mod(i, 7n), 0n), "日本語") ++
  word.if(Nat.is_eq(Nat.mod(i, 11n), 0n), String.repeat("ü", 300n)) ++ word.if(Nat.is_eq(Nat.mod(i, 17n), 0n), String.repeat("x", 600n)) ++ "common n" ++ Nat.show(i)

def id.of(+i: Nat) -> String:
  Bool.pick(String, Nat.is_eq(Nat.mod(i, 13n), 0n), "dup", "e" ++ Nat.show(i))

def change.of.k(+r: Nat, +i: Nat) -> M.Change:
  match r:
    case 0n: M.MessagePosted{th.of(i), id.of(i), M.User{}, text.of(i), Nat.mod(Nat.mul(i, 7n), 50n)}
    case 1n: M.MessagePosted{th.of(i), id.of(i), M.Assistant{}, text.of(i), i}
    case 2n: M.ActivityLogged{th.of(i), id.of(i), "tool", "Bash", text.of(i), i}
    case 3n: M.MessagePosted{th.other(i), M.Link.id(th.of(i), Nat.show(i)), M.User{}, text.of(i), Nat.mod(Nat.mul(i, 3n), 40n)}
    case _: M.Reminded{"r" ++ Nat.show(i), "b1", th.of(i), text.of(i), i}

def change.of(+i: Nat) -> M.Change:
  change.of.k(Nat.mod(i, 5n), i)

def changes.go(+n: Nat, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match n:
    case 0n: acc
    case 1n+k: changes.go(k, change.of(k) <> acc)

def entries() -> Nat:
  170n

def threads() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "P", "/p", 1n}, M.ThreadCreated{"ta", "p1", "A", M.Local{}, "", "", "claude", "", 3n},
   M.ThreadCreated{"tb", "p1", "B", M.Local{}, "", "", "claude", "", 9n}, M.ThreadCreated{"tc", "p1", "C", M.Local{}, "", "", "claude", "", 1n},
   M.ThreadCreated{"td", "p1", "D", M.Local{}, "", "", "claude", "", 60n}]

def changes() -> List<&2, M.Change>:
  List.append(&2, M.Change, threads(), changes.go(entries(), Nil{}))

def lines(cs: List<&2, M.Change>) -> List<&2, S.LL>:
  match cs:
    case Nil{}: Nil{}
    case c <> t: S.LL{20n, True{}, Some{c}} <> lines(t)

def state() -> M.State:
  M.State.replay(changes(), M.State.empty())

# The store
# ---------

def table.of(+k: String) -> String:
  Maybe.default(&2, String, List.head(&2, String, String.split(k, '\u{1f}')), "")

def items(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> List<&2, List<&2, String>>:
  match xs:
    case Nil{}: Nil{}
    case (+k, +v) <> t: S.Sql.of(S.Put{table.of(k), String.split(v, '\u{1f}')}) <> items(t)

def dir.of(r: Result<&1, &1, U32 & String, U32 & (String & String)>) -> IO(String):
  match r:
    case Done{(_, (out, _))}: IO.pure(String, String.trim(out))
    case Fail{(_, e)}: IO.die(String, 1, "thinstore_test: mktemp failed: " ++ e)

# Comparing many answers: how many, how many differ, and the first that did
type Acc is Data:
  Acc{n: Nat, bad: Nat, first: String}

def Acc.see.k(same: Bool, +a: Acc, name: String, got: String, want: String) -> Acc:
  match same a:
    case True{} Acc{n, b, f}: Acc{1n+n, b, f}
    case False{} Acc{n, b, f}: Acc{1n+n, 1n+b, Bool.pick(String, String.is_empty(f), name ++ "\n got  " ++ got ++ "\n want " ++ want, f)}

def Acc.see(+a: Acc, name: String, +got: String, +want: String) -> Acc:
  Acc.see.k(C.Check.same(got, want), a, name, got, want)

def Acc.report(+name: String, a: Acc) -> IO(Unit):
  match a:
    case Acc{n, 0n, _}: IO.print("ok " ++ name ++ " (" ++ Nat.show(n) ++ " compared)")
    case Acc{n, b, f}: IO.print("FAIL " ++ name ++ ": " ++ Nat.show(b) ++ " of " ++ Nat.show(n) ++ " differ; first: " ++ f)

# Pages
# -----

def request(+th: String) -> J.Json:
  J.Json.obj([J.KV{"hub", J.Str{"h"}}, J.KV{"target", V.Target.json("thread", th)}, J.KV{"anchor", J.Json.obj(Nil{})}])

def show.entries(es: List<&2, M.Entry>) -> String:
  match es:
    case Nil{}: ""
    case +e <> t: M.Idx.id(e) ++ "@" ++ M.Idx.thread(e) ++ "=" ++ T.Entry.text(e) ++ ";" ++ show.entries(t)

# what a thin render reads of the index for one page
def show.page(+i: T.Index, +th: String, +ids: List<&2, String>) -> String:
  String.join(ids, ",") ++ "\n" ++ J.Json.show(V.Page.window(i, th, request(th), "ep", ids)) ++ "\n" ++ show.entries(T.Page.entries(ids, i, Nil{})) ++ "\n" ++ String.join(T.Index.recent(i), ",")

def pages.th(+n: Nat, +i: T.Index, +r: T.Index, +h: String, +th: String, +acc: Acc) -> IO(Acc):
  match n:
    case 0n: IO.pure(Acc, acc)
    case 1n+k:
      do IO<Acc>:
        +before : Nat = Nat.add(Nat.mul(k, 5n), Nat.mod(k, 3n))
        +want : String = show.page(i, th, T.Index.page(i, th, before, 32n))
        +pg : TS.Page <- TS.Ts.page(h, th, before, 32n, r)
        pages.th(k, i, r, h, th, Acc.see(acc, th ++ " before " ++ Nat.show(before), show.page(TS.Page.index(pg), th, TS.Page.ids(pg)), want))

def pages(ths: List<&2, String>, +i: T.Index, +r: T.Index, +h: String, +acc: Acc) -> IO(Acc):
  match ths:
    case Nil{}: IO.pure(Acc, acc)
    case +th <> t:
      do IO<Acc>:
        +a : Acc <- pages.th(36n, i, r, h, th, acc)
        pages(t, i, r, h, a)

# Message reads
# -------------

def read.p(+th: String, +id: String, +cursor: String) -> J.Json:
  J.Json.obj([J.KV{"target", V.Target.json("thread", th)}, J.KV{"entryId", J.Str{id}}, J.KV{"cursor", J.Str{cursor}}])

def show.read(+r: V.MessageRead, +p: J.Json) -> String:
  match r:
    case V.MessageRead{+code, +row, +start}:
      code ++ "|" ++ Nat.show(T.Row.n(row)) ++ "|" ++ Nat.show(start) ++ "|" ++ Bool.pick(String, String.is_empty(code), J.Json.show(V.Message.result(row, start, p, "ep")), "")

def read.store(+h: String, +th: String, +anchor: Nat, +p: J.Json, +r: T.Index, +st: M.State) -> IO(String):
  do IO<String>:
    +pg : TS.Page <- TS.Ts.page(h, th, anchor, 32n, r)
    +whole : TS.Page <- TS.Ts.whole(h, th, TS.Ts.cursor.ord(p), J.Json.get_str(p, "entryId", ""), pg)
    return show.read(V.Message.read(TS.Page.index(whole), th, p, "ep", TS.Page.ids(pg), st), p)

def read.one(+h: String, +th: String, +anchor: Nat, +p: J.Json, +i: T.Index, +r: T.Index, +st: M.State, +acc: Acc) -> IO(Acc):
  do IO<Acc>:
    +got : String <- read.store(h, th, anchor, p, r, st)
    return Acc.see(acc, th ++ " @" ++ Nat.show(anchor) ++ " " ++ J.Json.show(p), got, show.read(V.Message.read(i, th, p, "ep", T.Index.page(i, th, anchor, 32n), st), p))

# every row of the page (and a foreign id), first read and continued
def reads.ids(ids: List<&2, String>, +h: String, +th: String, +anchor: Nat, +i: T.Index, +r: T.Index, +st: M.State, +acc: Acc) -> IO(Acc):
  match ids:
    case Nil{}: read.one(h, th, anchor, read.p(th, "nope", ""), i, r, st, acc)
    case +o <> t:
      do IO<Acc>:
        +id : String = M.Idx.id(T.Row.entry(T.Index.row(i, V.Page.number(o))))
        +a1 : Acc <- read.one(h, th, anchor, read.p(th, id, ""), i, r, st, acc)
        +a2 : Acc <- read.one(h, th, anchor, read.p(th, id, W.Cursor.write("ep", V.Target.json("thread", th), "message:" ++ o, 1n)), i, r, st, a1)
        +a3 : Acc <- read.one(h, th, anchor, read.p(th, id, W.Cursor.write("ep", V.Target.json("thread", th), "message:" ++ o, 2n)), i, r, st, a2)
        reads.ids(t, h, th, anchor, i, r, st, a3)

def reads.at(anchors: List<&2, Nat>, +h: String, +th: String, +i: T.Index, +r: T.Index, +st: M.State, +acc: Acc) -> IO(Acc):
  match anchors:
    case Nil{}: IO.pure(Acc, acc)
    case +a <> t:
      do IO<Acc>:
        +a1 : Acc <- reads.ids(T.Index.page(i, th, a, 32n), h, th, a, i, r, st, acc)
        reads.at(t, h, th, i, r, st, a1)

def reads(ths: List<&2, String>, +anchors: List<&2, Nat>, +h: String, +i: T.Index, +r: T.Index, +st: M.State, +acc: Acc) -> IO(Acc):
  match ths:
    case Nil{}: IO.pure(Acc, acc)
    case +th <> t:
      do IO<Acc>:
        +a : Acc <- reads.at(anchors, h, th, i, r, st, acc)
        reads(t, anchors, h, i, r, st, a)

# Search
# ------

def search.p(+th: String, +q: String) -> J.Json:
  J.Json.obj([J.KV{"target", V.Target.json("thread", th)}, J.KV{"query", J.Str{q}}])

def search.next(+sp: T.SearchPage) -> Nat:
  match sp:
    case T.SearchPage{_, _, +last, True{}}: V.Page.number(last)
    case _: 0n

# one query's pages, following the cursor until there are no more
def search.chain(+fuel: Nat, go: Bool, +h: String, +q: String, +th: String, +before: Nat, +i: T.Index, +acc: Acc) -> IO(Acc):
  match fuel go:
    case 0n _: IO.pure(Acc, acc)
    case _ False{}: IO.pure(Acc, acc)
    case 1n+f True{}:
      do IO<Acc>:
        +want : T.SearchPage = T.Index.search(i, q, th, before)
        +got : TS.Found <- TS.Ts.search(h, q, th, before)
        +a : Acc = Acc.see(acc, "search " ++ q ++ " in " ++ th ++ " before " ++ Nat.show(before),
          J.Json.show(V.Search.result(TS.Found.page(got), TS.Found.index(got), search.p(th, q), "ep")), J.Json.show(V.Search.result(want, i, search.p(th, q), "ep")))
        search.chain(f, Nat.is_gt(search.next(want), 0n), h, q, th, search.next(want), i, a)

# the top bar's search (every thread, the open one's results first on
# the first page): the store's (TS.Ts.search.top) is the index's
def top.here(first: Bool, +i: T.Index, +q: String, +th: String) -> T.SearchPage:
  match first:
    case True{}: T.Index.search.in(i, q, th)
    case False{}: T.Search.none()

def top.both(+here: T.SearchPage, +i: T.Index, +q: String, +hide: Map<&2, Bool>, +before: Nat) -> T.SearchPage:
  T.Search.join(here, T.Index.search.rest(i, q, hide, T.Search.left(here), before))

def top.want(+i: T.Index, +q: String, +th: String, +hide: Map<&2, Bool>, +before: Nat) -> T.SearchPage:
  top.both(top.here(Bool.and(Nat.is_eq(before, 0n), Bool.not(String.is_empty(th))), i, q, th), i, q, T.Search.gone.but(hide, th), before)

def top.chain(+fuel: Nat, go: Bool, +h: String, +q: String, +th: String, +before: Nat, +i: T.Index, +hide: Map<&2, Bool>, +acc: Acc) -> IO(Acc):
  match fuel go:
    case 0n _: IO.pure(Acc, acc)
    case _ False{}: IO.pure(Acc, acc)
    case 1n+f True{}:
      do IO<Acc>:
        +want : T.SearchPage = top.want(i, q, th, hide, before)
        +got : TS.Found <- TS.Ts.search.top(h, q, th, hide, before)
        +a : Acc = Acc.see(acc, "every thread's search " ++ q ++ " from " ++ th ++ " before " ++ Nat.show(before),
          J.Json.show(V.Search.result(TS.Found.page(got), TS.Found.index(got), search.p(th, q), "ep")), J.Json.show(V.Search.result(want, i, search.p(th, q), "ep")))
        top.chain(f, Nat.is_gt(search.next(want), 0n), h, q, th, search.next(want), i, hide, a)

def searches.th(ths: List<&2, String>, +h: String, +q: String, +i: T.Index, +acc: Acc) -> IO(Acc):
  match ths:
    case Nil{}: IO.pure(Acc, acc)
    case +th <> t:
      do IO<Acc>:
        +a : Acc <- search.chain(30n, True{}, h, q, th, 0n, i, acc)
        +a2 : Acc <- search.chain(3n, True{}, h, q, th, 77n, i, a)
        +a3 : Acc <- top.chain(30n, True{}, h, q, th, 0n, i, Map.set(&2, Bool, Map.new(&2, Bool), "tb", True{}), a2)
        searches.th(t, h, q, i, a3)

def searches(qs: List<&2, String>, +ths: List<&2, String>, +h: String, +i: T.Index, +acc: Acc) -> IO(Acc):
  match qs:
    case Nil{}: IO.pure(Acc, acc)
    case +q <> t:
      do IO<Acc>:
        +a : Acc <- searches.th(ths, h, q, i, acc)
        searches(t, ths, h, i, a)

def main() -> IO(Unit):
  do IO<Unit>:
    made : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("mktemp", ["-d"], "", 4096, 10000)
    +dir : String <- dir.of(made)
    +h0 : String <- D.Db.open(dir ++ "/store.db")
    +h : String = String.drop(h0, 3n)
    sc : String <- D.Db.exec(h, S.Sdb.schema())
    C.Check.str("schema", sc, "ok")
    +m : S.Mat = S.Mat.replay(lines(changes()), S.Mat.empty())
    w : String <- D.Db.script(h, items(Map.to_list(&2, String, S.Mat.facts(m))))
    C.Check.str("facts written", String.take(w, 2n), "ok")
    +st : M.State = state()
    +i : T.Index = T.Index.of(st)
    C.Check.str("the index holds every entry", Nat.show(T.Index.n(i)), Nat.show(entries()))
    +r : T.Index <- TS.Ts.recent(h, st)
    C.Check.str("recent threads", String.join(T.Index.recent(r), ","), String.join(T.Index.recent(i), ","))
    +ths : List<&2, String> = ["ta", "tb", "tc", "td", "", "zz"]
    +a1 : Acc <- pages(ths, i, r, h, Acc{0n, 0n, ""})
    Acc.report("every page of every thread at every anchor", a1)
    +a2 : Acc <- reads(["ta", "tb", "tc"], [0n, 100n, 40n], h, i, r, st, Acc{0n, 0n, ""})
    Acc.report("message reads, first and continued", a2)
    +a3 : Acc <- searches(["common", "alpha", "beta alpha", "日本語", "absent", "", "COMMON n13", "common absent", "n7", "üüü"], ["ta", "tc", "", "zz"], h, i, Acc{0n, 0n, ""})
    Acc.report("search pages and their cursors", a3)
    cl : String <- D.Db.close(h)
    rm : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("rm", ["-rf", dir], "", 4096, 10000)
    return Unit{}
