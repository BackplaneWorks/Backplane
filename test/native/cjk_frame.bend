import Base
import ../../src/gfx/draw.bend as D
import ../../src/gfx/raster.bend as R
import ../../src/gfx/text.bend as X
import ../../src/gfx/fontfx.bend as FX
import ../../src/core/cjk.bend as C
import ../../src/app/theme.bend as T

# Headless Japanese fixture at the window's actual 12/13/14 px sizes.
# Both palettes, 360 px columns, baseline marks and exact cell rulers.
# Build with scripts/build-js.sh; writes /tmp/cjk.ppm.
def paragraph() -> String:
  "日本語の表示を確認します。「新しいスレッド」を開いて、プロジェクトの設定を変更しました。テストは通過しています。句読点や小さい文字、長音符ーが行頭に残らず、読みやすく折り返されます。"

def mixed() -> String:
  "PRを開いた。テストは通過。Backplaneの設定で日本語を選びます。"

def grid() -> List<&2, String>:
  ["012345678901234567890123456789012345678901234",
   "|AB|日本|CD|直骨|EF|写画|GH|かな|IJ|",
   "|12|日本|34|直骨|56|写画|78|かな|90|",
   "新しいスレッド  ボット  プロジェクト  設定",
   "直骨写画  「日本語。」 （かな） αβγΩλπ→✓"]

def all() -> String:
  paragraph() ++ mixed() ++ String.join(grid(), "") ++ "直骨写画日本語あいうえお「」。（）ー"

def lines(ls: List<&2, String>, +cache: Map<&2, X.TG>, +f: X.Face, +c: D.Clip, +x: U32, +y: U32, +lh: U32, +color: U32) -> List<&2, R.Op>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      List.append(&2, R.Op, X.Text.line(cache, f, l, c, x, y, lh, color), lines(t, cache, f, c, x, (y + lh : U32), lh, color))

def rulers(n: Nat, +c: D.Clip, +x: U32, +y: U32, +cell: U32, +color: U32) -> List<&2, R.Op>:
  match n:
    case 0n:
      Nil{}
    case 1n+k:
      D.Draw.rect(c, x, y, 1, 63, color, 100) <> rulers(k, c, (x + cell : U32), y, cell, color)

def band(+cache: Map<&2, X.TG>, +f: X.Face, +light: Bool, +x: U32, +y: U32, +px: U32) -> List<&2, R.Op>:
  +c = {D.Clip{x, y, (x + 460 : U32), (y + 320 : U32)} : D.Clip}
  +fg = T.Th.of(light, T.Th.text())
  +dim = T.Th.of(light, T.Th.faint())
  +lh = Bool.pick(U32, U32.is_eq(px, 12), 16, Bool.pick(U32, U32.is_eq(px, 13), 18, 23))
  +cell = X.Text.width(cache, f, "0")
  List.concat(&2, R.Op, [
    X.Text.line(cache, f, U32.show(px) ++ " px / " ++ U32.show(cell) ++ " px cell", c, x, y, lh, dim),
    lines(X.Text.wrap(cache, f, paragraph(), 360), cache, f, c, x, (y + 27 : U32), lh, fg),
    lines(X.Text.wrap(cache, f, mixed(), 360), cache, f, c, x, (y + 150 : U32), lh, fg),
    lines(grid(), cache, f, c, x, (y + 202 : U32), lh, fg),
    rulers(43n, c, x, (y + 201 : U32), cell, dim),
    [D.Draw.rect(c, (x + 360 : U32), (y + 25 : U32), 1, 170, dim, 100)]])

def report.glyph(+cp: U32, g: X.TG, +asc: U32, +desc: U32, +cell: U32) -> IO(Unit):
  match g:
    case X.TG{+adv, +left, +top, +w, +h, _, _}:
      do IO<Unit>:
        IO.print(SCon{Chr{cp}, ""} ++ " adv26.6=" ++ U32.show(adv) ++ " ink left/top(+32768)=" ++ U32.show(left) ++ "/" ++ U32.show(top) ++ " bitmap=" ++ U32.show(w) ++ "x" ++ U32.show(h))
        IO.print(Bool.pick(String, Bool.and(U32.is_eq(adv, (cell * Bool.pick(U32, C.Cjk.wide(cp), 2, 1) : U32)), Bool.and(U32.is_le(top, (32768 + asc : U32)), U32.is_le((32768 + h : U32), (top + desc : U32)))), "ok glyph cell and ink", "FAIL glyph cell or ink"))

def report.glyphs(s: String, +cache: Map<&2, X.TG>, +f: X.Face, +asc: U32, +desc: U32, +cell: U32) -> IO(Unit):
  match s:
    case SNil{}:
      IO.pure(Unit, Unit{})
    case SCon{Chr{+cp}, t}:
      IO.bind(Unit, Unit, report.glyph(cp, X.Text.get(cache, f, cp), asc, desc, cell), u => report.glyphs(t, cache, f, asc, desc, cell))

# The raw effect metrics confirm exact primary 0 advance (in 26.6),
# independently of Text.width's rounding.
def report.face(+f: X.Face, +cache: Map<&2, X.TG>) -> IO(Unit):
  match f:
    case X.Face{_, _, +px, +a, +d, +lh, _, +name, _}:
      do IO<Unit>:
        IO.print(name ++ " px=" ++ U32.show(px) ++ " asc/desc/lh=" ++ U32.show(a) ++ "/" ++ U32.show(d) ++ "/" ++ U32.show(lh))
        report.glyphs("直骨写画日本語あいうえお「」。（）ー", cache, f, a, d, X.TG.adv(X.Text.get(cache, f, 48)))

def warm(+f: X.Face, c: Map<&2, X.TG>) -> IO(Map<&2, X.TG>):
  do IO<Map<&2, X.TG>>:
    c1 : Map<&2, X.TG> <- X.Text.warm(c, f)
    X.Text.ensure(c1, f, all())

def draw() -> IO(Unit):
  do IO<Unit>:
    +f12 : X.Face <- X.Text.face(["Berkeley Mono", "monospace"], 12)
    +f13 : X.Face <- X.Text.face(["Berkeley Mono", "monospace"], 13)
    +f14 : X.Face <- X.Text.face(["Berkeley Mono", "monospace"], 14)
    +fp : X.Face <- X.Text.face(["DejaVu Sans", "Liberation Sans", "sans-serif"], 14)
    +fj : X.Face <- X.Text.face(["Noto Sans CJK JP"], 14)
    +c12 : Map<&2, X.TG> <- warm(f12, Map.new(&2, X.TG))
    +c13 : Map<&2, X.TG> <- warm(f13, c12)
    +c14 : Map<&2, X.TG> <- warm(f14, c13)
    +cp : Map<&2, X.TG> <- X.Text.ensure(c14, fp, "日本語")
    +c : Map<&2, X.TG> <- X.Text.ensure(cp, fj, "日本語")
    IO.print(Bool.pick(String, U32.is_eq(X.TG.adv(X.Text.get(c, fp, 26412)), X.TG.adv(X.Text.get(c, fj, 26412))), "ok proportional fallback keeps its own advance", "FAIL proportional fallback advance"))
    report.face(f12, c)
    report.face(f13, c)
    report.face(f14, c)
    +ops : List<&2, R.Op> = List.concat(&2, R.Op, [band(c, f12, False{}, 24, 20, 12), band(c, f13, False{}, 24, 340, 13), band(c, f14, False{}, 24, 660, 14), band(c, f12, True{}, 524, 20, 12), band(c, f13, True{}, 524, 340, 13), band(c, f14, True{}, 524, 660, 14), [D.Draw.rect(D.Clip.all(), 500, 0, 500, 990, T.Th.light(T.Th.bg()), 255)]])
    +img : Image = R.Frame.render(1000, 990, ops, T.Th.bg())
    f : File <- IO.try(File, File.open("/tmp/cjk.ppm", "w"))
    r : File & Result<&1, &1, U32 & String, Unit> <- File.write_bytes(f, D.Snap.ppm(img, 1000, 990))
    IO.print("/tmp/cjk.ppm")

def missing.glyph(g: X.TG, +miss: U32) -> IO(Unit):
  match g:
    case X.TG{+adv, _, _, +w, +h, _, _}:
      IO.print(Bool.pick(String, U32.is_eq(adv, miss) && U32.is_eq(w, 0) && U32.is_eq(h, 0), "ok missing CJK stays blank at miss advance", "FAIL missing CJK glyph"))

def missing.open(r: Result<&1, &1, U32 & String, U32>) -> IO(Unit):
  do IO<Unit>:
    +f : X.Face <- X.Text.face.o(r, 14, "explicit primary; no fontconfig fonts")
    +c : Map<&2, X.TG> <- X.Text.ensure(Map.new(&2, X.TG), f, "日本語")
    missing.glyph(X.Text.get(c, f, 26085), X.Face.miss(f))
    missing.glyph(X.Text.get(c, f, 26412), X.Face.miss(f))
    missing.glyph(X.Text.get(c, f, 35486), X.Face.miss(f))

def fixture.mode(no_font: Bool, file: String) -> IO(Unit):
  match no_font:
    case True{}:
      IO.bind(Result<&1, &1, U32 & String, U32>, Unit, FX.Font.open(file, 0, 14), r => missing.open(r))
    case False{}:
      draw()

# FONTCONFIG_FILE=<empty config> build/cjk_frame no-font <primary file>
# verifies the missing-font path without changing the machine's fonts.
def fixture.args(args: List<&1, String>) -> IO(Unit):
  match args:
    case _ <> mode <> file <> _:
      fixture.mode(String.eq(mode, "no-font"), file)
    case _:
      draw()

def main() -> IO(Unit):
  IO.bind(List<&1, String>, Unit, IO.args(), a => fixture.args(a))
