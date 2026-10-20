import Base
import ../menu_test.bend as MT
import ../check.bend as C
import ../../src/app/layout.bend as L
import ../../src/app/nui.bend as N
import ../../src/app/win.bend as W
import ../../src/app/forms.bend as F
import ../../src/app/theme.bend as T
import ../../src/core/client.bend as U
import ../../src/core/model.bend as M
import ../../src/core/edit.bend as ED
import ../../src/gfx/text.bend as X
import ../../src/gfx/raster.bend as R
import ../../src/gfx/draw.bend as D
import ../../src/gfx/tiles.bend as Q

# Native, real FreeType metrics and production layout/raster paths. No X11
# window, hub, device, timing result or installed application is involved.
def body() -> String:
  "Text size reflows the conversation and composer using the actual font advances. A long line should wrap before its panel edge; selecting text and clicking a model must still target what is drawn.\n\n" ++
  "| Setting | Behavior |\n|---|---|\n| Text size | Persistent across reconnects |\n| Resize | Rewraps at the new width |\n\n" ++
  "```\nabcdefghijklmnopqrstuv0123456789\n```\n\nThis sentence ends with a link to https://example.com/documentation/overflow/selection."

def token() -> String:
  "averylongunbrokentokenthatmustsplitacrossseverallineswithnothinglost01234567890123456789"

def draft() -> String:
  "First composer line.\nSecond line has a longer sentence for reflow and caret placement.\nThird line selected here."

def ui(+pct: U32) -> U.Ui:
  U.Ui.fold([M.MessagePosted{"t0", "u1", M.User{}, "Please make the text larger and keep the controls usable.", 1790000101n},
    M.MessagePosted{"t0", "a1", M.Assistant{}, body(), 1790000102n}, M.SettingSet{"ui.text_scale", U32.show(pct), 1790000103n}],
    MT.ui_sel(U.Ui.draft(MT.ui(), draft())))

def ops(o: L.Out) -> List<&2, R.Op>:
  match o:
    case L.Out{os, _}: os

def finish(r: File & Result<&1, &1, U32 & String, Unit>) -> IO(Unit):
  (f, result) = r
  do IO<Unit>:
    File.close(f)
    IO.pass(Unit, result)

def save(+path: String, +w: U32, +h: U32, os: List<&2, R.Op>) -> IO(Unit):
  do IO<Unit>:
    +img : Image = R.Frame.render(w, h, os, T.Th.bg())
    f : File <- IO.try(File, File.open(path, "w"))
    r : File & Result<&1, &1, U32 & String, Unit> <- File.write_bytes(f, D.Snap.ppm(img, w, h))
    finish(r)

# Check every wrapped line against its actual measured pixel budget.
def within(ls: List<&2, String>, +ty: L.Ty, +w: U32) -> Bool:
  match ls:
    case Nil{}: True{}
    case l <> rest:
      Bool.and(U32.is_le(L.Ty.width(ty, 3n, l), w), within(rest, ty, w))

def frame(+ty: L.Ty, +n: N.Nui) -> L.Out:
  L.Lay.frame(ty, L.Lay.memo(ty, n, L.TL.memo.none()), n)

# Exercise the real wheel route until the bounded fixture reaches its top.
def scroll_top(count: Nat, +ty: L.Ty, n: N.Nui) -> N.Nui:
  match count:
    case 0n: n
    case 1n+j: scroll_top(j, ty, MT.again(ty, MT.ev(n, W.WWheel{700, 200, 0})))

# Hit order follows paint priority, so the first Copy may be a later code
# block. Find the specific user bubble and require its row to be visible.
def visible_copy(hs: List<&2, N.Hit>, +text: String) -> Bool:
  match hs:
    case Nil{}: False{}
    case N.Hit{+x0, +y0, +x1, +y1, +a, +v} <> rest:
      Bool.or(Bool.and(Bool.and(String.eq(a, "copy"), String.eq(v, text)),
        Bool.and(U32.is_gt(x1, x0), Bool.and(U32.is_ge(y0, 52), U32.is_le(y1, 700)))), visible_copy(rest, text))

def checks(+ty: L.Ty) -> IO(Unit):
  do IO<Unit>:
    +tag : String = U32.show(L.Ty.pct(ty)) ++ ": "
    C.Check.str(tag ++ "uses FreeType", Bool.show(X.Face.ft(L.Ty.face(ty, 3n))), "True")
    +ls : List<&2, String> = L.Ty.wrap(ty, 3n, token(), 190)
    C.Check.str(tag ++ "unbroken token stays inside panel", Bool.show(within(ls, ty, 190)), "True")
    C.Check.str(tag ++ "unbroken token loses no characters", String.join(ls, ""), token())
    +wrapped : List<&2, String> = L.Comp.wrap(ty, draft(), 480)
    +starts : List<&2, Nat> = ED.Edit.starts(wrapped, draft())
    +row : String = Maybe.default(&2, String, List.get(&2, String, wrapped, 1n), "")
    +cx : U32 = L.Ty.width(ty, 3n, String.take(row, 5n))
    C.Check.str(tag ++ "click places caret on second wrapped row", Nat.show(L.Comp.under(N.Hit{100, 400, 580, 700, "focus", "composer"}, ty, wrapped, starts, 0n,
      (116 + cx : U32), (410 + L.Ty.px(ty, 23) + 1 : U32))), Nat.show(Nat.add(ED.Edit.start(starts, 1n), 5n)))
    +line : List<&2, L.TLine> = [L.TLine{"selected words", T.Th.text(), 0, "", 0}]
    C.Check.str(tag ++ "selection copies measured glyphs", String.join(L.TL.sel.texts(line, ty, L.TSel{True{}, 0n, 80, 0n, (80 + L.Ty.width(ty, 3n, "selected") : U32)}, 0n, 80, 600), "\n"), "selected")
    +n : N.Nui = MT.again(ty, N.Nui.init(ui(L.Ty.pct(ty)), 1200, 900))
    +memo : L.TLMemo = L.Lay.memo(ty, n, L.TL.memo.none())
    C.Check.str(tag ++ "memo equals fresh layout", Bool.show(Q.Eq.ops(ops(L.Lay.frame(ty, memo, n)), ops(frame(ty, n)))), "True")
    +menu : N.Nui = MT.again(ty, MT.act(n, "flag", "models"))
    +picked : N.Nui = MT.ev(menu, W.WButton{MT.at.x(N.Nui.hits(menu), "model"), MT.at.y(N.Nui.hits(menu), "model"), 1, True{}})
    C.Check.str(tag ++ "model hit selects and closes picker", Bool.show(U.Ui.flag(N.Nui.ui(picked), "models")), "False")
    save("build/text-scaling/native-" ++ U32.show(L.Ty.pct(ty)) ++ ".ppm", 1200, 900, ops(frame(ty, n)))
    save("build/text-scaling/menu-" ++ U32.show(L.Ty.pct(ty)) ++ ".ppm", 1200, 900, ops(frame(ty, menu)))
    +first : N.Nui = scroll_top(20n, ty, n)
    C.Check.str(tag ++ "wheel reaches timeline start", Maybe.default(&2, String, List.get(&2, String, String.split(MT.value(N.Nui.hits(first), "tl-at"), '|'), 1n), "missing"), "0")
    C.Check.str(tag ++ "first user message retains its visible copy target", Bool.show(visible_copy(N.Nui.hits(first), "Please make the text larger and keep the controls usable.")), "True")
    save("build/text-scaling/top-" ++ U32.show(L.Ty.pct(ty)) ++ ".ppm", 1200, 900, ops(frame(ty, first)))
    +small : N.Nui = MT.again(ty, MT.ev(n, W.WSize{800, 620}))
    C.Check.str(tag ++ "resize updates actual window width", U32.show(N.Nui.width(small)), "800")
    save("build/text-scaling/narrow-" ++ U32.show(L.Ty.pct(ty)) ++ ".ppm", 800, 620, ops(frame(ty, small)))

def main() -> IO(Unit):
  do IO<Unit>:
    +normal : L.Ty <- L.Ty.load()
    +large : L.Ty <- L.Ty.load.pct(150)
    C.Check.str("larger glyphs use larger actual advances", Bool.show(U32.is_gt(L.Ty.width(large, 3n, "Metric width"), L.Ty.width(normal, 3n, "Metric width"))), "True")
    C.Check.str("larger text reflows at unchanged width", Bool.show(Nat.is_gt(List.length(&2, String, L.Ty.wrap(large, 3n, body(), 400)), List.length(&2, String, L.Ty.wrap(normal, 3n, body(), 400)))), "True")
    checks(normal)
    checks(large)
