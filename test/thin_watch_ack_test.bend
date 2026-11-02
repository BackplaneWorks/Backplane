import Base
import ./check.bend as C
import ./thin_terminal_input_test.bend as F
import ../src/server/thin-actor.bend as A
import ../src/server/thin-view.bend as V
import ../src/core/client.bend as U
import ../src/core/json.bend as J

# A viewer's kicad.watch ACK (sent at each history step) needs no second
# full render: only a failed one, one with a result, or one that would
# clear an error on show marks the view dirty. Other replies are unchanged.

def actor(+u: U.Ui) -> A.Actor:
  A.Actor.make(u, "v", V.Target.json("thread", "a"))

def sent(+u: U.Ui, +method: String) -> A.Actor:
  +cmd = J.Json.obj([J.KV{"type", J.Str{"send"}}, J.KV{"text", J.Str{"{\"id\":101,\"m\":\"" ++ method ++ "\",\"p\":{\"kind\":\"board\"}}"}}])
  A.Actor.accept(actor(u), V.ActResult{u, [cmd]}, False{})

def answered(+u: U.Ui, +method: String, +reply: String) -> A.Actor:
  A.Actor.message(sent(u, method), True{}, "h", F.obj(reply))

def dirty(+u: U.Ui, +method: String, +reply: String) -> String:
  Bool.show(A.Actor.dirty(answered(u, method, reply)))

def ok() -> String: "{\"t\":\"reply\",\"id\":101,\"ok\":true}"

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("a successful watch ACK does not redraw", dirty(F.ui(), "kicad.watch", ok()), "False")
    C.Check.str("a failed watch redraws", dirty(F.ui(), "kicad.watch", "{\"t\":\"reply\",\"id\":101,\"ok\":false,\"error\":\"no board\"}"), "True")
    C.Check.str("a watch answer with a result redraws", dirty(F.ui(), "kicad.watch", "{\"t\":\"reply\",\"id\":101,\"ok\":true,\"result\":{\"kicad\":\"missing\"}}"), "True")
    C.Check.str("a watch ACK that clears an error on show redraws", dirty(U.Ui.error_set(F.ui(), "an earlier failure"), "kicad.watch", ok()), "True")
    C.Check.str("another method's ACK still redraws", dirty(F.ui(), "files.dir", ok()), "True")
