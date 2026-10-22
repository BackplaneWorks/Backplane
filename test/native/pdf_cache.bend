import Base
import ../../src/server/pdf.bend as PDF
import ../../src/server/proc.bend as P
import ../../src/server/host.bend as H
import ../../src/server/argv.bend as A

# Run the same command as PdfJob, with temporary inputs supplied by the test.
def finish(code: U32) -> IO(Unit):
  match code:
    case 0: IO.pure(Unit, Unit{})
    case _: IO.die(Unit, code, "PDF render failed")

def done(r: P.Ran) -> IO(Unit):
  match r:
    case P.Ran{code, out}:
      do IO<Unit>:
        IO.print(out)
        finish(code)

def run(args: List<&1, String>) -> IO(Unit):
  match args:
    case path <> page <> cache <> _:
      IO.bind(P.Ran, Unit, P.Proc.run("sh", ["-c", PDF.Pdf.sh(), "sh", path, page, cache], "/"), done)
    case _:
      IO.die(Unit, 2, "usage: pdf_cache PATH PAGE CACHE")

def ready(count: Nat) -> IO(Unit):
  match count:
    case 3n:
      do IO<Unit>:
        args : List<&1, String> <- A.Argv.args()
        run(args)
    case _:
      IO.die(Unit, 1, "CBOR fixture setup failed")

def main() -> IO(Unit):
  do IO<Unit>:
    # host.c's socket helpers also use its native CBOR buffer implementation.
    frame : List<&2, U32> <- H.Json.cbor_frame("null", "", "")
    ready(List.length(&2, U32, frame))
