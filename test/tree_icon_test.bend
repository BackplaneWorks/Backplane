import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/web/dom.bend as D
import ../src/web/icon.bend as WI
import ../src/web/view.bend as V
import ../src/web/presentation.bend as W

def show(n: D.Node) -> String:
  J.Json.show(W.Node.write(n))

# Independent pre-change button: every changing/accessibility field is still
# server supplied. Only the literal SVG child crosses the memo boundary.
def original(+id: String, +expanded: Bool, +count: Nat) -> D.Node:
  D.El{"button", "tree-toggle:" ++ id, [D.At{"class", "thread-expander"}, D.At{"type", "button"}, D.At{"data-count", Nat.show(count)},
    D.At{"aria-expanded", Bool.pick(String, expanded, "true", "false")}, D.At{"aria-label", Bool.pick(String, expanded, "Collapse subthreads", "Expand subthreads") ++ " (" ++ Nat.show(count) ++ ")"},
    D.At{"title", Bool.pick(String, expanded, "Collapse subthreads", "Expand subthreads") ++ " · " ++ Nat.show(count)}, D.At{"data-value", id}, D.On{"click", "thread-tree"}],
    [WI.WIcon.svg(Bool.pick(String, expanded, "chevron-down", "chevron-right"), 14)]}

def expanded(n: D.Node) -> String:
  J.Json.show(J.Json.arr(W.Node.write.go(V.View.expand([n]), Nil{})))

def expected(n: D.Node) -> String:
  J.Json.show(J.Json.arr(W.Node.write.go([n], Nil{})))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("open glyph expands to original exact path/size/attributes", show(V.View.make(WI.WIcon.tree(True{}))), show(WI.WIcon.svg("chevron-down", 14)))
    C.Check.str("closed glyph expands to original exact path/size/attributes", show(V.View.make(WI.WIcon.tree(False{}))), show(WI.WIcon.svg("chevron-right", 14)))
    C.Check.str("expanded parent preserves action target and accessible count", expanded(V.View.tree.expander("parent", True{}, 37n)), expected(original("parent", True{}, 37n)))
    C.Check.str("collapsed parent preserves action target and accessible count", expanded(V.View.tree.expander("parent", False{}, 37n)), expected(original("parent", False{}, 37n)))
    C.Check.str("leaf retains actual zero-child action and label", expanded(V.View.tree.expander("leaf", False{}, 0n)), expected(original("leaf", False{}, 0n)))
    C.Check.str("fresh expanded leaf remains independent of previous glyph", expanded(V.View.tree.expander("leaf", True{}, 0n)), expected(original("leaf", True{}, 0n)))
    C.Check.str("memo survives ordinary presentation transport", show(V.View.make(W.Node.read(W.Node.write(WI.WIcon.tree(True{}))))), show(WI.WIcon.svg("chevron-down", 14)))
    C.Check.str("ordinary row memo dispatch stays unchanged", show(V.View.make(V.View.row.m("row", "Fresh title", "now", "2", "busy", "row", True{}))), show(V.View.row.k("row", "Fresh title", "now", "2", "busy", "row", True{})))
