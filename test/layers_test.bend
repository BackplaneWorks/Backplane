import Base
import ./check.bend as C
import ../src/core/layers.bend as L

# The layer list and the focus (core/layers.bend): KiCad's order and
# names, the active layer with the rest dimmed or hidden, the masks the
# web and phones draw from, and the window's kind word.

def ns(ls: List<&2, U32>) -> String:
  match ls:
    case Nil{}:
      ""
    case +l <> +t:
      L.Lyr.name(0, l) ++ Bool.pick(String, List.is_empty(&2, U32, t), "", " ") ++ ns(t)

def lk(l: U32 & U32) -> String:
  (c, a) = l
  U32.show(c) ++ "/" ++ U32.show(a)

def fs(+f: L.LayerFocus) -> String:
  L.Lyr.show(f)

def ms(+m: L.Masks) -> String:
  U32.show(L.Masks.off(m)) ++ "/" ++ U32.show(L.Masks.dim(m))

def bool(b: Bool) -> String:
  Bool.pick(String, b, "true", "false")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("a board's layers in KiCad's order", ns(L.Lyr.order(0)),
      "F.Cu In1.Cu In2.Cu In3.Cu In4.Cu In5.Cu In6.Cu B.Cu Pads Vias Holes F.Paste B.Paste F.Silkscreen B.Silkscreen F.Mask B.Mask User.Drawings User.Comments Edge.Cuts Margin F.Courtyard B.Courtyard F.Fab B.Fab")
    C.Check.str("a schematic keeps its order", U32.show(Maybe.default(&2, U32, List.head(&2, U32, L.Lyr.order(1)), 99)), "0")
    C.Check.str("a comparison lists its marks last", L.Lyr.name(8, 28), "Added")
    C.Check.str("no focus: as it is", lk(L.Lyr.shade(0, L.Lyr.plain(), 16, (7, 200))), "7/200")
    C.Check.str("dimmed: another layer at a quarter", lk(L.Lyr.shade(0, L.LayerFocus{12, 1}, 16, (7, 200))), "7/50")
    C.Check.str("only: another layer hidden", lk(L.Lyr.shade(0, L.LayerFocus{12, 2}, 16, (7, 200))), "7/0")
    C.Check.str("only: the active layer shows", lk(L.Lyr.shade(0, L.LayerFocus{12, 2}, 12, (7, 210))), "7/210")
    C.Check.str("a layer hidden by default shows when active", lk(L.Lyr.shade(0, L.LayerFocus{15, 2}, 15, (7, 0))), "7/200")
    C.Check.str("copper keeps its vias", lk(L.Lyr.shade(0, L.LayerFocus{5, 2}, 23, (7, 255))), "7/255")
    C.Check.str("and its pads", lk(L.Lyr.shade(0, L.LayerFocus{5, 2}, 13, (7, 220))), "7/220")
    C.Check.str("silkscreen does not", lk(L.Lyr.shade(0, L.LayerFocus{16, 2}, 23, (7, 255))), "7/0")
    C.Check.str("the outline stays", lk(L.Lyr.shade(0, L.LayerFocus{16, 2}, 22, (7, 255))), "7/255")
    C.Check.str("masks: none", ms(L.Lyr.masks(0, 5, L.Lyr.plain())), "5/0")
    C.Check.str("masks: only F.Cu hides the rest but its own", ms(L.Lyr.masks(0, 0, L.LayerFocus{12, 2})), "37736447/0")
    C.Check.str("masks: dim F.Cu dims the rest", ms(L.Lyr.masks(0, 0, L.LayerFocus{12, 1})), "0/37736447")
    C.Check.str("masks: a hidden layer stays hidden, not dimmed", ms(L.Lyr.masks(0, 1, L.LayerFocus{12, 1})), "1/37736446")
    C.Check.str("masks: the active layer is never hidden", ms(L.Lyr.masks(0, 4096, L.LayerFocus{12, 2})), "37736447/0")
    C.Check.str("a click on a layer focuses it, the rest dim", fs(L.Lyr.pick(L.Lyr.plain(), 16)), "16|1")
    C.Check.str("a click on another moves the focus", fs(L.Lyr.pick(L.LayerFocus{16, 2}, 3)), "3|2")
    C.Check.str("a click on it again puts all back", fs(L.Lyr.pick(L.LayerFocus{16, 1}, 16)), "16|0")
    C.Check.str("ctrl+H from nothing dims around front copper", fs(L.Lyr.cycle(0, L.Lyr.plain())), "12|1")
    C.Check.str("then hides", fs(L.Lyr.cycle(0, L.LayerFocus{12, 1})), "12|2")
    C.Check.str("then back", fs(L.Lyr.cycle(0, L.LayerFocus{12, 2})), "12|0")
    C.Check.str("lifted: the others first", bool(L.Lyr.lift.ok(L.Lyr.lift.under(L.LayerFocus{5, 1}), 12)) ++ bool(L.Lyr.lift.ok(L.Lyr.lift.under(L.LayerFocus{5, 1}), 5)), "truefalse")
    C.Check.str("then the active one alone", bool(L.Lyr.lift.ok(L.Lyr.lift.over(L.LayerFocus{5, 1}), 5)) ++ bool(L.Lyr.lift.ok(L.Lyr.lift.over(L.LayerFocus{5, 1}), 12)), "truefalse")
    C.Check.str("nothing lifted: all", bool(L.Lyr.lift.ok(L.Lyr.lift.none(), 5)), "true")
    C.Check.str("the web and phones draw the active one last", U32.show(L.Lyr.top(L.LayerFocus{5, 1})) ++ " " ++ U32.show(L.Lyr.top(L.LayerFocus{5, 0})), "5 255")
    C.Check.str("the text round trip", fs(L.Lyr.read(L.Lyr.show(L.LayerFocus{16, 2}))), "16|2")
    C.Check.str("nothing read is no focus", fs(L.Lyr.read("")), "255|0")
    C.Check.str("the kind word keeps the kind", U32.show(L.Lyr.kind.base(L.Lyr.kind.with(9, L.LayerFocus{16, 2}))), "9")
    C.Check.str("and the focus", fs(L.Lyr.kind.focus(L.Lyr.kind.with(9, L.LayerFocus{16, 2}))), "16|2")
