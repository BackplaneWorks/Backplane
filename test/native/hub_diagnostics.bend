import Base
import ../../src/server/main.bend as Hub
import ../../src/server/jsloop.bend as Loop
import ../../src/core/cli.bend as CL

# Real hub, with a test-only effect that fails after the desktop connects.
def Fault.raise() -> IO(Unit):
  import "./hub-diagnostics.js"

def Fault.ready() -> IO(Bool):
  import "./hub-diagnostics.js"

def Fault.next(ready: Bool, again: Unit -> IO(Unit)) -> IO(Unit):
  match ready:
    case True{}: Fault.raise()
    case False{}: again(Unit{})

def Fault.watch(fuel: Nat) -> IO(Unit):
  match fuel:
    case 0n: IO.pure(Unit, Unit{})
    case 1n+p:
      do IO<Unit>:
        IO.sleep(100)
        ready : Bool <- Fault.ready()
        Fault.next(ready, u => Fault.watch(p))

def Fault.child(xs: List<&2, String>) -> U32:
  match xs:
    case Nil{}: 0
    case h <> _: CL.Cli.child(h)

def Fault.run(child: U32) -> IO(Unit):
  match child:
    case 0:
      do IO<Unit>:
        IO.spawn(Unit, Hub.main())
        Fault.watch(36000n)
    case _:
      Hub.main()

def main() -> IO(Unit):
  do IO<Unit>:
    rt : String <- Loop.Rt.loop()
    args : List<String> <- IO.args()
    Fault.run(Fault.child(CL.Cli.args(args)))
