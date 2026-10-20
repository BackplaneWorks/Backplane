import Base
import ../../src/core/textscale.bend as TS
import ../../src/gfx/text.bend as X
import ../../src/core/client.bend as U
import ../../src/core/i18n.bend as I
import ../../src/app/nui.bend as N
import ../../src/app/layout.bend as L
import ../../src/gfx/raster.bend as R
import ../../src/gfx/draw.bend as D
import ../../src/server/argv.bend as AV

# Render the actual settings layout with real system fonts, without a hub or window.
# settings_frame PAGE WIDTH HEIGHT LOCALE SCROLL OUT.ppm FONT_PERCENT

def state(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}: u

def ops(o: L.Out) -> List<&2, R.Op>:
  match o:
    case L.Out{os, _}: List.reverse(&2, R.Op, os)

def arg(+xs: List<&2, String>, +i: Nat) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, xs, i)), "")

def copy(xs: List<&1, String>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case x <> t: x <> copy(t)

def make(+as: List<&2, String>, +ty: L.Ty) -> L.Out:
  +w = Maybe.default(&2, U32, U32.read(arg(as, 1n)), 1200)
  +h = Maybe.default(&2, U32, U32.read(arg(as, 2n)), 800)
  +u = state(U.Ui.act(state(U.Ui.act(U.Ui.init(0n), "flag", "settings")), "settings-page", arg(as, 0n)))
  L.Lay.settings(ty, U.Ui.state.of(u), N.Nui.init(u, w, h), Map.new(&2, String), "review", "", 264, w, h, Maybe.default(&2, U32, U32.read(arg(as, 4n)), 0))

# A real-font stress fixture at the agreed scale percentages. This changes only
# fixture fonts; it does not implement the application's metric/cache seam.
def load(+pct: U32) -> IO(L.Ty):
  do IO<L.Ty>:
    +ui : X.Face <- X.Text.face(L.Ty.fonts.ui(), TS.Scale.px(pct, 14))
    +bold : X.Face <- X.Text.face(L.Ty.fonts.bold(), TS.Scale.px(pct, 14))
    +meta : X.Face <- X.Text.face(L.Ty.fonts.ui(), TS.Scale.px(pct, 12))
    +code : X.Face <- X.Text.face(L.Ty.fonts.code(), TS.Scale.px(pct, 13))
    +cache : Map<&2, X.TG> <- L.Ty.fonts.warm([ui,bold,meta,code], Map.new(&2, X.TG))
    return L.Ty{cache, X.Face.tabled(ui, cache), X.Face.tabled(bold, cache), X.Face.tabled(meta, cache), X.Face.tabled(ui, cache), X.Face.tabled(code, cache), I.En{}}

def go(+as: List<&2, String>) -> IO(Unit):
  do IO<Unit>:
    +ty0 : L.Ty <- load(TS.Scale.read(arg(as, 6n)))
    +ty : L.Ty = L.Ty.with_loc(ty0, Bool.pick(I.Loc, String.eq(arg(as, 3n), "ja"), I.Ja{}, I.En{}))
    # Non-ASCII glyphs requested by the actual layout are fetched before drawing.
    +frame : L.Out = make(as, ty)
    +ready : L.Ty <- L.Ty.fill(ty, X.Text.wants(ops(frame), Nil{}))
    +w : U32 = Maybe.default(&2, U32, U32.read(arg(as, 1n)), 1200)
    +h : U32 = Maybe.default(&2, U32, U32.read(arg(as, 2n)), 800)
    +img : Image = R.Frame.render(w, h, L.Theme.ops(False{}, ops(make(as, ready))), 1382425)
    f : File <- IO.try(File, File.open(arg(as, 5n), "w"))
    r : File & Result<&1, &1, U32 & String, Unit> <- File.write_bytes(f, D.Snap.ppm(img, w, h))
    IO.print("wrote " ++ arg(as, 5n))

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<&1, String> <- AV.Argv.args()
    go(copy(args))
