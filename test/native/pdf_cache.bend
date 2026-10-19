import Base
import ../../src/server/pdf.bend as PDF
import ../../src/server/proc.bend as P
import ../../src/server/host.bend as H
import ../../src/server/argv.bend as A

# Run the same command as PdfJob, with temporary inputs supplied by the test.
def done(r: P.Ran) -> IO(Unit):
  match r:
    case P.Ran{code, out}:
      do IO<Unit>:
        IO.print(out)
        match code:
          case 0: IO.pure(Unit, ())
          case _: IO.die(Unit, code, "PDF render failed")

def run(args: List<&1, String>) -> IO(Unit):
  match args:
    case path <> page <> cache <> _:
      IO.bind(P.Ran, Unit, P.Proc.run("sh", ["-c", PDF.Pdf.sh(), "sh", path, page, cache], "/"), done)
    case _:
      IO.die(Unit, 2, "usage: pdf_cache PATH PAGE CACHE")

def main() -> IO(Unit):
  do IO<Unit>:
    # host.c's socket helpers also use its native CBOR buffer implementation.
    frame : List<&2, U32> <- H.Json.cbor_frame("null", "", "")
    match List.length(&2, U32, frame):
      case 3n:
        args : List<&1, String> <- A.Argv.args()
        run(args)
      case _:
        IO.die(Unit, 1, "CBOR fixture setup failed")
