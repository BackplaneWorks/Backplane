import Base
import ./check.bend as C
import ../src/core/improvements.bend as I
import ../src/core/hub.bend as H
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/codec.bend as K
import ../src/core/attach.bend as A
import ../src/core/accounts.bend as AC

# Test the production planner, not a stand-in thread implementation.
def key() -> String: "0123456789abcdef0123456789abcdef"
def thread() -> String: I.Improvements.thread(key())
def model(t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, m, _, _, _, _}: m
def base() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "Here", "/tmp/project", 1n}, M.ProjectCreated{"q", "Other", "/tmp/other", 1n},
    M.SettingSet{"agent.provider", "codex", 2n}, M.SettingSet{"agent.model", "gpt-6.1-sol", 2n}], M.State.empty())
def create(+s: M.State, +p: String, key: String) -> H.Step:
  H.Hub.thread_create(s, 3n, 1, "9", J.Json.obj([J.KV{"project", J.Str{p}}, J.KV{"creationKey", J.Str{key}}]))
def created() -> M.State: M.State.replay(H.Queue.changes(create(base(), "p", key())), base())
def ready() -> M.State: M.State.apply(created(), M.WorktreeSet{thread(), "feature", "/tmp/isolated", 4n})
def binding() -> String: "{\"id\":\"work\",\"epoch\":2}"
def named() -> M.State: M.State.apply(created(), M.SettingSet{AC.Accounts.binding.key(thread(), "codex"), binding(), 4n})
def start_request(owner: String) -> J.Json:
  J.Json.obj([J.KV{"m", J.Str{"turn.start"}}, J.KV{"p", J.Json.obj([J.KV{"thread", J.Str{thread()}}, J.KV{"accountBinding", J.Str{owner}}])}])
def tags(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}: ""
    case c <> t: J.Json.get_str(K.Change.encode(c), "$", "") ++ " " ++ tags(t)
def reply(os: List<&2, H.Out>) -> String:
  match os:
    case Nil{}: ""
    case H.Send{_, text} <> _: text
    case _ <> t: reply(t)
def made_reply(step: H.Step) -> J.Json: Maybe.default(&2, J.Json, J.Json.parse(reply(H.Step.outs_of(step))), J.Null{})
def count(step: H.Step) -> String: Nat.show(List.length(&2, M.Change, H.Queue.changes(step)))
def text() -> String: "  improve this\n日本語 & <literal>  "
def body() -> String: A.Att.join(text(), ["/tmp/attachments/app.png"])
def first() -> H.Step: H.Hub.turn(ready(), 5n, 1, "10", thread(), body(), "improvement-request-fixed")
def sent() -> M.State: M.State.replay(H.Queue.changes(first()), ready())
def message(es: List<&2, M.Entry>) -> String:
  match es:
    case Nil{}: ""
    case M.Msg{_, _, M.User{}, text, _} <> _: text
    case _ <> t: message(t)
def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("valid key", Bool.show(I.Improvements.key(key())), "True")
    C.Check.str("invalid key", Bool.show(I.Improvements.key("../else")), "False")
    C.Check.str("one thread created", count(create(base(), "p", key())), "1")
    C.Check.str("stable thread id", J.Json.get_str(made_reply(create(base(), "p", key())), "thread", ""), thread())
    C.Check.str("normal provider default", M.Thread.provider(H.Hub.thread(created(), thread())), "codex")
    C.Check.str("normal model default", model(H.Hub.thread(created(), thread())), "gpt-6.1-sol")
    C.Check.str("normal isolation", Bool.show(M.Worktree.unmade(H.Hub.thread(created(), thread()))), "True")
    C.Check.str("lost create reply retry no mutation", count(create(created(), "p", key())), "0")
    C.Check.str("same project retry returns same thread", J.Json.get_str(made_reply(create(created(), "p", key())), "thread", ""), thread())
    C.Check.str("cannot reuse key for another project", Bool.show(J.Json.get_bool(made_reply(create(created(), "q", key())), "ok", True{})), "False")
    C.Check.str("unknown project rejected", count(create(base(), "missing", key())), "0")
    C.Check.str("context reads normal named account generation", J.Json.get_str(H.Improvements.context(named(), thread()), "accountBinding", ""), binding())
    C.Check.str("ordinary account fence accepts exact generation", Bool.show(AC.Accounts.request.current(named(), start_request(binding()), "")), "True")
    C.Check.str("ordinary account fence rejects missing generation", Bool.show(AC.Accounts.request.current(named(), start_request(""), "")), "False")
    C.Check.str("exact request and attachment kept", message(H.Hub.entries(sent())), body())
    C.Check.str("normal image extraction", String.join(A.Att.images(message(H.Hub.entries(sent()))), ""), "/tmp/attachments/app.png")
    C.Check.str("lost start reply retry no mutation", count(H.Hub.turn(sent(), 6n, 1, "11", thread(), body(), "improvement-request-fixed")), "0")
    return Unit{}
