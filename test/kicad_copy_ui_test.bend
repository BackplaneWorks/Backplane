import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/kihealth.bend as KH
import ../src/core/hub.bend as H
import ../src/mobile/screen.bend as MOB
import ../src/web/view.bend as WEB
import ../src/web/dom.bend as D

def yes(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def ui() -> U.Ui:
  U.Ui.recv(U.Ui.init(1n), J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"ok", J.Flag{True{}}},
    J.KV{"result", J.Json.obj([J.KV{"kicadCopy", J.Flag{True{}}}, J.KV{"thread", J.Str{"t1"}}, J.KV{"main", J.Str{"/main/board.kicad_pro"}}, J.KV{"copy", J.Str{"/thread/board.kicad_pro"}}])}]))

def first(n: Maybe<&2, KH.KNote>) -> String:
  match n:
    case Some{KH.KNote{_, _, _, KH.KAct{l, _, _} <> _}}: l
    case _: ""

def sent(cs: List<&2, U.Cmd>) -> J.Json:
  match cs:
    case U.Send{line} <> _: Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{})
    case _ <> t: sent(t)
    case Nil{}: J.Null{}

def request(a: U.Act) -> J.Json:
  match a:
    case U.Act{_, cs}: sent(cs)

def choice(os: List<&2, H.Out>) -> String:
  match os:
    case H.Git{_, _, "kicad", _, _, a, _} <> _: a
    case _ <> t: choice(t)
    case Nil{}: ""

def routed(s: H.Step) -> String:
  match s:
    case H.Step{_, os}: choice(os)

def main() -> IO(Unit):
  do IO<Unit>:
    +u : U.Ui = ui()
    +nf : Map<&2, String> = U.Ui.infos(u)
    C.Check.str("choice belongs to requesting thread", yes(U.KO.pending(nf, "t1")), "yes")
    C.Check.str("other thread has no choice", yes(U.KO.pending(nf, "t2")), "no")
    C.Check.str("Open main is first and default", first(U.KN.thread(nf, U.Ui.state(u), "t1")), "Open main")
    C.Check.str("web renders the shared choice", Nat.show(List.length(&2, D.Node, WEB.View.kn(U.KN.thread(nf, U.Ui.state(u), "t1"), False{}))), "1")
    C.Check.str("phone includes thread-copy action", yes(String.contains(J.Json.show(MOB.Mob.kx.menu(U.KN.thread(nf, U.Ui.state(u), "t1"))), "kicad-copy-thread")), "yes")
    +a : J.Json = request(U.Ui.act(u, "kicad-copy-main", "t1"))
    C.Check.str("main action opens KiCad", J.Json.get_str(a, "m", ""), "kicad.open")
    C.Check.str("main action explicitly selects main", J.Json.get_str(J.Json.get_obj(a, "p"), "copy", ""), "main")
    C.Check.str("main action names original thread", J.Json.get_str(J.Json.get_obj(a, "p"), "thread", ""), "t1")
    C.Check.str("thread-copy action is explicit", J.Json.get_str(J.Json.get_obj(request(U.Ui.act(u, "kicad-copy-thread", "t1")), "p"), "copy", ""), "thread")
    C.Check.str("hub forwards the main choice to its job", routed(H.Hub.nav(3n, U.Ui.state(u), 1n, 0, "1", J.Json.get_obj(a, "p"), "kicad.open")), "main")
    C.Check.str("cancel opens nothing", J.Json.show(request(U.Ui.act(u, "kicad-copy-cancel", "t1"))), "null")
    C.Check.str("cancel clears choice", yes(U.KO.pending(U.Ui.infos(U.Act.ui(U.Ui.act(u, "kicad-copy-cancel", "t1"))), "t1")), "no")
    C.Check.str("main clears choice", yes(U.KO.pending(U.Ui.infos(U.Act.ui(U.Ui.act(u, "kicad-copy-main", "t1"))), "t1")), "no")
