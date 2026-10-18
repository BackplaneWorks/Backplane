import Base
import ../src/core/i18n.bend as I
import ./check.bend as C
import ../src/app/layout.bend as L
import ../src/core/chip.bend as CH
import ../src/core/links.bend as LK
import ../src/gfx/text.bend as X

# Compare every field, including saved draw records and bubble copy fields.
def same(a: List<&2, L.TLine>, b: List<&2, L.TLine>) -> Bool:
  match a b:
    case Nil{} Nil{}:
      True{}
    case L.TLine{a0, a1, a2, a3, a4} <> at L.TLine{b0, b1, b2, b3, b4} <> bt:
      Bool.and(String.eq(a0, b0), Bool.and(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(String.eq(a3, b3), Bool.and(U32.is_eq(a4, b4), same(at, bt))))))
    case _ _:
      False{}

def cp(+text: String, +links: String, +cs: List<&2, CH.Chip>) -> L.Cp:
  L.Cp{text, links, CH.Chip.enc(cs), Nil{}}

def old.bubble(+ty: L.Ty, +t: String, +links: String, +cs: List<&2, CH.Chip>, +px: U32) -> List<&2, L.TLine>:
  +ls = L.Ty.wrap(ty, 3n, t, U32.min((px * 7 / 10 : U32), 560))
  +w = {(L.TL.widest(ty, ls, 0) + 40 : U32) : U32}
  List.concat(&2, L.TLine, [[L.TLine{"", 0, 6, "", w}], L.TL.bubble.lines(String.is_empty(links), ty, ls, links, CH.Chip.enc(cs), "original", w), [L.TLine{"", 0, 6, "", w}, L.TLine{"", 0, 4, "", 0}]])

def check(+ty: L.Ty, +text: String, +links: String, +cs: List<&2, CH.Chip>, +px: U32) -> IO(Unit):
  do IO<Unit>:
    C.Check.str("paragraph exact " ++ U32.show(px), Bool.show(same(L.TL.rpara(ty, cp(text, links, cs), px), L.TL.rpara.k(String.is_empty(links), ty, L.Ty.wrap(ty, 3n, text, px), links, CH.Chip.enc(cs)))), "True")
    C.Check.str("list exact " ++ U32.show(px), Bool.show(same(L.TL.ritem(ty, cp(text, links, cs), "•  ", "   ", px), L.TL.ritem.k(String.is_empty(links), ty, L.Ty.wrap(ty, 3n, text, px), links, CH.Chip.enc(cs), "•  ", "   "))), "True")
    C.Check.str("bubble exact " ++ U32.show(px), Bool.show(same(L.TL.bubble.b(ty, cp(text, links, cs), "original", Nil{}, px), old.bubble(ty, text, links, cs, px))), "True")

def widths(ws: List<&2, U32>, +ty: L.Ty, +text: String, +links: String, +cs: List<&2, CH.Chip>) -> IO(Unit):
  match ws:
    case Nil{}:
      IO.pure(Unit, Unit{})
    case +w <> +t:
      do IO<Unit>:
        check(ty, text, links, cs, w)
        widths(t, ty, text, links, cs)

def run(+ty: L.Ty) -> IO(Unit):
  do IO<Unit>:
    +a : CH.Chip = CH.Chip.make("backplane://thread/a", "message", "Same · idle", "")
    +b : CH.Chip = CH.Chip.make("backplane://thread/b", "message", "Same · idle", "")
    +r : CH.Chip = CH.Chip.make("backplane://thread/L~t", "message", "Remote · working", "Boat")
    widths([16, 120, 320, 900], ty, "before " ++ CH.Chip.label.of(a) ++ ", " ++ CH.Chip.label.of(b) ++ " and " ++ CH.Chip.label.of(r) ++ ". after", "", [a, b, r])
    widths([16, 120, 320, 900], ty, "  before  " ++ CH.Chip.label.of(a) ++ "\n\nhttps://example.com  " ++ CH.Chip.label.of(r) ++ " ", LK.Link.enc([LK.PLink{"https://example.com", "https://example.com"}]), [a, r])
    widths([16, 120, 320, 900], ty, "日本語（" ++ CH.Chip.label.of(a) ++ "）中文。", "", [a])
    widths([16, 120, 320, 900], ty, "plain  text\nwith_no_refs_whatsoever", "", Nil{})

# Unequal fractional advances catch rounding differences hidden by the
# bitmap face. U+00B7 and CJK use the map; ASCII comes from the face table.
def fractional() -> L.Ty:
  +cache = Map.set(&2, X.TG, Map.set(&2, X.TG, Map.set(&2, X.TG, Map.new(&2, X.TG), X.Text.key(0, 32), X.TG.blank(129)), X.Text.key(0, 95), X.TG.blank(211)), X.Text.key(0, 183), X.TG.blank(301))
  +f = {X.Face{0, True{}, 14, 10, 4, 16, 483, "fractional", X.GTab.of(7n, cache, 0, 0)} : X.Face}
  L.Ty{cache, f, f, f, f, f, I.En{}}

def main() -> IO(Unit):
  do IO<Unit>:
    run(L.Ty.none())
    run(fractional())
