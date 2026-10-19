import Base
import ./check.bend as C
import ../src/server/filebytes.bend as F

def bytes(bs: List<&2, U32>) -> List<&2, String>:
  match bs:
    case Nil{}: Nil{}
    case b <> t: U32.show(b) <> bytes(t)

def show(r: Result<&2, &2, String, List<&2, U32>>) -> String:
  match r:
    case Done{bs}: "ok:" ++ String.join(bytes(bs), ",")
    case Fail{why}: "error:" ++ why

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("empty file succeeds", show(F.FileBytes.result(Done{Nil{}})), "ok:")
    C.Check.str("binary file bytes preserved", show(F.FileBytes.result(Done{[0, 10, 128, 255]})), "ok:0,10,128,255")
    C.Check.str("open permission failure retained", show(F.FileBytes.result(Fail{(13, "Permission denied")})), "error:Permission denied")
    C.Check.str("read failure retained", show(F.FileBytes.result(Fail{(5, "Input/output error")})), "error:Input/output error")
    C.Check.str("size failure retained", show(F.FileBytes.result(Fail{(75, "Value too large")})), "error:Value too large")
    C.Check.str("failure without diagnostic stays failure", show(F.FileBytes.result(Fail{(5, "")})), "error:")
