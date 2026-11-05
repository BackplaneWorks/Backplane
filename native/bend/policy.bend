import Base
import ../../src/core/model.bend as M
import ../../src/core/thin.bend as T
import ../../src/core/thinq.bend as Q
import ../../src/core/tailnet.bend as TN
import ./kcommon.bend as C

# The policy process of the Rust effects-host spike (native/host). One request
# on stdin, one answer on stdout:
#   hello \t VERSION \t BUSY \t COMMIT     -> hex of TN.Hello.answer.c (the real /hello body)
#   page \t THREAD                          -> then one SQL row per line, cells joined by \t
#                                              (\ \t \n escaped as \\ \t \n): the page query's rows
#                                              -> Q.Q.rows, T.Index.window, T.Index.page
# page answers "ids a,b,c" and one "row ORD len len ..." line per row (block lengths in code points).

def Hx.digit(+n: Nat) -> String:
  match n:
    case 0n: "0"
    case 1n: "1"
    case 2n: "2"
    case 3n: "3"
    case 4n: "4"
    case 5n: "5"
    case 6n: "6"
    case 7n: "7"
    case 8n: "8"
    case 9n: "9"
    case 10n: "a"
    case 11n: "b"
    case 12n: "c"
    case 13n: "d"
    case 14n: "e"
    case 15n+_: "f"

def Hx.byte(+b: U32) -> String:
  Hx.digit(U32.to_nat(U32.shr(U32.shr(U32.shr(U32.shr(b)))))) ++ Hx.digit(Nat.mod(U32.to_nat(b), 16n))

def Hx.of(bs: List<&2, U32>) -> String:
  match bs:
    case Nil{}: ""
    case b <> rest: Hx.byte(b) ++ Hx.of(rest)

# unescape one cell: \\ \t \n
def Un.esc(+c: U32) -> U32:
  Bool.pick(U32, U32.is_eq(c, 116), 9, Bool.pick(U32, U32.is_eq(c, 110), 10, c))

def Un.go(s: String, +acc: String, esc: Bool) -> String:
  match s esc:
    case SNil{} _: String.reverse(acc)
    case SCon{Chr{+c}, tail} True{}:
      Un.go(tail, SCon{Chr{Un.esc(c)}, acc}, False{})
    case SCon{Chr{+c}, tail} False{}:
      Un.go(tail, Bool.pick(String, U32.is_eq(c, 92), acc, SCon{Chr{c}, acc}), U32.is_eq(c, 92))

def Un.cells(xs: List<&2, String>, acc: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}: List.reverse(&2, String, acc)
    case x <> rest: Un.cells(rest, Un.go(x, "", False{}) <> acc)

def Un.keep(empty: Bool, l: String, acc: List<&2, List<&2, String>>) -> List<&2, List<&2, String>>:
  match empty:
    case True{}: acc
    case False{}: Un.cells(String.split(l, '\t'), Nil{}) <> acc

def Un.rows(ls: List<&2, String>, acc: List<&2, List<&2, String>>) -> List<&2, List<&2, String>>:
  match ls:
    case Nil{}: List.reverse(&2, List<&2, String>, acc)
    case +l <> rest: Un.rows(rest, Un.keep(String.is_empty(l), l, acc))

def Out.lens(k: Nat, +b: T.Blocks, +n: Nat) -> String:
  match k:
    case 0n: ""
    case 1n+r: " " ++ Nat.show(String.length(T.Blocks.get(b, n))) ++ Out.lens(r, b, Nat.add(n, 1n))

def Out.row.b(+n: Nat, +b: T.Blocks) -> String:
  "row " ++ Nat.show(n) ++ Out.lens(T.Blocks.n(b), b, 0n) ++ "\n"

def Out.row(r: T.Row) -> String:
  match r:
    case T.Row{n, _, _, b}: Out.row.b(n, b)

def Out.rows(rs: List<&2, T.Row>) -> String:
  match rs:
    case Nil{}: ""
    case r <> rest: Out.row(r) ++ Out.rows(rest)

def Out.ids(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}: ""
    case x <> Nil{}: x
    case x <> rest: x ++ "," ++ Out.ids(rest)

def Pol.page.i(+i: T.Index, +th: String) -> String:
  "ids " ++ Out.ids(T.Index.page(i, th, 0n, 32n)) ++ "\n" ++ Out.rows(Map.values(&2, T.Row, T.Index.rows(i)))

def Pol.page(+th: String, rows: List<&2, List<&2, String>>) -> String:
  Pol.page.i(T.Index.window(Q.Q.rows(rows), th, 0n, T.Index.empty()), th)

def Pol.hello(cs: List<&2, String>) -> String:
  match cs:
    case _ <> v <> b <> c <> Nil{}: Hx.of(TN.Hello.answer.c(v, C.K.digits(b, 0n), c))
    case _: "error hello"

def Pol.mode.k(page: Bool, th: String, rest: List<&2, String>) -> String:
  match page:
    case True{}: Pol.page(th, Un.rows(rest, Nil{}))
    case False{}: "error mode"

def Pol.mode(cs: List<&2, String>, rest: List<&2, String>) -> String:
  match cs:
    case +m <> th <> Nil{}: Pol.mode.k(String.eq(m, "page"), th, rest)
    case +m <> v <> b <> c <> Nil{}: Pol.hello(m <> v <> b <> c <> Nil{})
    case _: "error request"

def Pol.run(ls: List<&2, String>) -> String:
  match ls:
    case Nil{}: "error empty"
    case head <> rest: Pol.mode(String.split(head, '\t'), rest)

def Io.text(res: Result<&1, &1, U32 & String, String>) -> String:
  match res:
    case Done{s}: s
    case Fail{_}: ""

def Io.fin(r: File & Result<&1, &1, U32 & String, String>) -> IO(String):
  (f, res) = r
  do IO<String>:
    File.close(f)
    return Io.text(res)

def Io.read(f: File, n: U32) -> IO(String):
  do IO<String>:
    r : File & Result<&1, &1, U32 & String, String> <- File.read(f, n)
    t : String <- Io.fin(r)
    return t

def Io.size.k(f: File, res: Result<&1, &1, U32 & String, U32>) -> IO(String):
  match res:
    case Done{n}: Io.read(f, n)
    case Fail{_}: IO.pure(String, "")

def Io.size(r: File & Result<&1, &1, U32 & String, U32>) -> IO(String):
  (f, res) = r
  Io.size.k(f, res)

def Io.slurp(path: String) -> IO(String):
  do IO<String>:
    f : File <- IO.try(File, File.open(path, "r"))
    r : File & Result<&1, &1, U32 & String, U32> <- File.size(f)
    t : String <- Io.size(r)
    return t

def Pol.path(xs: List<&1, String>) -> String:
  match xs:
    case _ <> p <> _: p
    case _: ""

def main() -> IO(Unit):
  do IO<Unit>:
    xs : List<&1, String> <- IO.args()
    text : String <- Io.slurp(Pol.path(xs))
    IO.write(Pol.run(String.split(text, '\n')))
