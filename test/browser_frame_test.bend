import Base
import ./check.bend as C
import ../src/core/browser-frame.bend as B
import ../src/server/thin-actor.bend as A
import ../src/core/json.bend as J

def shot() -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"shot"}}, J.KV{"bot", J.Str{"t"}}, J.KV{"n", J.Json.nat(7n)}])

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("ordinary thread input names its page", B.BrowserFrame.owner("", "t"), "t")
    C.Check.str("bot input keeps its owner", B.BrowserFrame.owner("bot", "t"), "bot")
    C.Check.str("selected page accepts its frame", Bool.show(B.BrowserFrame.owns("t", "t")), "True")
    C.Check.str("other page cannot replace the surface", Bool.show(B.BrowserFrame.owns("t", "other")), "False")
    C.Check.str("hidden page cannot accept a frame", Bool.show(B.BrowserFrame.owns("", "t")), "False")
    C.Check.str("browser frame stays off thin wire", Bool.show(A.Message.on_wire(shot())), "False")
    C.Check.str("cast renewal is queued for delivery", Bool.show(A.Actor.read_only(J.Json.obj([J.KV{"type", J.Str{"send"}}, J.KV{"text", J.Str{"{\"m\":\"browser.seen\"}"}}]))), "True")
