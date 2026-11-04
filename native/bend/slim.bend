import Base
import ../../src/core/json.bend as J
import ../../src/core/client.bend as U
import ../../src/mobile/hubs.bend as H
import ../../src/mobile/session.bend as MS

# The slim backend's policy, in machine code: Backplane's own event fold and
# screen model, with no JS. `slim EVENTS.jsonl KEY` folds a hub's event log
# and prints the screen every native client draws. Only Base IO: no effects.

def read.k(r: File & Result<&1, &1, U32 & String, String>) -> IO(String):
  (f, res) = r
  do IO<String>:
    File.close(f)
    s : String <- IO.pass(String, res)
    return s

def read(+path: String) -> IO(String):
  do IO<String>:
    f : File <- IO.try(File, File.open(path, "r"))
    r : File & Result<&1, &1, U32 & String, String> <- File.read(f, 2000000000)
    read.k(r)

def Slim.item(m: Maybe<&2, J.Json>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match m:
    case None{}: acc
    case Some{j}: j <> acc

def Slim.items(xs: List<&2, String>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: List.reverse(&2, J.Json, acc)
    case x <> t: Slim.items(t, Slim.item(J.Json.parse(x), acc))

def Slim.screen(+u: U.Ui, +key: String, +now: Nat) -> J.Json:
  MS.Session.result.shown(H.Hubs{"slim", now, key, [H.Hub{key, u}]}, Nil{}, key, H.Hubs.shown(H.Hubs.loc(H.Hubs{"slim", now, key, [H.Hub{key, u}]}), [H.Hub{key, u}], key))

def copy(xs: List<&1, String>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case x <> t: x <> copy(t)

def arg(+xs: List<&2, String>, +i: Nat) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, xs, i)), "")

def main() -> IO(Unit):
  do IO<Unit>:
    a : List<&1, String> <- IO.args()
    +xs : List<&2, String> = copy(a)
    +now : Nat <- IO.now()
    text : String <- read(arg(xs, 1n))
    +u : U.Ui = U.Ui.fold.items(Slim.items(String.lines(text), Nil{}), U.Ui.init(now))
    IO.print(J.Json.show(Slim.screen(u, arg(xs, 2n), now)))
