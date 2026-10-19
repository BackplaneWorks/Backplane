import Base
import ../../src/core/json.bend as J
import ../../src/server/host.bend as H
import ../../src/server/review.bend as R

# The real release child, without starting a hub or publishing anything.
# A second mode checks the existing publication gate with a failed result.
def report(r: R.Res) -> IO(Unit):
  match r:
    case R.Res{_, +o}:
      IO.print(J.Json.show(o))

def args(xs: List<String>) -> IO(Unit):
  match xs:
    case "--failed-report" <> cwd <> Nil{}:
      IO.bind(R.Res, Unit, R.Rvr.made(J.Json.obj([J.KV{"err", J.Str{"synthetic export failure"}}]), "t", cwd, "fixture-tag", True{}), report)
    case _:
      IO.die(Unit, 2, "usage: release_child --review release DIR TAG HOME")

def linked(ok: Bool) -> IO(Unit):
  match ok:
    case True{}:
      IO.bind(List<String>, Unit, IO.args(), xs => R.Rvc.main(xs, args))
    case False{}:
      IO.die(Unit, 1, "host linkage failed")

def main() -> IO(Unit):
  # host.c's join helpers need its CBOR buffer in small native targets.
  IO.bind(List<&2, U32>, Unit, H.Json.cbor_frame("null", "", ""), frame => linked(Nat.is_eq(List.length(&2, U32, frame), 3n)))
