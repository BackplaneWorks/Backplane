import Base
import ../../../src/core/cat.bend as C
import ../../../src/core/json.bend as J

# Prints every cat rig the desktop client can draw, one line each:
#   <look> <mood> <Cat.rig.json>
# (looks 0..71 x the eight moods), then "done". scripts/gpui/boat-build.sh runs it
# on Boat and splits the lines into cat-rigs/<look>-<mood>.json. Generated, never hand-copied.

def rig(+look: Nat, mood: String) -> String:
  Nat.show(look) ++ " " ++ mood ++ " " ++ J.Json.show(C.Cat.rig.json(look, mood))

def moods(+look: Nat, ms: List<&2, String>) -> IO(Unit):
  match ms:
    case Nil{}:
      IO.print("")
    case +m <> t:
      do IO<Unit>:
        IO.print(rig(look, m))
        moods(look, t)

def looks(+n: Nat) -> IO(Unit):
  match n:
    case 0n:
      IO.print("done")
    case 1n+k:
      do IO<Unit>:
        moods(k, C.Cat.moods())
        looks(k)

def main() -> IO(Unit):
  looks(C.Cat.looks())
