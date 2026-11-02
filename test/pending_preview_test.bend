import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/json.bend as J

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}: u

def sent(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}: ""
    case U.Send{x} <> _: J.Json.get_str(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(x), J.Null{}), "p"), "text", "")
    case _ <> tail: sent(tail)

def outgoing(a: U.Act) -> String:
  match a:
    case U.Act{_, cs}: sent(cs)

def main() -> IO(Unit):
  +text = String.repeat("Pasted research article. 日本語\n", 1500n)
  +initial = ui(U.Ui.select(U.Ui.init(0n), "T"))
  +sending = U.Ui.act(U.Ui.scratch_set(initial, "T", text), "send", "")
  +pending = ui(sending)
  +small = U.Ui.info_set(pending, "local.body.budget", "1")
  do IO<Unit>:
    C.Check.str("normal pending bubble retains full text", String.join(U.Ui.pending(pending, "T"), ""), String.trim(text))
    C.Check.str("fitted pending bubble is a prefix with ellipsis", String.join(U.Ui.pending(small, "T"), ""), String.take(text, 1024n) ++ "…")
    C.Check.str("short pending text stays whole", U.Pending.preview("hello 日本語"), "hello 日本語")
    C.Check.str("exact preview boundary stays whole", U.Pending.preview(String.repeat("x", 1024n)), String.repeat("x", 1024n))
    C.Check.str("fitting preserves the complete Send payload", outgoing(sending), String.trim(text))
    C.Check.str("fitting preserves the reliable outbox", sent(U.Ui.outbox(small)), String.trim(text))
    C.Check.str("another thread has no pending bubble", String.join(U.Ui.pending(small, "other"), ""), "")
