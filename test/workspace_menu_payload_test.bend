import Base
import ./check.bend as C
import ../src/web/view.bend as W
import ../src/web/dom.bend as D
import ../src/web/presentation.bend as P
import ../src/core/json.bend as J
import ../src/core/i18n.bend as I

# Compare the complete menu contract, excluding decorative SVG descendants.
# Actions, values, labels, selection, groups and keyboard hints remain exact.
def without_icons.keep(icon: Bool, node: D.Node, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match icon:
    case True{}: acc
    case False{}: node <> acc

def without_icons(xs: List<&2, D.Node>, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match xs:
    case Nil{}: List.reverse(&2, D.Node, acc)
    case D.Txt{text} <> rest: without_icons(rest, D.Txt{text} <> acc)
    case D.El{+tag, key, attrs, kids} <> rest:
      without_icons(rest, without_icons.keep(String.eq(tag, "svg"),
        D.El{tag, key, attrs, without_icons(kids, Nil{})}, acc))

def svg_count(xs: List<&2, D.Node>) -> Nat:
  match xs:
    case Nil{}: 0n
    case D.Txt{_} <> rest: svg_count(rest)
    case D.El{tag, _, _, kids} <> rest:
      Nat.add(Bool.pick(Nat, String.eq(tag, "svg"), 1n, 0n), Nat.add(svg_count(kids), svg_count(rest)))

def menu_body(menu: D.Node) -> List<&2, D.Node>:
  match menu:
    case D.El{_, _, _, _ <> D.El{_, _, _, kids} <> _}: kids
    case _: Nil{}

def shown(xs: List<&2, D.Node>) -> String:
  J.Json.show(P.Node.write(D.El{"div", "", Nil{}, xs}))

def check(+cur: String, +term: Bool, +palette: Bool) -> IO(Unit):
  do IO<Unit>:
    +menu : D.Node = W.View.workspace.menu(I.En{}, cur, term, palette)
    C.Check.str("menu payload leaves room for actual thread content: " ++ cur,
      Bool.show(Nat.is_lt(J.Json.bytes(P.Node.write(menu)), 12000n)), "True")
    C.Check.str("menu options do not repeat decorative SVG trees: " ++ cur,
      Nat.show(svg_count(menu_body(menu))), "0")
    C.Check.str("every menu action label state and shortcut is preserved: " ++ cur,
      shown(without_icons(menu_body(menu), Nil{})),
      shown(without_icons(W.View.workspace.choices(False{}, I.En{}, cur, term, palette), Nil{})))
    C.Check.str("visible rail keeps all eleven tool icons: " ++ cur,
      Nat.show(svg_count(W.View.workspace.choices(True{}, I.En{}, cur, term, palette))), "11")
    C.Check.str("menu opener keeps the current icon and disclosure arrow: " ++ cur,
      Nat.show(svg_count([menu])), "2")

def main() -> IO(Unit):
  do IO<Unit>:
    check("", False{}, False{})
    check("3d", False{}, False{})
    check("board", False{}, True{})
    check("schematic", True{}, False{})
    check("files", False{}, False{})
    check("browser", False{}, True{})
    check("terminal", True{}, False{})
    check("commands", False{}, True{})
