import Base
import ../../src/core/autosettle.bend as A
import ../../src/core/snapshot.bend as SN
import ../../src/core/storedb.bend as DB
import ../../src/core/model.bend as M
import ../../src/core/json.bend as J
import ../../src/server/hotboot.bend as H

# Only pure functions are emitted. The driver supplies rows from a copied,
# read-only SQLite snapshot; it cannot send a message or settle live threads.
def row.go(xs: List<&2, J.Json>, acc: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}: List.reverse(&2, String, acc)
    case x <> rest: row.go(rest, SN.Snap.str.j(x) <> acc)

def row(xs: List<&2, J.Json>) -> List<&2, String>:
  row.go(xs, Nil{})

def rows.go(xs: List<&2, J.Json>, acc: List<&2, List<&2, String>>) -> List<&2, List<&2, String>>:
  match xs:
    case Nil{}: List.reverse(&2, List<&2, String>, acc)
    case x <> rest: rows.go(rest, row(J.Json.elems(x)) <> acc)

def rows(xs: List<&2, J.Json>) -> List<&2, List<&2, String>>:
  rows.go(xs, Nil{})

def table(xs: List<&2, J.Json>, m: Map<&2, List<&2, List<&2, String>>>) -> Map<&2, List<&2, List<&2, String>>>:
  match xs:
    case name <> data <> _: Map.set(&2, List<&2, List<&2, String>>, m, SN.Snap.str.j(name), rows(J.Json.elems(data)))
    case _: m

def tables(xs: List<&2, J.Json>, m: Map<&2, List<&2, List<&2, String>>>) -> Map<&2, List<&2, List<&2, String>>>:
  match xs:
    case Nil{}: m
    case x <> rest: tables(rest, table(J.Json.elems(x), m))

def state(input: String) -> M.State:
  SN.Snap.state(SN.SRows{tables(J.Json.elems(Maybe.default(&2, J.Json, J.Json.parse(input), J.Null{})), Map.new(&2, List<&2, List<&2, String>>))})

def ids(cs: List<&2, M.Change>) -> List<&2, J.Json>:
  match cs:
    case Nil{}: Nil{}
    case M.ThreadSettled{id, _, _} <> rest: J.Str{id} <> ids(rest)
    case _ <> rest: ids(rest)

def report(+s: M.State, now: Nat) -> String:
  +out = ids(A.Auto.upgrade(s, now))
  J.Json.show(J.Json.obj([J.KV{"wouldSettle", J.Json.arr(out)}, J.KV{"count", J.Json.nat(List.length(&2, J.Json, out))}, J.KV{"sequence", J.Json.nat(M.State.seq(s))}]))

def run(input: String, now: Nat) -> String:
  report(state(input), now)

def columns(table: String) -> String:
  String.join(DB.Sdb.cols(table), ", ")

def window() -> String:
  H.Hot.q.window()

def open_ids(input: String) -> String:
  H.Hot.ids(SN.SRows{tables(J.Json.elems(Maybe.default(&2, J.Json, J.Json.parse(input), J.Null{})), Map.new(&2, List<&2, List<&2, String>>))})
