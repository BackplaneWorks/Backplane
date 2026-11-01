import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/i18n.bend as I
import ../src/mobile/session.bend as S

# The phones' composer for the draft typed now (session.bend's Composer.*):
# what a key changes, without a screen.

def thread(+draft: String, +skills: J.Json, +state: String, +claude: Bool) -> J.Json:
  J.Json.obj([J.KV{"draft", J.Str{draft}}, J.KV{"skills", skills}, J.KV{"state", J.Str{state}}, J.KV{"attaching", J.Arr{J.End{}}}, J.KV{"claude", J.Flag{claude}}, J.KV{"send", J.Str{"Send"}}, J.KV{"sendAct", J.Str{"send"}}])

def hub() -> J.Json:
  J.Arr{J.Item{J.Json.obj([J.KV{"name", J.Str{"$plot"}}, J.KV{"desc", J.Str{"d"}}, J.KV{"on", J.Flag{True{}}}]), J.End{}}}

def names(+t: J.Json, +draft: String) -> String:
  String.join(S.Composer.names(J.Json.elems(J.Json.get_obj(S.Composer.of(t, draft, I.En{}), "skills"))), " ")

def screen(+t: J.Json, +draft: String) -> J.Json:
  J.Json.get_obj(J.Json.get_obj(S.Session.shown.draft(J.Json.obj([J.KV{"j", J.Json.obj([J.KV{"thread", t}])}]),
    J.Json.obj([J.KV{"sel", J.Str{"t"}}, J.KV{"drafts", J.Json.obj([J.KV{"t", J.Str{draft}}])}]), I.En{}), "j"), "thread")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("a slash draft offers Claude's commands at once", names(thread("", J.Arr{J.End{}}, "idle", True{}), "/co"), "/compact /context")
    C.Check.str("another agent's thread offers Backplane's own", names(thread("", J.Arr{J.End{}}, "idle", False{}), "/"), "/btw /give /secret")
    C.Check.str("the hub's rows stay for the draft it drew", names(thread("$pl", hub(), "idle", True{}), "$pl"), "$plot")
    C.Check.str("the hub's rows stay while neither draft is a slash command", names(thread("$pl", hub(), "idle", True{}), "$plo"), "$plot")
    C.Check.str("a slash draft the hub has not drawn takes its own commands", names(thread("/co", hub(), "idle", True{}), "/con"), "/context")
    C.Check.str("a draft that stops being a command offers nothing local", names(thread("/co", J.Arr{J.End{}}, "idle", True{}), "/co x"), "")
    C.Check.str("running and blank: Stop", J.Json.get_str(S.Composer.of(thread("", J.Arr{J.End{}}, "run", True{}), " ", I.En{}), "sendAct", ""), "interrupt")
    C.Check.str("running and typed: send", J.Json.get_str(S.Composer.of(thread("", J.Arr{J.End{}}, "run", True{}), "go on", I.En{}), "sendAct", ""), "send")
    C.Check.str("idle and blank: send", J.Json.get_str(S.Composer.of(thread("", J.Arr{J.End{}}, "idle", True{}), "", I.En{}), "sendAct", ""), "send")
    C.Check.str("the button's label follows /btw", J.Json.get_str(S.Composer.of(thread("", J.Arr{J.End{}}, "idle", True{}), "/btw why", I.En{}), "send", ""), "Ask")
    C.Check.str("a full screen's composer is the per-key patch",
      J.Json.show(screen(thread("$pl", hub(), "run", True{}), "/c")),
      J.Json.show(J.Json.obj([J.KV{"draft", J.Str{"/c"}}, J.KV{"skills", J.Json.get_obj(S.Composer.of(thread("$pl", hub(), "run", True{}), "/c", I.En{}), "skills")},
        J.KV{"state", J.Str{"run"}}, J.KV{"attaching", J.Arr{J.End{}}}, J.KV{"claude", J.Flag{True{}}},
        J.KV{"send", J.Str{"Steer"}}, J.KV{"sendAct", J.Str{"send"}}])))
    C.Check.str("no thread stays no thread", J.Json.show(screen(J.Null{}, "x")), "null")
