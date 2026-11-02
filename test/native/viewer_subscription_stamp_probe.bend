import Base
import ../../src/server/plot.bend as PV
import ../../src/core/json.bend as J
import ./viewer_revision_state_probe.bend as F

# Join an existing PCB/assembly read. No poll, CAD child or PGot ever runs.
def initial(+path: String) -> PV.PSt:
  +board = PV.Src.key(0, "/fixture", path)
  +model = PV.Src.key(2, "/fixture", path)
  PV.PSt{[PV.Src{board, 0, path, 1, "earlier-save", 0n, True{}, None{}, ""},
    PV.Src{model, 2, path, 1, "earlier-save", 0n, True{}, None{}, ""}],
    [PV.Sub{3, board, Nil{}, "", True{}, "", True{}}, PV.Sub{3, model, Nil{}, "", True{}, "", True{}}], True{}}

def run(+path: String) -> IO(Unit):
  do IO<Unit>:
    IO.print(J.Json.show(J.Json.obj([J.KV{"stage", J.Str{"source"}}, J.KV{"path", J.Str{path}}])))
    +first : PV.PSt <- F.probe.handle(initial(path), PV.PAlso{7, 2, "/fixture", path, "", True{}}, "join-busy")
    +again : PV.PSt <- F.probe.handle(first, PV.PAlso{7, 2, "/fixture", path, "", True{}}, "repeat-watch")
    last : PV.PSt <- F.probe.handle(again, PV.PAlso{8, 2, "/fixture", path, "", False{}}, "legacy")
    return Unit{}

def main() -> IO(Unit):
  do IO<Unit>:
    source : Result<&1, &1, U32 & String, String> <- IO.get_env("VIEWER_STAMP_TEST_SOURCE")
    run(Result.default(&1, &1, U32 & String, String, source, "/missing"))
