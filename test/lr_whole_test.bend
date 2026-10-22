import Base
import ./check.bend as C
import ../src/core/hub.bend as H
import ../src/core/model.bend as M
import ../src/core/thin.bend as T
import ../src/core/thin-wire.bend as W
import ../src/core/json.bend as J
import ../src/server/thin-view.bend as V

# Launch reports are kept whole (hub.bend's Lr.note), and a report stored
# cut short before that reads whole when its original is exactly
# recoverable (Lr.whole, thin-view's Message.whole). The last line printed
# is the fixture a UI check may read.

# t1's reply: over Lr.cut() characters, under 8 KiB (one read completes it)
def reply() -> String:
  String.repeat("Step done: the board passes DRC and ERC. ", 80n) ++ "Last line ✓"

# t1 (launched by t0) ended a turn with that reply, reported at 9
def st() -> M.State:
  H.Lr.demo.k(True{}, False{}, False{}, [M.MessagePosted{"t1", "a2", M.Assistant{}, reply(), 3n}])

def head() -> String:
  H.Lr.head("t1", "work") ++ " completed:\n\n"

# the row as it was stored before: the reply's first Lr.cut() characters and the tail
def legacy() -> String:
  head() ++ String.take(reply(), H.Lr.cut()) ++ H.Lr.cut.tail("t1")

def id() -> String:
  H.Lr.msg("t1", "r9")

# what the real Lr path posts to t0 now
def told.k(+cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}: ""
    case M.MessagePosted{"t0", _, _, +text, _} <> _: text
    case _ <> +t: told.k(t)

def told() -> String:
  told.k(H.Queue.changes(H.Launch.tell(st(), 9n, "t1", H.Step{H.Lr.demo.end(), Nil{}})))

# t0's view: the legacy row, ordinal 1
def index() -> T.Index:
  T.Index.post(T.Index.empty(), M.Msg{id(), "t0", M.User{}, legacy(), 9n})

def target() -> J.Json:
  V.Target.json("thread", "t0")

def request() -> J.Json:
  J.Json.obj([J.KV{"target", target()}, J.KV{"entryId", J.Str{id()}}, J.KV{"cursor", J.Str{""}}])

def read() -> V.MessageRead:
  V.Message.read(index(), "t0", request(), "epoch", ["1"], M.State.apply(st(), M.MessagePosted{"t0", id(), M.User{}, legacy(), 9n}))

def row(+r: V.MessageRead) -> T.Row:
  match r:
    case V.MessageRead{_, +row, _}: row

def result() -> J.Json:
  V.Message.result(row(read()), 0n, request(), "epoch")

def fixture() -> String:
  J.Json.show(J.Json.obj([J.KV{"entryId", J.Str{id()}}, J.KV{"owner", J.Str{"t0"}}, J.KV{"source", J.Str{"t1"}}, J.KV{"at", J.Json.nat(9n)},
    J.KV{"version", J.Str{Nat.show(T.Row.n(row(read())))}}, J.KV{"preview", J.Str{legacy()}}, J.KV{"fullText", J.Str{head() ++ reply()}},
    J.KV{"newReport", J.Str{told()}}]))

def window.whole(es: List<&2, M.Entry>) -> Bool:
  match es:
    case Nil{}: False{}
    case +e <> t: Bool.or(String.eq(T.Entry.text(e), head() ++ reply()), window.whole(t))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("the reply is over the old cut and under 8 KiB", Bool.show(Bool.and(Nat.is_gt(String.length(reply()), H.Lr.cut()), Nat.is_lt(String.length(reply()), 8000n))), "True")
    C.Check.str("a new report carries the reply whole", Bool.show(String.ends_with(told(), reply())), "True")
    C.Check.str("a new report has no cut-short hint", Bool.show(String.contains(told(), "cut short")), "False")
    C.Check.str("a legacy cut row reads whole", Bool.show(String.eq(H.Lr.whole(st(), id(), legacy(), 9n), head() ++ reply())), "True")
    C.Check.str("not a report id: left as stored", Bool.show(String.eq(H.Lr.whole(st(), M.Link.id("t1", "m5"), legacy(), 9n), legacy())), "True")
    C.Check.str("text that is not the reply's exact cut: left as stored", Bool.show(String.eq(H.Lr.whole(st(), id(), head() ++ "x" ++ String.take(reply(), H.Lr.cut()) ++ H.Lr.cut.tail("t1"), 9n),
      head() ++ "x" ++ String.take(reply(), H.Lr.cut()) ++ H.Lr.cut.tail("t1"))), "True")
    C.Check.str("a report from before the reply: left as stored", Bool.show(String.eq(H.Lr.whole(st(), id(), legacy(), 2n), legacy())), "True")
    C.Check.str("the hint names another thread: left as stored", Bool.show(String.eq(H.Lr.whole(st(), id(), head() ++ String.take(reply(), H.Lr.cut()) ++ H.Lr.cut.tail("t9"), 9n),
      head() ++ String.take(reply(), H.Lr.cut()) ++ H.Lr.cut.tail("t9"))), "True")
    C.Check.str("the view reads the legacy row whole", Bool.show(String.eq(T.Entry.text(T.Row.entry(row(read()))), head() ++ reply())), "True")
    C.Check.str("one read completes it", Bool.show(J.Json.get_bool(J.Json.get_obj(result(), "message"), "complete", False{})), "True")
    C.Check.str("the read returns the whole text", Bool.show(String.eq(J.Json.get_str(J.Json.get_obj(result(), "message"), "text", ""), head() ++ reply())), "True")
    C.Check.str("the thread's window shows the legacy row whole, with no read-more", Bool.show(window.whole(M.State.entries.all(V.State.window(M.State.apply(st(), M.MessagePosted{"t0", id(), M.User{}, legacy(), 9n}), index(), target(), ["1"])))), "True")
    C.Check.str("the window marks no message partial", Nat.show(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(V.Page.window(index(), "t0", J.Json.obj([J.KV{"target", target()}]), "epoch", ["1"]), "partial")))), "0")
    IO.print("fixture " ++ fixture())
