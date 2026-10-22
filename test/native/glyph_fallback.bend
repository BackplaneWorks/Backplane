import Base
import ../../src/gfx/text.bend as X

# Glyphs the UI's face lacks come from a fallback font: each code point is
# fetched as the window does (a want, then Text.fill) and printed with its
# size (w x h 0 x 0: nothing drew it).
#   scripts/build-js.sh test/native/glyph_fallback.bend build/glyph_fallback
#   build/glyph_fallback

def dims(g: X.TG) -> String:
  match g:
    case X.TG{adv, _, _, w, h, _, _}:
      U32.show(w) ++ "x" ++ U32.show(h) ++ " adv " ++ U32.show(adv)

def wants(s: String, +id: U32, +miss: U32) -> List<&2, X.Want>:
  match s:
    case SNil{}:
      Nil{}
    case SCon{Chr{+cp}, t}:
      X.Want{id, cp, miss} <> wants(t, id, miss)

def show(s: String, +cache: Map<&2, X.TG>, +f: X.Face) -> IO(Unit):
  match s:
    case SNil{}:
      IO.pure(Unit, Unit{})
    case SCon{Chr{+cp}, t}:
      IO.bind(Unit, Unit, IO.print("U+" ++ U32.show(cp) ++ " " ++ SCon{Chr{cp}, SNil{}} ++ ": " ++ dims(X.Text.get(cache, f, cp))), u => show(t, cache, f))

def sample() -> String:
  "αβγΩλπ€日本→✓"

def main() -> IO(Unit):
  do IO<Unit>:
    +s : String = sample()
    +f : X.Face <- X.Text.face(["Berkeley Mono", "monospace"], 14)
    IO.print("face: " ++ X.Face.name(f))
    +c : Map<&2, X.TG> <- X.Text.fill(wants(s, X.Face.id(f), X.Face.miss(f)), Map.new(&2, X.TG))
    show(s, c, f)
