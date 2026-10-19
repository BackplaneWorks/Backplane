import Base
import ./check.bend as C
import ../src/core/board.bend as B
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/hist.bend as H
import ../src/app/nui.bend as N
import ../src/app/viewer.bend as V

def item() -> B.Item:
  B.Item{"part", 1, 12, B.PSeg{B.Pt{0.0, 0.0}, B.Pt{1.0, 1.0}, 0.2}, "part"}

def loaded(+kind: U32, +path: String) -> V.Vw:
  N.Vw.board.k(False{}, N.Vw.kind_set.k(True{}, V.Vw.init(), kind, path), V.Src.key(kind, "/p", path), [item()], 10n)

def count(v: V.Vw) -> String:
  match v:
    case V.Vw{s, _, _, _, _, _, _, _, _, _, _}:
      Nat.show(List.length(&2, V.Seen, s))

def json(s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def same_request() -> N.Nui:
  +event = json("{\"t\":\"view\",\"thread\":\"a\",\"path\":\"/p/a.kicad_pcb\",\"kind\":\"board\"}")
  +u = U.Ui.recv(U.Act.ui(U.Ui.act(U.Ui.init(0n), "select", "a")), event)
  +n = N.Nui.with_vw(N.Nui.init(u, 800, 600), loaded(0, "/p/a.kicad_pcb"))
  N.Nui.request(N.Nui.with_ui(n, U.Ui.recv(N.Nui.ui(n), event)))

def main() -> IO(Unit):
  +board = loaded(0, "/p/a.kicad_pcb")
  +sch = loaded(1, "/p/a.kicad_sch")
  +other = N.Vw.kind_set(board, 0, "/p/b.kicad_pcb")
  do IO<Unit>:
    C.Check.str("repeat board keeps loaded scene", count(N.Vw.kind_set(board, 0, "/p/a.kicad_pcb")), "1")
    C.Check.str("repeat schematic keeps loaded scene", count(N.Vw.kind_set(sch, 1, "/p/a.kicad_sch")), "1")
    C.Check.str("repeat canonical path keeps loaded scene", count(N.Vw.kind_set(loaded(0, ""), 0, "")), "1")
    C.Check.str("repeated viewer_show keeps native scene", count(N.Nui.vw(same_request())), "1")
    C.Check.str("another file starts empty", count(other), "0")
    C.Check.str("another file must be read", Bool.show(String.eq(N.Vx.shown(N.Vw.x(other)), V.Src.key(0, "/p", "/p/b.kicad_pcb"))), "False")
    C.Check.str("another kind starts empty", count(N.Vw.kind_set(board, 1, "/p/a.kicad_sch")), "0")
    C.Check.str("history family retains scene", count(N.Vw.kind_set(board, 0, H.Hist.git("/p/.git", "c1", "/p", "a.kicad_pcb"))), "1")
