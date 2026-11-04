import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/web/view.bend as V
import ../src/web/presentation.bend as WP
import ../src/mobile/screen.bend as MB
import ../src/app/layout.bend as L
import ../src/app/nui.bend as N

def progress(+phase: String) -> J.Json:
  J.Json.obj([J.KV{"attempt", J.Str{"fixture"}}, J.KV{"phase", J.Str{phase}}, J.KV{"done", J.Num{"412000000"}}, J.KV{"total", J.Num{"980000000"}}, J.KV{"speed", J.Num{"1000000"}}, J.KV{"known", J.Flag{True{}}}, J.KV{"eta", J.Num{"568"}}])

def ui(phase: String) -> U.Ui:
  U.Ui.info_set(U.Ui.init(1n), "cad.install", J.Json.show(J.Json.obj([J.KV{"kicad", progress(phase)}, J.KV{"freecad", progress("download")}])) )

def web(phase: String) -> String:
  J.Json.show(WP.Node.write(V.View.cad.row(ui(phase), "kicad")))

def hits(o: L.Out) -> List<&2, N.Hit>:
  match o:
    case L.Out{_, hs}: hs

def has(hs: List<&2, N.Hit>, +act: String) -> Bool:
  match hs:
    case Nil{}: False{}
    case N.Hit{_, _, _, _, a, _} <> t: Bool.or(String.eq(a, act), has(t, act))

def native(+u: U.Ui) -> L.Out:
  L.Set.cad.row(L.Ty.none(), N.Nui.init(u, 800, 900), "kicad", 0, 500, 0)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("web has real byte label", Bool.show(String.contains(web("download"), "Downloading 412 / 980 MB")), "True")
    C.Check.str("web has progress element", Bool.show(String.contains(web("download"), "progress")), "True")
    C.Check.str("web offers Cancel", Bool.show(String.contains(web("extract"), "cad-cancel")), "True")
    C.Check.str("web offers terminal Dismiss", Bool.show(String.contains(web("failed"), "cad-dismiss")), "True")
    C.Check.str("active job has no terminal Dismiss", Bool.show(String.contains(web("download"), "cad-dismiss")), "False")
    C.Check.str("dismissed result falls back to installed-tool state", J.Json.get_str(U.Cadview.at(U.Ui.infos(ui("dismissed")), "kicad"), "attempt", ""), "")
    C.Check.str("web offers Retry on error", Bool.show(String.contains(web("failed"), "Retry")), "True")
    C.Check.str("mobile gets bytes and meter", Bool.show(String.contains(J.Json.show(MB.Mob.cad.row(ui("download"), "kicad")), "installProgress")), "True")
    C.Check.str("mobile gets known total", Nat.show(J.Json.get_nat(J.Json.get_obj(MB.Mob.cad.row(ui("download"), "kicad"), "installProgress"), "fraction", 0n)), "420")
    C.Check.str("mobile gets Cancel", Bool.show(String.contains(J.Json.show(MB.Mob.cad.row(ui("download"), "kicad")), "cad-cancel")), "True")
    C.Check.str("native Cancel hit is reachable", Bool.show(has(hits(native(ui("extract"))), "cad-cancel")), "True")
    C.Check.str("native terminal Dismiss hit is reachable", Bool.show(has(hits(native(ui("failed"))), "cad-dismiss")), "True")
    C.Check.str("native active job has no Dismiss hit", Bool.show(has(hits(native(ui("extract"))), "cad-dismiss")), "False")
    C.Check.str("native Retry hit is reachable", Bool.show(has(hits(native(ui("failed"))), "cad-install")), "True")
    C.Check.str("switching views retains current snapshot", U.Ui.info(U.Ui.reselect("another-thread", ui("download")), "cad.install"), U.Ui.info(ui("download"), "cad.install"))
