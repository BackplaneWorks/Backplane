import Base
import ./check.bend as C
import ./menu_test.bend as MT
import ../src/app/nui.bend as N
import ../src/gfx/tiles.bend as TI
import ../src/gfx/raster.bend as R
import ../src/app/theme.bend as T
import ../src/core/textscale.bend as TS
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/app/layout.bend as L
import ../src/gfx/text.bend as X

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}: u

def ops(o: L.Out) -> List<&2, R.Op>:
  match o:
    case L.Out{os, _}: os

def Cache.check(+ty: L.Ty) -> IO(Unit):
  do IO<Unit>:
    +n : N.Nui = N.Nui.init(U.Ui.fold([M.MessagePosted{"t0", "m1", M.Assistant{}, "Fresh content wraps with the selected font.", 1790000102n}], MT.ui()), 1200, 900)
    +entry : M.Entry = M.Msg{"m1", "t0", M.Assistant{}, "Fresh content wraps with the selected font.", 1790000102n}
    +stale : L.TLMemo = L.TLMemo{L.Lay.tl.px(n), "t0", [L.TLE{entry, L.TLPart{[L.TLine{"STALE MEASURED CONTENT", T.Th.text(), 0, "", 0}], Nil{}, Nil{}}}], L.LiveLocal{"en", L.Lay.tl.px(n), "", False{}, Nil{}}}
    C.Check.str("same-font control keeps deliberately old measured entry", Bool.show(TI.Eq.ops(ops(L.Lay.frame(L.Ty.none(), L.Lay.memo(L.Ty.none(), n, stale), n)), ops(L.Lay.frame(L.Ty.none(), L.Lay.memo(L.Ty.none(), n, L.TL.memo.none()), n)))), "False")
    C.Check.str("font change discards same-width cached entry", Bool.show(TI.Eq.ops(ops(L.Lay.frame(ty, L.Lay.memo(ty, n, stale), n)), ops(L.Lay.frame(ty, L.Lay.memo(ty, n, L.TL.memo.none()), n)))), "True")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("missing size retains default", U32.show(TS.Scale.read("")), "100")
    C.Check.str("invalid size cannot escape bounds", U32.show(TS.Scale.read("99999999999")), "100")
    C.Check.str("malformed size retains default", U32.show(TS.Scale.read("150x")), "100")
    C.Check.str("smaller size", U32.show(TS.Scale.read("85")), "85")
    C.Check.str("largest size", U32.show(TS.Scale.read("150")), "150")
    +u : U.Ui = ui(U.Ui.setting(U.Ui.init(0n), "ui.text_scale=150"))
    C.Check.str("setting applies before server echo", U32.show(TS.Scale.of(U.Ui.state(u))), "150")
    +replayed : U.Ui = U.Ui.fold([M.SettingSet{"ui.text_scale", "130", 1n}], U.Ui.init(2n))
    C.Check.str("setting survives replay", U32.show(TS.Scale.of(U.Ui.state(replayed))), "130")
    C.Check.str("native uses physical font pixels once", U32.show(TS.Scale.px(150, 14)), "21")
    C.Check.str("entry boundary never gains height", U32.show(L.TL.h.s(29, 150)), "0")
    C.Check.str("image rows retain image geometry", U32.show(L.TL.h.s(12, 150)), "20")
    C.Check.str("body row grows with text", U32.show(L.TL.h.s(0, 150)), "35")
    +ty : L.Ty = L.TyScaled{150, L.Ty.none(), X.Text.bitmap()}
    C.Check.str("minimum composer rows grow too", U32.show(L.Comp.eh.s(1n, ty)), "24")
    C.Check.str("default composer remains unchanged", U32.show(L.Comp.eh(1n)), "0")
    C.Check.str("default typography key remains stable", L.Ty.tag(L.Ty.none(), "en"), "en")
    C.Check.str("font size participates in layout key", L.Ty.tag(ty, "en"), "en/text=150")
    C.Check.str("tall metrics cannot underflow baseline", U32.show(X.Text.baseline(X.Face{0, False{}, 21, 20, 5, 25, 672, "tall", X.GTab.none(7n)}, 10, 20)), "30")
    Cache.check(ty)
