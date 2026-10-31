import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/live.bend as L
import ../src/core/live-checkpoint.bend as K
import ../src/core/steer.bend as S

# Exercise the real stop projection after restoring a buffer; recovery uses
# the same projection, so this checks the text becomes durable exactly once.
def state(extra: List<&2, M.Change>) -> M.State:
  M.State.replay(List.append(&2, M.Change, [M.ThreadCreated{"t", "", "Test", M.Local{}, "", "", "claude", "", 1n},
    M.MessagePosted{"t", "q", M.User{}, "question", 2n}, M.TurnChanged{"t", M.Running{}, 2n}], extra), M.State.empty())

def buffers() -> List<&2, L.Buf>:
  L.Live.add.at(Nil{}, "t", "日本語 reply\nline 8\n", 123n)

def record() -> J.Json:
  K.Checkpoint.one(buffers(), state(Nil{}), "t")

def restored(+s: M.State) -> List<&2, L.Buf>:
  K.Checkpoint.restore.items([record()], s)

def posted(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}: ""
    case M.MessagePosted{_, _, M.Assistant{}, text, _} <> tail: text ++ posted(tail)
    case _ <> tail: posted(tail)

def stop(+s: M.State) -> List<&2, M.Change>:
  S.Steer.all([M.TurnChanged{"t", M.Interrupted{}, 4n}], s, restored(s))

def names(ts: List<&2, String>) -> String:
  String.join(ts, ",")

def dirty(cs: List<&2, M.Change>) -> String:
  J.Json.show(J.Json.arr(K.Checkpoint.dirty(cs, buffers(), state(Nil{}))))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("completed empty buffers are omitted from disk", Nat.show(List.length(&2, J.Json, K.Checkpoint.records(L.Live.end(buffers(), "t"), state(Nil{})))), "0")
    C.Check.str("stream text restores exactly", S.Steer.text(restored(state(Nil{})), "t"), "日本語 reply\nline 8\n")
    C.Check.str("serialized checkpoint restores", S.Steer.text(K.Checkpoint.restore(state(Nil{}), J.Json.show(J.Json.arr([record()]))), "t"), "日本語 reply\nline 8\n")
    C.Check.str("stream generation restores", Nat.show(S.Steer.gen(restored(state(Nil{})), "t")), "123")
    C.Check.str("restart's interruption posts the prefix", posted(stop(state(Nil{}))), "日本語 reply\nline 8\n")
    C.Check.str("a committed interruption cannot restore it twice", Nat.show(List.length(&2, L.Buf, restored(M.State.replay(stop(state(Nil{})), state(Nil{}))))), "0")
    C.Check.str("a completed reply makes its checkpoint stale", Nat.show(List.length(&2, L.Buf, restored(state([M.MessagePosted{"t", "answer", M.Assistant{}, "whole reply", 3n}])))), "0")
    C.Check.str("a new question makes its checkpoint stale", Nat.show(List.length(&2, L.Buf, restored(state([M.MessagePosted{"t", "next", M.User{}, "next", 3n}])))), "0")
    C.Check.str("a stopped turn is never restored", Nat.show(List.length(&2, L.Buf, restored(state([M.TurnChanged{"t", M.Interrupted{}, 3n}])))), "0")
    C.Check.str("a failed turn is never restored", Nat.show(List.length(&2, L.Buf, restored(state([M.TurnChanged{"t", M.Failed{}, 3n}])))), "0")
    C.Check.str("unrelated tool activity leaves the stream recoverable", S.Steer.text(restored(state([M.ActivityLogged{"t", "tool", "tool", "Bash", "working", 3n}])), "t"), "日本語 reply\nline 8\n")
    C.Check.str("deltas elsewhere: an unrelated change rewrites nothing", names(K.Checkpoint.touched([M.ActivityLogged{"t", "tool", "tool", "Bash", "working", 3n}])), "")
    C.Check.str("each touched thread once, in order", names(K.Checkpoint.touched([M.MessagePosted{"t", "q2", M.User{}, "x", 3n}, M.TurnChanged{"u", M.Running{}, 3n}, M.TurnChanged{"t", M.Completed{}, 4n}])), "t,u")
    C.Check.str("a touched stream is written whole with its anchor", Bool.show(String.contains(dirty([M.MessagePosted{"t", "q2", M.User{}, "x", 3n}]), "\"text\":\"日本語 reply\\nline 8\\n\"")), "True")
    C.Check.str("a touched thread with no buffer is removed", dirty([M.TurnChanged{"u", M.Completed{}, 3n}]), "[{\"anchor\":\"\",\"buffer\":{\"thread\":\"u\",\"text\":\"\"}}]")
    C.Check.str("the restored threads are named for the journals kept", J.Json.show(J.Json.arr(K.Checkpoint.threads(restored(state(Nil{}))))), "[\"t\"]")
