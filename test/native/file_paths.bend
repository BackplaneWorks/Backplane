import Base
import ../../src/server/filelist.bend as F
import ../../src/server/proc.bend as P
import ../../src/server/host.bend as H
import ../../src/server/argv.bend as A
import ../../src/core/json.bend as J

def done(r: P.Ran) -> IO(Unit):
  match r:
    case P.Ran{0, out}: IO.print(J.Json.show(F.FileList.result(out)))
    case P.Ran{code, _}: IO.die(Unit, code, "file listing failed")

def run(args: List<&1, String>) -> IO(Unit):
  match args:
    case mode <> cwd <> _:
      IO.bind(P.Ran, Unit, P.Proc.run("sh", ["-c", F.FileList.cmd(String.eq(mode, "git"))], cwd), done)
    case _: IO.die(Unit, 2, "usage: file_paths git|find FOLDER")

def ready(count: Nat) -> IO(Unit):
  match count:
    case 3n:
      do IO<Unit>:
        args : List<&1, String> <- A.Argv.args()
        run(args)
    case _: IO.die(Unit, 1, "CBOR fixture setup failed")

def main() -> IO(Unit):
  do IO<Unit>:
    frame : List<&2, U32> <- H.Json.cbor_frame("null", "", "")
    ready(List.length(&2, U32, frame))
