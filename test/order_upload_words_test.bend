import Base
import ./check.bend as C
import ../src/core/order.bend as O
import ../src/core/json.bend as J

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("one file", O.Order.ask.count(1n), "1 file")
    C.Check.str("several files", O.Order.ask.count(2n), "2 files")
    C.Check.str("no files", O.Order.ask.count(0n), "0 files")
    C.Check.str("single-file approval", Bool.show(String.starts_with(O.Order.ask.detail("mfg/board", J.Null{}, ["gerbers.zip"]), "Send 1 file from mfg/board")), "True")
