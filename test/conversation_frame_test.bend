import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/thin.bend as T
import ../src/core/client.bend as U
import ../src/server/conversation-frame.bend as F
import ../src/server/thin-body.bend as B
import ../src/server/thin-view.bend as V
import ../src/core/thin-live.bend as L

def repeat(n: Nat, +s: String) -> String:
  match n:
    case 0n: ""
    case 1n+k: s ++ repeat(k, s)

def state() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "P", "/tmp/window", 1n}, M.ThreadCreated{"t", "p", "T", M.Local{}, "", "", "codex", "", 1n}], M.State.empty())

def main() -> IO(Unit):
  do IO<Unit>:
    +source : String = "Real source entry 日本語 😀\n" ++ repeat(400n, "😀\\\"\u{1}\n")
    +index : T.Index = T.Index.post(T.Index.empty(), M.Msg{"message", "t", M.Assistant{}, source, 1n})
    +p : J.Json = J.Json.obj([J.KV{"target", V.Target.json("thread", "t")}, J.KV{"bodyPages", J.Num{"1"}}, J.KV{"bodyBudget", J.Num{"1"}}])
    +ids : List<&2, String> = B.Body.window.bounded(["1"], index, p)
    +u : U.Ui = U.Ui.select.k(U.Ui.adopt.raw(U.Ui.init(1n), state()), "t")
    +window : J.Json = V.Render.window(L.Stream.blank(), index, "t", p, "epoch", ids)
    +shown : J.Json = F.Frame.shown(u, index, ids, window)
    C.Check.str("nonempty source always retains a row", Nat.show(List.length(&2, String, ids)), "1")
    C.Check.str("compact frame contains actual source", Bool.show(String.contains(J.Json.show(shown), "Real source entry")), "True")
    C.Check.str("encoded Unicode preview fits budget", Bool.show(Nat.is_le(J.Json.bytes(J.Str{F.Frame.text(source, 3072n)}), 3074n)), "True")
    C.Check.str("single source frame below transport budget", Bool.show(Nat.is_le(J.Json.bytes(F.Control.envelope(J.Num{"140737488355329"}, J.Json.obj([J.KV{"shown", shown}, J.KV{"window", window}]))), 262144n)), "True")
    C.Check.str("source continuation remains available", Bool.show(B.Body.large(T.Index.row(index, 1n), p)), "True")
    +part : F.Part = F.Part.take(source, 32768n)
    C.Check.str("control slicing is lossless", F.Part.text(part) ++ F.Part.rest(part), source)
    C.Check.str("control payload fits encoded budget", Bool.show(Nat.is_le(J.Json.bytes(J.Str{F.Part.text(part)}), 32770n)), "True")
