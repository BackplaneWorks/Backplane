import Base
import ./check.bend as C
import ../src/core/board.bend as B
import ../src/core/pcb2d.bend as P
import ../src/core/sexpr.bend as S
import ../src/core/utf8.bend as U

# A board read as latin1 text forms (the --plot child's path for live
# boards, server/effects/plot_board.js) gives the items of the same board
# read as one byte list: same order, keys, signatures, layers and info.

def latin1.go(bs: List<&2, U32>) -> List<&2, Char>:
  match bs:
    case Nil{}:
      Nil{}
    case b <> t:
      Char.from_u32(b) <> latin1.go(t)

def latin1(bs: List<&2, U32>) -> String:
  String.from_list(latin1.go(bs))

def texts(fs: List<&2, List<&2, U32>>) -> List<&2, String>:
  match fs:
    case Nil{}:
      Nil{}
    case f <> t:
      latin1(f) <> texts(t)

def show(xs: List<&2, B.Item>) -> String:
  match xs:
    case Nil{}:
      ""
    case B.Item{k, s, l, _, i} <> t:
      k ++ "|" ++ U32.show(s) ++ "|" ++ U32.show(l) ++ "|" ++ i ++ "\n" ++ show(t)

def bytes.items(m: Maybe<&2, List<&2, B.Item>>) -> String:
  match m:
    case Some{xs}:
      show(xs)
    case None{}:
      "none"

def text.items(m: Maybe<&2, List<&2, List<&2, U32>>>) -> String:
  match m:
    case Some{fs}:
      show(P.Pcb2.items.text(texts(fs)))
    case None{}:
      "none"

def board() -> String:
  "(kicad_pcb (version 20240108) (general (thickness 1.2))\n" ++
  "  (gr_line (start 0 0) (end 10 0) (layer \"Edge.Cuts\") (width 0.1))\n" ++
  "  (gr_text \"Ω (µ) \\\"q\\\"\" (at 10 20 0) (layer \"F.SilkS\") (uuid label) (effects (font (size 1 1) (thickness 0.125))))\n" ++
  "  (segment (start 1 1) (end 5 5) (width 0.2) (layer \"F.Cu\") (net 1))\n" ++
  "  (via (at 3 3) (size 0.6) (drill 0.3) (layers \"F.Cu\" \"B.Cu\") (net 1))\n" ++
  "  (footprint \"R_0603\" (layer \"F.Cu\") (at 20 20 90) (property \"Reference\" \"R1\" (at 0 -1 90) (layer \"F.SilkS\") (effects (font (size 1 1))))\n" ++
  "    (fp_line (start -1 -1) (end 1 1) (layer \"F.SilkS\") (width 0.12))\n" ++
  "    (pad \"1\" smd rect (at -0.8 0 90) (size 0.8 0.9) (layers \"F.Cu\" \"F.Mask\")))\n" ++
  "  (zone (net 1) (layer \"B.Cu\") (filled_polygon (layer \"B.Cu\") (pts (xy 0 0) (xy 4 0) (xy 4 4)))))\n"

def main() -> IO(Unit):
  +bs = U.Utf8.encode(board())
  +want = bytes.items(P.Pcb2.items(bs))
  do IO<Unit>:
    C.Check.str("text forms give the byte path's items", text.items(S.Sx.forms(bs)), want)
    C.Check.str("the board has items", Bool.show(String.is_empty(want)), "False")
    C.Check.str("latin1 text gives back its bytes", Bool.show(String.eq(latin1(P.Pcb2.bytes(latin1(bs))), latin1(bs))), "True")
