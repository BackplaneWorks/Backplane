import Base
import ./check.bend as C
import ../src/core/hier.bend as H
import ../src/core/utf8.bend as U

# /p/top has power and io; io has adc; power includes top again (a loop)
def tab() -> List<&2, H.Kids>:
  [H.Kids{"/p/top.kicad_sch", [H.Sheet{"power", "/p/power.kicad_sch"}, H.Sheet{"io", "/p/io/io.kicad_sch"}]},
   H.Kids{"/p/power.kicad_sch", [H.Sheet{"again", "/p/top.kicad_sch"}]},
   H.Kids{"/p/io/io.kicad_sch", [H.Sheet{"adc", "/p/io/adc.kicad_sch"}]}]

def Show.nats(xs: List<&2, Nat>) -> String:
  match xs:
    case Nil{}:
      ""
    case x <> t:
      Nat.show(x) ++ " " ++ Show.nats(t)

def Show.sheets(ss: List<&2, H.Sheet>) -> String:
  match ss:
    case Nil{}:
      ""
    case H.Sheet{n, f} <> t:
      n ++ "=" ++ f ++ " " ++ Show.sheets(t)

def Show.maybe(m: Maybe<&2, Nat>) -> String:
  match m:
    case Some{n}:
      Nat.show(n)
    case None{}:
      "none"

def sch() -> String:
  "(kicad_sch (version 20231120)\n  (wire (pts (xy 0 0) (xy 1 1)))\n  (sheet (at 10 10) (size 20 20) (uuid \"u1\")\n    (property \"Sheetname\" \"Power\" (at 10 9 0))\n    (property \"Sheetfile\" \"power.kicad_sch\" (at 10 31 0)))\n  (sheet (at 40 10) (size 20 20) (uuid \"u2\")\n    (property \"Sheet name\" \"IO\" (at 40 9 0))\n    (property \"Sheet file\" \"io/io.kicad_sch\" (at 40 31 0))))\n"

# n sheet rows as Hier.show writes them, one per line
def many(n: Nat) -> String:
  match n:
    case 0n:
      "0\t0\ttop.kicad_sch\t/p/top.kicad_sch"
    case 1n+k:
      many(k) ++ "\n1\t0\tsheet\t/p/sheet.kicad_sch"

def main() -> IO(Unit):
  do IO<Unit>:
    +rs : List<&2, H.Row> = H.Hier.rows(tab(), "/p/top.kicad_sch")
    C.Check.str("rows", H.Hier.show(rs),
      "0\t0\ttop.kicad_sch\t/p/top.kicad_sch\n1\t0\tpower\t/p/power.kicad_sch\n2\t1\tagain\t/p/top.kicad_sch\n1\t0\tio\t/p/io/io.kicad_sch\n2\t0\tadc\t/p/io/adc.kicad_sch")
    C.Check.str("text round trip", H.Hier.show(H.Hier.read(H.Hier.show(rs))), H.Hier.show(rs))
    # each row is read once: 40 rows come back at once (reading the rest
    # twice per row took 2^n reads, minutes at 30 sheets)
    C.Check.str("forty sheets read", Nat.show(List.length(&2, H.Row, H.Hier.read(many(40n)))), "41")
    C.Check.str("chain of adc", Show.nats(H.Hier.chain(rs, 4n)), "0 3 4 ")
    C.Check.str("chain of root", Show.nats(H.Hier.chain(rs, 0n)), "0 ")
    C.Check.str("parent of adc", Nat.show(H.Hier.parent(rs, 4n)), "3")
    C.Check.str("parent of root", Nat.show(H.Hier.parent(rs, 0n)), "0")
    C.Check.str("child io of root", Show.maybe(H.Hier.child(rs, 0n, "io")), "3")
    C.Check.str("child adc of io", Show.maybe(H.Hier.child(rs, 3n, "adc")), "4")
    C.Check.str("no grandchild as child", Show.maybe(H.Hier.child(rs, 0n, "adc")), "none")
    C.Check.str("at follows the file", Nat.show(H.Hier.at(rs, 1n, "/p/io/adc.kicad_sch")), "4")
    C.Check.str("at keeps a repeated file's row", Nat.show(H.Hier.at(rs, 2n, "/p/top.kicad_sch")), "2")
    C.Check.str("at unknown is root", Nat.show(H.Hier.at(rs, 3n, "/elsewhere.kicad_sch")), "0")
    C.Check.str("resolve", H.Hier.resolve("/p/io/io.kicad_sch", "adc.kicad_sch"), "/p/io/adc.kicad_sch")
    C.Check.str("resolve absolute", H.Hier.resolve("/p/top.kicad_sch", "/q/x.kicad_sch"), "/q/x.kicad_sch")
    C.Check.str("resolve bare parent", H.Hier.resolve("top.kicad_sch", "a.kicad_sch"), "a.kicad_sch")
    C.Check.str("sheets", Show.sheets(H.Hier.sheets(U.Utf8.encode(sch()))), "Power=power.kicad_sch IO=io/io.kicad_sch ")
    C.Check.str("kids", Show.sheets(H.Hier.kids.sheets(H.Hier.kids("/p/top.kicad_sch", U.Utf8.encode(sch())))), "Power=/p/power.kicad_sch IO=/p/io/io.kicad_sch ")
    C.Check.str("todo", String.join(H.Hier.todo([H.Sheet{"a", "/p/a"}, H.Sheet{"b", "/p/top.kicad_sch"}, H.Sheet{"c", "/p/a"}], tab(), ["/p/x"]), ","), "/p/x,/p/a")
