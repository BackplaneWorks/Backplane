import Base
import ./check.bend as C
import ../src/web/view.bend as V
import ../src/web/dom.bend as D
import ../src/core/i18n.bend as I
import ../src/core/msgs.bend as MSG

def title.attrs(attrs: List<&2, D.Attr>) -> String:
  match attrs:
    case Nil{}: ""
    case D.At{+key, value} <> tail:
      Bool.pick(String, String.eq(key, "title"), value, title.attrs(tail))
    case _ <> tail: title.attrs(tail)

def title(node: D.Node) -> String:
  match node:
    case D.El{_, _, attrs, _}: title.attrs(attrs)
    case _: ""

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("Send names the supported keys", title(V.View.send.loc(I.En{}, "send", MSG.T.composer.send(I.En{}))), "Enter or Ctrl+Enter to send")
    C.Check.str("Steer promises no queue", title(V.View.send.loc(I.En{}, "send", MSG.T.composer.steer(I.En{}))), "Enter or Ctrl+Enter to send")
    C.Check.str("Japanese Steer names the supported keys", title(V.View.send.loc(I.Ja{}, "send", MSG.T.composer.steer(I.Ja{}))), "Enter または Ctrl+Enterで送信")
    C.Check.str("Save keeps its own keys", title(V.View.send.loc(I.En{}, "send", MSG.T.common.save(I.En{}))), MSG.T.intent.save.tip(I.En{}))
    C.Check.str("Stop keeps its own hint", title(V.View.send.loc(I.En{}, "interrupt", MSG.T.composer.stop(I.En{}))), MSG.T.composer.stop.tip(I.En{}))
