import Base
import ./check.bend as C
import ../src/core/hub.bend as H
import ../src/core/mail.bend as ML
import ../src/core/mailbody.bend as MB
import ../src/core/model.bend as M
import ../src/core/bothub.bend as BH
import ../src/core/bot.bend as BT
import ../src/core/mcp.bend as MC
import ../src/core/json.bend as J
import ../src/core/codec.bend as CD
import ../src/core/tz.bend as TZ

def base() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "Project", "/tmp/mailfile", 1n},
    M.ThreadCreated{"a", "p", "Worker", M.Local{}, "", "", "codex", "", 1n},
    M.ThreadCreated{"b", "p", "Beta", M.Local{}, "", "", "codex", "", 1n},
    M.ThreadCreated{"c", "p", "Gamma", M.Local{}, "", "", "codex", "", 1n},
    M.ThreadCreated{"d", "p", "Delta", M.Local{}, "", "", "codex", "", 1n},
    M.BotSet{"bb", "Beta", "b", 0n, "", 1n}, M.BotSet{"bc", "Gamma", "c", 0n, "", 1n}, M.BotSet{"bd", "Delta", "d", 0n, "", 1n}], M.State.empty())

def enabled() -> M.State:
  M.State.apply(base(), M.SettingSet{"mail.on", "on", 1n})

def disabled() -> M.State:
  M.State.apply(base(), M.SettingSet{"mail.on", "off", 1n})

def outs(st: H.Step) -> List<&2, H.Out>:
  match st:
    case H.Step{_, +os}: os

def appends(+os: List<&2, H.Out>) -> Nat:
  match os:
    case Nil{}: 0n
    case H.MailAppend{_, _, _} <> +t: 1n+appends(t)
    case _ <> +t: appends(t)

def first(+os: List<&2, H.Out>) -> Maybe<&2, MB.Body>:
  match os:
    case Nil{}: None{}
    case H.MailAppend{+line, _, _} <> _: MB.Body.parse(line)
    case _ <> +t: first(t)

def signature(body: Maybe<&2, MB.Body>) -> String:
  match body:
    case None{}: "none"
    case Some{+b}: ML.Mail.conv(MB.Body.mail(b)) ++ "/" ++ ML.Mail.kind(MB.Body.mail(b)) ++ "/" ++ ML.Mail.wanted(MB.Body.mail(b))

def pending(+os: List<&2, H.Out>) -> List<&2, M.Change>:
  match os:
    case Nil{}: Nil{}
    case H.MailAppend{_, _, +cs} <> +t: List.append(&2, M.Change, cs, pending(t))
    case _ <> +t: pending(t)

def changes(+cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}: ""
    case +h <> +t: J.Json.show(CD.Change.encode(h)) ++ "\n" ++ changes(t)

def posts(+cs: List<&2, M.Change>) -> Nat:
  match cs:
    case Nil{}: 0n
    case M.MessagePosted{_, _, _, _, _} <> +t: 1n+posts(t)
    case _ <> +t: posts(t)

def source(+name: String, +from: String, +msg: String, +text: String, +want: String, +kind: String) -> IO(Unit):
  do IO<Unit>:
    +on : H.Step = H.Hub.inbox.from(enabled(), 10n, from, "b", M.User{}, text, msg)
    +off : H.Step = H.Hub.inbox.from(disabled(), 10n, from, "b", M.User{}, text, msg)
    +o : ML.Origin = ML.Mail.classify(M.Link.from(msg), msg, False{}, text)
    C.Check.str(name ++ " source kind", ML.Origin.kind(o), kind)
    C.Check.str(name ++ " filed once", Nat.show(appends(outs(on))), "1")
    C.Check.str(name ++ " route/kind/want", signature(first(outs(on))), ML.Mail.route(o, "bot:bb") ++ "/" ++ ML.Origin.kind(o) ++ "/" ++ want)
    C.Check.str(name ++ " off no mail", Nat.show(appends(outs(off))), "0")
    C.Check.str(name ++ " bot FOH clean", Nat.show(posts(H.Queue.changes(on))), "0")
    C.Check.str(name ++ " legacy delivery retained off", changes(H.Queue.changes(off)), changes(H.Queue.changes(H.Origin.step(H.Hub.inbox.id(disabled(), 10n, from, "b", M.User{}, text, msg), "b", msg, o, 10n))))

def scope_bot(+s: M.State, +thread: String, +want: String) -> String:
  J.Json.get_str(MC.Mcp.mail.params(s, thread, J.Json.obj([J.KV{"bot", J.Str{want}}])), "bot", "")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("a bot cannot read another mailroom", scope_bot(enabled(), "b", "*"), "bb")
    C.Check.str("a bot cannot name another bot", scope_bot(enabled(), "b", "bc"), "bb")
    C.Check.str("the uberbot may read every mailroom", scope_bot(M.State.apply(enabled(), M.SettingSet{"uber.bot", "bb", 2n}), "b", "*"), "*")
    C.Check.str("a worker thread sees only its own", scope_bot(enabled(), "a", "*"), "thread:a")
    source("thread_send", "a", "t:a:m", "hello", "act", "thread")
    source("launch report", "a", "t:a:lr-completed", "[Backplane] Launched thread completed", "fyi", "launch")
    source("wake", "a", "t:a:lr-wake", "Wake watch: ready", "act", "wake")
    source("lost work", "backplane", "t:backplane:l1", "Lost tool result", "act", "lost")
    source("task result", "a", "t:a:td-job", "[Backplane] Delegated task finished", "act", "task")
    source("deploy", "deploy", "t:deploy:m", "Deployed", "act", "deploy")
    source("lock grant", "lock:build", "t:lock:build:m", "Granted", "act", "lock")
    source("bot_send", "a", "t:a:bot", "[message from Alpha]\nhi", "act", "thread")
    source("routine funnel", "", "m-routine", "[routine: daily]\ncheck", "act", "routine")
    source("webhook funnel", "", "m-hook", "[webhook: push]\nbody", "act", "hook")
    +on : M.State = enabled()
    C.Check.str("queued thread_send filed", Nat.show(appends(outs(MC.Mcp.send.m(False{}, on, 10n, "a", "b", "queued", "queue")))), "1")
    C.Check.str("real launch report files", Nat.show(appends(outs(H.Lr.send(on, 10n, "a", "b", "[Backplane] Launched thread completed")))), "1")
    C.Check.str("real lost work files", Nat.show(appends(outs(H.Lost.note(on, 10n, "b", "lost result")))), "1")
    C.Check.str("real task result files", Nat.show(appends(outs(H.Task.finish.go(True{}, on, 10n, "task", "a", "b", "codex", "", "finished", M.TaskDone{}, "result", Nil{}, Nil{})))), "1")
    +room : BT.Room = BT.Room{"r", "Room", ["bb", "bc", "bd"], 1n, False{}}
    +post : H.Step = BH.Bots.post.id("room-msg", on, 10n, "a", "Worker", "", room, "Room message", 0n)
    C.Check.str("room files once for 3 members", Nat.show(appends(outs(post))), "1")
    C.Check.str("room FYI has no member delivery", Nat.show(posts(H.Queue.changes(post))), "0")
    C.Check.str("room route and fyi", signature(first(outs(post))), "room:r/room/fyi")
    +remote : H.Step = BH.Bots.incoming.go(on, 10n, BT.World.bot(M.State.bots(on), "bb"), "Remote@machine", "", "remote", 0n)
    C.Check.str("remote incoming filed once", Nat.show(appends(outs(remote))), "1")
    C.Check.str("remote peer kind", signature(first(outs(remote))), "peer:Remote@machine:dm:Beta:Remote@machine/peer/act")
    +carry : H.Step = H.Carry.k(True{}, on, 10n, "b")
    C.Check.str("carry filed once", Nat.show(appends(outs(carry))), "1")
    C.Check.str("carry route/kind", signature(first(outs(carry))), "system:bb/carry/fyi")
    +routine : H.Step = BH.Bots.fire.prompt(on, 10n, 0n, BT.Routine{"daily", "bb", "Daily", "@daily", "check", True{}, 0n, 1n, False{}, 0n, "", "routine", False{}, 0n})
    C.Check.str("routine uses stable id", signature(first(outs(routine))), "routine:daily/routine/act")
    +hook : H.Step = BH.Bots.hooked.go(True{}, on, 10n, "push", "bb", "Push", "payload")
    C.Check.str("webhook uses stable id", signature(first(outs(hook))), "hook:push/hook/act")
    +file : H.Step = H.Mail.file(on, 10n, ML.Origin{"thread:a", "thread", ""}, "bot:bb", "hé🙂", "same")
    +filed : M.State = M.State.replay(pending(outs(file)), on)
    +twice : M.State = M.State.replay(pending(outs(file)), filed)
    C.Check.str("idempotent header fold", Nat.show(ML.Conv.n(ML.Box.conv(M.State.mail(twice), "pair:bot:bb|thread:a"))), "1")
    C.Check.str("UTF8 byte size", Nat.show(H.Mail.bytes("hé🙂", 0n)), "7")
    +p : J.Json = J.Json.obj([J.KV{"bot", J.Str{"bb"}}, J.KV{"n", J.Num{"50"}}])
    +listing : J.Json = H.Mail.list(filed, p)
    C.Check.str("list total", Nat.show(J.Json.get_nat(listing, "total", 0n)), "1")
    +row : J.Json = Maybe.default(&2, J.Json, List.head(&2, J.Json, J.Json.elems(J.Json.get_obj(listing, "convs"))), J.Null{})
    C.Check.str("list row head/from", J.Json.get_str(row, "head", "") ++ "/" ++ J.Json.get_str(row, "from", ""), "hé🙂/thread:a")
    C.Check.str("list unread", Bool.show(J.Json.get_bool(row, "unread", False{})), "True")
    C.Check.str("search header", Nat.show(J.Json.get_nat(H.Mail.find(filed, J.Json.obj([J.KV{"bot", J.Str{"bb"}}, J.KV{"q", J.Str{"hé"}}])), "total", 0n)), "1")
    +marked : M.State = M.State.replay(H.Mail.mark(filed, 11n, J.Json.obj([J.KV{"bot", J.Str{"bb"}}, J.KV{"conv", J.Str{"pair:bot:bb|thread:a"}}, J.KV{"status", J.Str{"done"}}])), filed)
    C.Check.str("mark status", ML.Mark.status(ML.Box.mark_of(M.State.mail(marked), "bb", "pair:bot:bb|thread:a")), "done")
    +tagged : M.State = M.State.replay(H.Mail.tag(11n, J.Json.obj([J.KV{"bot", J.Str{"bb"}}, J.KV{"conv", J.Str{"pair:bot:bb|thread:a"}}, J.KV{"tags", J.Json.arr([J.Str{"red"}, J.Str{"work"}])}])), marked)
    C.Check.str("tag arrays preserved", String.join(ML.Mark.tags(ML.Box.mark_of(M.State.mail(tagged), "bb", "pair:bot:bb|thread:a")), ","), "red,work")
    +snoozed : M.State = M.State.replay([M.MailMarked{"bb", "pair:bot:bb|thread:a", "snoozed", 10n, 20n, 12n}, M.MailMarked{"bb", "future", "snoozed", 0n, 100n, 12n}], tagged)
    +due : M.State = M.State.replay(H.Mail.due(snoozed, 20n), snoozed)
    C.Check.str("snooze due only", ML.Mark.status(ML.Box.mark_of(M.State.mail(due), "bb", "pair:bot:bb|thread:a")) ++ "/" ++ ML.Mark.status(ML.Box.mark_of(M.State.mail(due), "bb", "future")), "unread/snoozed")
    C.Check.str("snooze tick idempotent", Nat.show(List.length(&2, M.Change, H.Mail.due(due, 20n))), "0")
    +offroom : H.Step = BH.Bots.post.id("room-msg", disabled(), 10n, "a", "Worker", "", room, "Room message", 0n)
    C.Check.str("room off no filing", Nat.show(appends(outs(offroom))), "0")
    C.Check.str("room on no FOH posts", Nat.show(posts(H.Queue.changes(post))), "0")
    C.Check.str("room off still delivers", Bool.show(Nat.is_gt(posts(H.Queue.changes(offroom)), 0n)), "True")
    C.Check.str("MCP definitions include seven mail tools", Nat.show(List.length(&2, J.Json, MC.Mcp.mail.tools())), "7")
    C.Check.str("mail RPC methods appended", Nat.show(CD.Codec.pos(H.Hub.methods(), "mail.list")), "40")
    C.Check.str("MCP tool names appended", Nat.show(CD.Codec.pos(MC.Mcp.names(), "mail_list")), "35")
    C.Check.str("nonbot scope hides other mail", Nat.show(J.Json.get_nat(H.Mail.list(filed, J.Json.obj([J.KV{"bot", J.Str{"thread:d"}}])), "total", 99n)), "0")
