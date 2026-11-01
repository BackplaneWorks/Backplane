import Base
import ../../src/server/argv.bend as AV
import ../../src/core/far.bend as FR
import ../../src/core/model.bend as M
import ../../src/core/utf8.bend as U

# `far_load_probe STAGE FILE`: a far mirror (<home>/far/<link>.jsonl) read
# as the hub's Hub.far.loaded reads it, up to STAGE (text, lines, read, view, msg, msgtext: the message itself), so
# /usr/bin/time -v shows what each stage costs in memory.
# Run: bend test/native/far_load_probe.bend STAGE FILE

def probe.stage(stage: String, +text: String) -> String:
  match stage:
    case "text": "chars " ++ Nat.show(String.length(text))
    case "lines": "lines " ++ Nat.show(List.length(&2, String, String.lines(text)))
    case "read": "items " ++ Nat.show(List.length(&2, FR.Item, FR.In.items(FR.Fr.file.read(text, "p0", "far"))))
    case "view": "seq " ++ Nat.show(M.State.seq(FR.In.view(FR.Fr.file.read(text, "p0", "far"))))
    case "msg": "msg " ++ Nat.show(String.length(FR.In.msg(FR.Fr.file.read(text, "p0", "far"))))
    case "msgtext": FR.In.msg(FR.Fr.file.read(text, "p0", "far"))
    case _: "stage: lines | read | view | msg"

def probe.done(r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(List<&2, U32>):
  (f, res) = r
  do IO<List<&2, U32>>:
    File.close(f)
    return Result.default(&1, &1, U32 & String, List<&2, U32>, res, Nil{})

def probe.sized(r: File & Result<&1, &1, U32 & String, U32>) -> IO(List<&2, U32>):
  (f, res) = r
  IO.bind(File & Result<&1, &1, U32 & String, List<&2, U32>>, List<&2, U32>,
    File.read_bytes(f, U32.max(Result.default(&1, &1, U32 & String, U32, res, 0), 1)), probe.done)

def probe.opened(r: Result<&1, &1, U32 & String, File>) -> IO(List<&2, U32>):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, U32>, List<&2, U32>, File.size(f), probe.sized)
    case Fail{_}:
      IO.pure(List<&2, U32>, Nil{})

# as server.bend's Files.text (Far.load.job reads a mirror with it)
def probe.text(path: String) -> IO(String):
  do IO<String>:
    bs : List<&2, U32> <- IO.bind(Result<&1, &1, U32 & String, File>, List<&2, U32>, File.open(path, "r"), probe.opened)
    return U.Utf8.decode(bs)

def probe.k(xs: List<String>) -> IO(Unit):
  match xs:
    case stage <> file <> _:
      do IO<Unit>:
        text : String <- probe.text(file)
        IO.print(probe.stage(stage, text))
    case _:
      IO.print("usage: far_load_probe STAGE FILE")

def main() -> IO(Unit):
  IO.bind(List<String>, Unit, AV.Argv.args(), probe.k)
