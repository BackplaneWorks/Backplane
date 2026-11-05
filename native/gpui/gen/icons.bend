import Base
import ../../../src/core/icons.bend as I
import ../../../src/core/svg.bend as S
import ../../../src/core/json.bend as J

# Prints the icon set (core/icons.bend, Lucide) one line per icon:
#   icon <name> [{"d": absolute path data, "fill": bool}, ...]
# then "done". scripts/gpui/boat-build.sh runs it on Boat and writes assets/icons.json.
# Generated, never hand-copied.

def layer(l: I.Layer) -> J.Json:
  match l:
    case I.Layer{d, +f}:
      J.Json.obj([J.KV{"d", J.Str{S.Svg.show(S.Svg.parse(d))}}, J.KV{"fill", J.Json.bool(f)}])

def layers(ls: List<&2, I.Layer>) -> List<&2, J.Json>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      layer(l) <> layers(t)

def icons(ds: List<&2, I.IDef>) -> IO(Unit):
  match ds:
    case Nil{}:
      IO.print("done")
    case I.IDef{n, ls} <> t:
      do IO<Unit>:
        IO.print("icon " ++ n ++ " " ++ J.Json.show(J.Json.arr(layers(ls))))
        icons(t)

def main() -> IO(Unit):
  icons(I.Icon.all())
