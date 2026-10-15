import Base
import ../src/core/acp.bend as A
import ../src/core/json.bend as J
import ../src/core/utf8.bend as U

# Helpers shared by the tests that replay a recorded ACP-style turn line by
# line: the run's state and what it sent and reported, the JSON readers, and
# reading a fixture file. Each test keeps its own step, since each feeds a
# different turn type to its own line translator.

type Run is Data:
  Run{a: A.Acp, sends: List<&2, String>, emits: List<&2, String>}

def Run.new() -> Run:
  Run{A.Acp.new(), Nil{}, Nil{}}

def Run.add(o: A.AcpOut, +ss: List<&2, String>, +es: List<&2, String>) -> Run:
  match o:
    case A.AcpOut{a, s, e, _}:
      Run{a, List.append(&2, String, ss, s), List.append(&2, String, es, e)}

def sends(r: Run) -> List<&2, String>:
  match r:
    case Run{_, s, _}:
      s

def emits(r: Run) -> String:
  match r:
    case Run{_, _, e}:
      String.join(e, " | ")

def done(r: Run) -> String:
  match r:
    case Run{A.Acp{_, _, True{}}, _, _}:
      "done"
    case Run{_, _, _}:
      "open"

def obj(+line: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{})

def method(+line: String) -> String:
  J.Json.get_str(obj(line), "method", "")

def methods(xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      method(x) <> methods(t)

def params(+line: String) -> J.Json:
  J.Json.get_obj(obj(line), "params")

def nth(xs: List<&2, String>, n: Nat) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, xs, n)), "")

def yes(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def lines.r(r: U.Lines & List<&2, String>) -> List<&2, String>:
  (_, ls) = r
  ls

def file.done(r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(List<&2, String>):
  (f, res) = r
  do IO<List<&2, String>>:
    File.close(f)
    return lines.r(U.Lines.feed(U.Lines.new(), Result.default(&1, &1, U32 & String, List<&2, U32>, res, Nil{})))

def file.sized(r: File & Result<&1, &1, U32 & String, U32>) -> IO(List<&2, String>):
  (f, res) = r
  IO.bind(File & Result<&1, &1, U32 & String, List<&2, U32>>, List<&2, String>, File.read_bytes(f, U32.max(Result.default(&1, &1, U32 & String, U32, res, 0), 1)), file.done)

def file.opened(r: Result<&1, &1, U32 & String, File>) -> IO(List<&2, String>):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, U32>, List<&2, String>, File.size(f), file.sized)
    case Fail{_}:
      IO.pure(List<&2, String>, Nil{})

# a file's lines ("" lines when it cannot be read)
def file(path: String) -> IO(List<&2, String>):
  IO.bind(Result<&1, &1, U32 & String, File>, List<&2, String>, File.open(path, "r"), file.opened)
