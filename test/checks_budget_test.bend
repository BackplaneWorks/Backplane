import Base
import ./check.bend as C
import ../src/core/checks.bend as K
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/i18n.bend as I
import ../src/web/view.bend as V
import ../src/web/dom.bend as D
import ../src/web/presentation.bend as P

def findings(+source: String) -> List<&2, K.Finding>:
  List.replicate(K.Finding, 1000n, K.Checks.finding(source, "error", "unconnected", String.repeat("部品|", 200n), "F.Cu"))

def report() -> String:
  K.Checks.md(List.concat(&2, K.Finding, [findings("erc"), findings("drc"), findings("bom")]), Nil{}, Nil{})

def main() -> IO(Unit):
  +md = report()
  +u = U.Ui.info_set(U.Ui.view_put(U.Ui.init(0n), "review", "checks"), "rp.md::checks", md)
  +node = P.Node.write(D.Dom.div("report", V.View.review(I.En{}, u)))
  do IO<Unit>:
    IO.print("Rendered report bytes: " ++ Nat.show(J.Json.bytes(node)))
    C.Check.str("large checks retain all source totals", Bool.show(Bool.and(String.contains(md, "ERC (1000)"), String.contains(md, "DRC (1000)"))), "True")
    C.Check.str("omissions are explicit", Bool.show(String.contains(md, "996 more not shown")), "True")
    C.Check.str("long descriptions are marked", Bool.show(String.contains(md, "…")), "True")
    C.Check.str("rendered checks leave room in the 256 KiB envelope", Bool.show(Nat.is_lt(J.Json.bytes(node), 131072n)), "True")
