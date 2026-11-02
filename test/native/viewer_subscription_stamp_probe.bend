import Base
import ../../src/server/plot.bend as PV
import ../../src/server/argv.bend as AV
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
    +first : PV.PSt <- F.probe.handle(initial(path), PV.PAlso{7, 2, "/fixture", path, "", True{}}, "join-busy")
    +again : PV.PSt <- F.probe.handle(first, PV.PAlso{7, 2, "/fixture", path, "", True{}}, "repeat-watch")
    last : PV.PSt <- F.probe.handle(again, PV.PAlso{8, 2, "/fixture", path, "", False{}}, "legacy")
    return Unit{}

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- AV.Argv.args()
    run(Maybe.default(&1, String, List.head(&1, String, args), "/missing"))
