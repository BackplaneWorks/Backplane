import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/client.bend as U
import ../src/web/view.bend as V
import ../src/web/presentation.bend as WP

# Agent setup's link buttons (view.bend's View.agy.btn.k): a link is drawn
# as the primary button only when its row says so (every link was blue, so
# with Claude Code found "Open installation guide" outshone the next
# step); the guide is the primary step only while the program is missing.

def html(+n: U.AgBtn) -> String:
  J.Json.show(WP.Node.write(V.View.agy.btn(U.Ui.init(1n), n)))

def btns(r: U.AgRow) -> List<&2, U.AgBtn>:
  match r:
    case U.AgRow{_, _, _, bs}: bs

def on(bs: List<&2, U.AgBtn>) -> String:
  match bs:
    case Nil{}: ""
    case U.AgBtn{_, _, _, o} <> t: Bool.show(o) ++ on(t)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("a plain link is not primary", Bool.show(String.contains(html(U.AgBtn{"Guide", "open-url", "https://x", False{}}), "btn primary")), "False")
    C.Check.str("a primary link is", Bool.show(String.contains(html(U.AgBtn{"Sign in", "open-url", "https://x", True{}}), "btn primary")), "True")
    C.Check.str("program not found: the guide is the step", on(btns(U.Psv.install(U.Ui.init(1n), "claude"))), "True")
