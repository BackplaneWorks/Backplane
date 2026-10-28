import Base
import ./check.bend as C
import ../src/core/board.bend as B
import ../src/core/schematic.bend as Sch
import ../src/core/utf8.bend as U

# A sheet's connections (schematic.bend's Conn): which wires, junctions,
# labels, pins and power symbols are one net on the sheet, as each item's
# info says (";g=<tag>"). R1 and R2 at (100,100) and (120,100), their pins
# 1 at y 96.19 and 2 at y 103.81:
# - VBUS: R1.1 up a wire to y 90, along to x 120 and down to R2.1; the
#   label VBUS sits on that wire's middle; a wire at y 93 meets the right
#   one at a junction, and crosses the left one with none
# - GND: R1.2 down to one GND symbol, R2.2 down to another (by name)
# - apart: two wires crossing at (80,85) with no junction; a T at (60,90)

def sheet() -> String:
  "(kicad_sch (version 20250114)
  (lib_symbols
    (symbol \"Device:R\" (pin_numbers (hide yes)) (pin_names (offset 0))
      (property \"Reference\" \"R\" (at 2 0 90) (effects (font (size 1.27 1.27))))
      (symbol \"R_0_1\" (rectangle (start -1 2.5) (end 1 -2.5) (stroke (width 0.254)) (fill (type none))))
      (symbol \"R_1_1\"
        (pin passive line (at 0 3.81 270) (length 1.27) (name \"~\" (effects (font (size 1.27 1.27)))) (number \"1\" (effects (font (size 1.27 1.27)))))
        (pin passive line (at 0 -3.81 90) (length 1.27) (name \"~\" (effects (font (size 1.27 1.27)))) (number \"2\" (effects (font (size 1.27 1.27)))))))
    (symbol \"power:GND\" (power) (pin_names (offset 0))
      (property \"Reference\" \"#PWR\" (at 0 -6.35 0) (effects (font (size 1.27 1.27)) (hide yes)))
      (symbol \"GND_0_1\" (polyline (pts (xy 0 0) (xy 0 -1.27)) (stroke (width 0)) (fill (type none))))
      (symbol \"GND_1_1\" (pin power_in line (at 0 0 270) (length 0) (hide yes) (name \"GND\" (effects (font (size 1.27 1.27)))) (number \"1\" (effects (font (size 1.27 1.27))))))))
  (symbol (lib_id \"Device:R\") (at 100 100 0) (unit 1) (uuid \"r1\")
    (property \"Reference\" \"R1\" (at 102 100 0) (effects (font (size 1.27 1.27))))
    (property \"Value\" \"10k\" (at 104 100 0) (effects (font (size 1.27 1.27)))))
  (symbol (lib_id \"Device:R\") (at 120 100 0) (unit 1) (uuid \"r2\")
    (property \"Reference\" \"R2\" (at 122 100 0) (effects (font (size 1.27 1.27))))
    (property \"Value\" \"10k\" (at 124 100 0) (effects (font (size 1.27 1.27)))))
  (symbol (lib_id \"power:GND\") (at 100 110 0) (unit 1) (uuid \"p1\")
    (property \"Reference\" \"#PWR01\" (at 100 116 0) (effects (font (size 1.27 1.27)) (hide yes)))
    (property \"Value\" \"GND\" (at 100 114 0) (effects (font (size 1.27 1.27)))))
  (symbol (lib_id \"power:GND\") (at 120 115 0) (unit 1) (uuid \"p2\")
    (property \"Reference\" \"#PWR02\" (at 120 121 0) (effects (font (size 1.27 1.27)) (hide yes)))
    (property \"Value\" \"GND\" (at 120 119 0) (effects (font (size 1.27 1.27)))))
  (wire (pts (xy 100 96.19) (xy 100 90)) (stroke (width 0) (type default)) (uuid \"w1\"))
  (wire (pts (xy 100 90) (xy 120 90)) (stroke (width 0) (type default)) (uuid \"w2\"))
  (wire (pts (xy 120 90) (xy 120 96.19)) (stroke (width 0) (type default)) (uuid \"w3\"))
  (label \"VBUS\" (at 110 90 0) (effects (font (size 1.27 1.27)) (justify left bottom)) (uuid \"l1\"))
  (wire (pts (xy 90 93) (xy 130 93)) (stroke (width 0) (type default)) (uuid \"w6\"))
  (junction (at 120 93) (diameter 0) (color 0 0 0 0) (uuid \"j1\"))
  (wire (pts (xy 100 103.81) (xy 100 110)) (stroke (width 0) (type default)) (uuid \"w4\"))
  (wire (pts (xy 120 103.81) (xy 120 115)) (stroke (width 0) (type default)) (uuid \"w5\"))
  (wire (pts (xy 80 100) (xy 80 80)) (stroke (width 0) (type default)) (uuid \"w7\"))
  (wire (pts (xy 70 85) (xy 90 85)) (stroke (width 0) (type default)) (uuid \"w8\"))
  (wire (pts (xy 60 80) (xy 60 100)) (stroke (width 0) (type default)) (uuid \"w10\"))
  (wire (pts (xy 60 90) (xy 50 90)) (stroke (width 0) (type default)) (uuid \"w11\"))
)"

def items(m: Maybe<&2, List<&2, B.Item>>) -> List<&2, B.Item>:
  match m:
    case Some{xs}:
      xs
    case None{}:
      Nil{}

def hit(xs: List<&2, B.Item>, +k: String) -> Bool:
  match xs:
    case B.Item{key, _, _, _, _} <> _:
      String.eq(key, k)
    case Nil{}:
      False{}

def find.go(xs: List<&2, B.Item>, h: Bool, +k: String) -> Maybe<&2, B.Item>:
  match xs h:
    case i <> _ True{}:
      Some{i}
    case _ <> +t False{}:
      find.go(t, hit(t, k), k)
    case Nil{} _:
      None{}

def find(+xs: List<&2, B.Item>, +k: String) -> Maybe<&2, B.Item>:
  find.go(xs, hit(xs, k), k)

def info(m: Maybe<&2, B.Item>) -> String:
  match m:
    case Some{B.Item{_, _, _, _, i}}:
      i
    case None{}:
      "(missing)"

def g.of(ps: List<&2, String>) -> String:
  match ps:
    case Nil{}:
      "(none)"
    case +p <> t:
      Bool.pick(String, String.starts_with(p, "g="), String.drop(p, 2n), g.of(t))

# an item's group tag
def g(+xs: List<&2, B.Item>, +k: String) -> String:
  g.of(String.split(info(find(xs, k)), ';'))

def same(+a: String, +b: String) -> String:
  Bool.show(Bool.and(String.eq(a, b), Bool.not(String.eq(a, "(none)"))))

def main() -> IO(Unit):
  +xs = items(Sch.Sch.items(U.Utf8.encode(sheet())))
  do IO<Unit>:
    C.Check.str("a wire has its group", Bool.show(Bool.not(String.eq(g(xs, "w1"), "(none)"))), "True")
    C.Check.str("wires meeting at their ends", same(g(xs, "w1"), g(xs, "w2")), "True")
    C.Check.str("and the next", same(g(xs, "w2"), g(xs, "w3")), "True")
    C.Check.str("a pin at a wire's end", same(g(xs, "r1/pin/1"), g(xs, "w1")), "True")
    C.Check.str("so both pins 1", same(g(xs, "r1/pin/1"), g(xs, "r2/pin/1")), "True")
    C.Check.str("a label on a wire's middle", same(g(xs, "l1/0"), g(xs, "w2")), "True")
    C.Check.str("a wire through a junction", same(g(xs, "w6"), g(xs, "w3")), "True")
    C.Check.str("the junction too", same(g(xs, "j1"), g(xs, "w3")), "True")
    C.Check.str("a pin's own net is not the other's", same(g(xs, "r1/pin/1"), g(xs, "r1/pin/2")), "False")
    C.Check.str("a power symbol's pin at a wire's end", same(g(xs, "w4"), g(xs, "r1/pin/2")), "True")
    C.Check.str("two power symbols of one name", same(g(xs, "r1/pin/2"), g(xs, "r2/pin/2")), "True")
    C.Check.str("a power symbol says its group", same(g(xs, "p1/g1001"), g(xs, "w4")), "True")
    C.Check.str("wires crossing with no junction", same(g(xs, "w7"), g(xs, "w8")), "False")
    C.Check.str("a T: an end on a wire's middle", same(g(xs, "w10"), g(xs, "w11")), "True")
    C.Check.str("the info keeps its words", Bool.show(String.starts_with(info(find(xs, "r1/pin/1")), "pin 1;name=~;type=passive;symbol=R1;g=")), "True")
    return Unit{}
