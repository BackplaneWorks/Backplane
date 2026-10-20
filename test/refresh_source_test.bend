import Base
import ./check.bend as C
import ../src/core/board.bend as B
import ../src/core/client.bend as U
import ../src/app/nui.bend as N
import ../src/app/viewer.bend as V

def item(+x: U32) -> B.Item:
  B.Item{"wire", x, 14, B.PSeg{B.Pt{0.0, 0.0}, B.Pt{U32.to_f32(x), 1.0}, 0.4}, "wire"}

def endpoint(v: V.Vw) -> String:
  match v:
    case V.Vw{V.Seen{B.Item{_, _, _, B.PSeg{_, B.Pt{x, _}, _}, _}, _} <> _, _, _, _, _, _, _, _, _, _, _}:
      U32.show(F32.to_u32(x))
    case _: "empty"

def at(+path: String, +x: U32) -> N.Nui:
  +key = V.Src.key(1, "/p", path)
  +v = N.Vw.board.k(False{}, N.Vw.kind_set.k(True{}, V.Vw.init(), 1, path), key, [item(x)], 10n)
  N.Nui.asked(N.Nui.with_vw(N.Nui.init(U.Ui.init(0n), 800, 600), v), key)

def main() -> IO(Unit):
  +b = at("/p/b.kicad_sch", 20)
  +latest = N.Nui.board(b, V.Src.key(1, "/p", "/p/b.kicad_sch"), [item(25)], 12n)
  +a = at("/p/a.kicad_sch", 30)
  do IO<Unit>:
    C.Check.str("old source result after switch is ignored", endpoint(N.Nui.vw(N.Nui.board(b, V.Src.key(1, "/p", "/p/a.kicad_sch"), [item(10)], 11n))), "20")
    C.Check.str("current source gets newest geometry", endpoint(N.Nui.vw(latest)), "25")
    C.Check.str("old source cannot replace final revision", endpoint(N.Nui.vw(N.Nui.board(latest, V.Src.key(1, "/p", "/p/a.kicad_sch"), [item(10)], 13n))), "25")
    C.Check.str("switch back reads saved source", endpoint(N.Nui.vw(a)), "30")
