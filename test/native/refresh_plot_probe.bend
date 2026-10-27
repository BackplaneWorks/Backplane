import Base
import ../../src/server/plot.bend as PV
import ../../src/server/host.bend as H
import ../../src/server/argv.bend as AV
import ../../src/core/hier.bend as HI

# The hub's real plot actor and own --plot children, without HTTP/UI.
# Emits the exact phone/web CBOR payload decoded to JSON for assertions.

def hex.digit(+n: U32) -> Char:
  Chr{Bool.pick(U32, U32.is_lt(n, 10), U32.add(n, 48), U32.add(n, 87))}

def hex.go(bs: List<&2, U32>, acc: String) -> String:
  match bs:
    case Nil{}: String.reverse(acc)
    case +b <> t:
      hex.go(t, SCon{hex.digit(U32.and(b, 15)), SCon{hex.digit(U32.shrn(b, 4n)), acc}})

def receive(m: Maybe<&1, PV.POut>, k: Unit -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{PV.PBin{_, bs}}:
      do IO<Unit>:
        IO.print("plothex " ++ hex.go(bs, ""))
        k(Unit{})
    case Some{PV.POut{_, text}}:
      do IO<Unit>:
        IO.print("note " ++ text)
        k(Unit{})
    case None{}: IO.pure(Unit, Unit{})

def consume(fuel: Nat, +out: Chan(PV.POut)) -> IO(Unit):
  match fuel:
    case 0n: IO.pure(Unit, Unit{})
    case 1n+f:
      IO.bind(Maybe<&1, PV.POut>, Unit, Chan.recv(PV.POut, out), m => receive(m, u => consume(f, out)))

def stop(+q: Chan(PV.PMsg), +out: Chan(PV.POut)) -> IO(Unit):
  do IO<Unit>:
    IO.sleep(12000)
    Chan.close(PV.PMsg, q)
    Chan.close(PV.POut, out)

def actor(+path: String, +dir: String) -> IO(Unit):
  do IO<Unit>:
    # Force host.c's CBOR support into this native component too.
    framed : List<&2, U32> <- H.Json.items_frames("{", "", "", Nil{}, "}", 1, 100, 100, "", "")
    +q : Chan(PV.PMsg) <- Chan.new(PV.PMsg, 32)
    +out : Chan(PV.POut) <- Chan.new(PV.POut, 32)
    IO.spawn(Unit, PV.Pv.loop(1000n, q, PV.Pv.empty(), out, dir))
    ok : Result<&1, &1, PV.PMsg, Unit> <- Chan.send(PV.PMsg, q, PV.PWatch{1, 1, HI.Hier.dir(path), path, ""})
    IO.spawn(Unit, stop(q, out))
    consume(200n, out)

def dispatch(args: List<String>) -> IO(Unit):
  match args:
    case "--plot" <> kind <> path <> info <> _:
      PV.Pv.child(Maybe.default(&2, U32, U32.read(kind), 1), path, info)
    case "--plot-tree" <> path <> _:
      PV.Pv.child.tree(path)
    case path <> dir <> _:
      actor(path, dir)
    case _: IO.pure(Unit, Unit{})

def main() -> IO(Unit):
  IO.bind(List<String>, Unit, AV.Argv.args(), dispatch)
