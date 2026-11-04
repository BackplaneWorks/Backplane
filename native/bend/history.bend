import Base
import ../../src/core/model.bend as M
import ../../src/core/thin.bend as T
import ../../src/core/thinq.bend as Q
import ./kcommon.bend as C

# History kernel, pure part only: the 49 rows the store's page query answers
# (src/server/thinstore.bend Ts.page; the SQL itself is an effect with no C
# twin) become Rows (Q.Q.rows: entry + 512-byte blocks), the page index
# (T.Index.window) and the page's ids (T.Index.page). 16 queries per run.
# Mirrors test/gpui/bend/ts/history.ts.

def H.text(k: Nat, +i: Nat) -> String:
  match k:
    case 0n: SNil{}
    case 1n+r: SCon{Chr{Bool.pick(U32, C.K.is(C.K.mod(i, 97n), 96n), 233, 120)}, H.text(r, Nat.add(i, 1n))}

def H.cell.k(+msg: Bool, +q: Nat, +j: Nat, +ord: Nat) -> List<&2, String>:
  [Nat.show(ord), Nat.show(ord), C.K.id("e", ord), C.K.id("t", C.K.mod(q, 4n)), "",
    Bool.pick(String, msg, "msg", "act"),
    Bool.pick(String, msg, Bool.pick(String, C.K.is(C.K.mod(j, 4n), 0n), "user", "assistant"), "tool"),
    Bool.pick(String, msg, "", "bash"),
    H.text(Nat.add(40n, C.K.mod(C.K.h(ord), 1900n)), 0n), Nat.show(Nat.add(1000000n, ord))]

def H.cell.o(+q: Nat, +j: Nat, +ord: Nat) -> List<&2, String>:
  H.cell.k(C.K.is(C.K.mod(j, 2n), 0n), q, j, ord)

def H.cell(+q: Nat, +j: Nat) -> List<&2, String>:
  H.cell.o(q, j, Nat.add(Nat.add(100000n, Nat.mul(q, 1000n)), Nat.sub(49n, j)))

def H.rows(k: Nat, +q: Nat, +j: Nat, acc: List<&2, List<&2, String>>) -> List<&2, List<&2, String>>:
  match k:
    case 0n: List.reverse(&2, List<&2, String>, acc)
    case 1n+r: H.rows(r, q, Nat.add(j, 1n), H.cell(q, j) <> acc)

def H.queries(k: Nat, +q: Nat, acc: List<&2, List<&2, List<&2, String>>>) -> List<&2, List<&2, List<&2, String>>>:
  match k:
    case 0n: List.reverse(&2, List<&2, List<&2, String>>, acc)
    case 1n+r: H.queries(r, Nat.add(q, 1n), H.rows(49n, q, 0n, Nil{}) <> acc)

def H.lens(k: Nat, +b: T.Blocks, +n: Nat, +acc: Nat) -> Nat:
  match k:
    case 0n: acc
    case 1n+r: H.lens(r, b, Nat.add(n, 1n), C.K.mix(acc, String.length(T.Blocks.get(b, n))))

def H.row.b(+n: Nat, +b: T.Blocks) -> Nat:
  H.lens(T.Blocks.n(b), b, 0n, C.K.mix(n, T.Blocks.n(b)))

def H.row(r: T.Row) -> Nat:
  match r:
    case T.Row{n, _, _, b}: H.row.b(n, b)

def H.sum(rs: List<&2, T.Row>, +acc: Nat) -> Nat:
  match rs:
    case Nil{}: acc
    case r <> rest: H.sum(rest, Nat.mod(Nat.add(acc, H.row(r)), 4294967291n))

def H.ids(xs: List<&2, String>, +acc: Nat) -> Nat:
  match xs:
    case Nil{}: acc
    case x <> rest: H.ids(rest, C.K.mix(acc, C.K.digits(x, 0n)))

def H.page(+i: T.Index, +th: String) -> Nat:
  Nat.mod(Nat.add(H.ids(T.Index.page(i, th, 0n, 32n), 0n), H.sum(Map.values(&2, T.Row, T.Index.rows(i)), 0n)), 4294967291n)

def H.query(xs: List<&2, List<&2, String>>, +q: Nat) -> Nat:
  H.page(T.Index.window(Q.Q.rows(xs), C.K.id("t", C.K.mod(q, 4n)), 123n, T.Index.empty()), C.K.id("t", C.K.mod(q, 4n)))

def H.once(qs: List<&2, List<&2, List<&2, String>>>, +q: Nat, +acc: Nat) -> Nat:
  match qs:
    case Nil{}: acc
    case x <> rest: H.once(rest, Nat.add(q, 1n), C.K.mix(acc, H.query(x, q)))

def H.many(k: Nat, +qs: List<&2, List<&2, List<&2, String>>>, +acc: Nat) -> Nat:
  match k:
    case 0n: acc
    case 1n+rest: H.many(rest, qs, H.once(qs, 0n, 0n))

def H.reps(k: Nat, +qs: List<&2, List<&2, List<&2, String>>>, +inner: Nat) -> IO(Unit):
  match k:
    case 0n: IO.pure(Unit, Unit{})
    case 1n+rest:
      do IO<Unit>:
        t0 : Nat <- IO.now()
        +d : Nat = H.many(inner, qs, 0n)
        IO.print("digest " ++ Nat.show(d))
        t1 : Nat <- IO.now()
        IO.print("ms " ++ C.K.fmt(Nat.sub(t1, t0), inner))
        H.reps(rest, qs, inner)

def main() -> IO(Unit):
  do IO<Unit>:
    +a : C.Args <- C.K.args()
    +r : Nat = C.K.Args.b(a)
    t0 : Nat <- IO.now()
    +qs : List<&2, List<&2, List<&2, String>>> = H.queries(16n, 0n, Nil{})
    +len : Nat = List.length(&2, List<&2, List<&2, String>>, qs)
    t1 : Nat <- IO.now()
    IO.print("queries " ++ Nat.show(len))
    IO.print("gen_ms " ++ Nat.show(Nat.sub(t1, t0)))
    H.reps(r, qs, Bool.pick(Nat, Nat.is_eq(C.K.Args.c(a), 0n), 1n, C.K.Args.c(a)))
