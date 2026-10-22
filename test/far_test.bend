import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/codec.bend as K
import ../src/core/json.bend as J
import ../src/core/client.bend as U
import ../src/core/botui.bend as BU
import ../src/core/bot.bend as BT
import ../src/core/far.bend as FR
import ../src/mobile/hubs.bend as H
import ../src/mobile/screen.bend as MS

# Threads elsewhere (far.bend): a hub's clients fold a linked machine's
# bots and threads in under ids of their own, once, in order, and send
# what they do to them back to that machine.

def items(cs: List<&2, M.Change>) -> List<&2, J.Json>:
  match cs:
    case Nil{}:
      Nil{}
    case c <> t:
      K.Change.encode(c) <> items(t)

def log(+since: Nat, cs: List<&2, M.Change>) -> J.Json:
  J.Json.obj([J.KV{"t", J.Json.str("log")}, J.KV{"since", J.Json.nat(since)}, J.KV{"items", J.Json.arr(items(cs))}])

def info(+remote: String) -> J.Json:
  J.Json.obj([J.KV{"t", J.Json.str("info")}, J.KV{"info", J.Json.obj([J.KV{"bots.remote", J.Json.str(remote)}])}])

# items numbered from n, as the owning hub numbers its log
def numbered(cs: List<&2, M.Change>, +n: Nat) -> List<&2, FR.Item>:
  match cs:
    case Nil{}:
      Nil{}
    case c <> t:
      FR.Item{n, K.Change.wire(c)} <> numbered(t, 1n+n)

def far(+since: Nat, +head: Nat, away: Bool, xs: List<&2, FR.Item>) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(FR.Fr.msg("p1", "box", away, since, head, xs)), J.Null{})

# box's Kit: its thread (no project), a message, a turn that ran
def box() -> List<&2, FR.Item>:
  numbered([M.ThreadCreated{"bt1", "", "Kit", M.Local{}, "", "", "claude", "", 100n},
    M.BotSet{"b1", "Kit", "bt1", 7n, "helps", 100n},
    M.MessagePosted{"bt1", "m1", M.User{}, "hello", 101n},
    M.TurnChanged{"bt1", M.Running{}, 102n}], 11n)

def more() -> List<&2, FR.Item>:
  numbered([M.MessagePosted{"bt1", "m2", M.Assistant{}, "hi there", 103n}, M.TurnChanged{"bt1", M.Completed{}, 104n}], 20n)

def here() -> List<&2, M.Change>:
  [M.PeerSet{"p1", "box", "http://box:3787", False{}, 50n},
   M.ThreadCreated{"t1", "", "Pip", M.Local{}, "", "", "claude", "", 90n}, M.BotSet{"b9", "Pip", "t1", 3n, "", 90n}]

def recv(+u: U.Ui, +o: J.Json) -> U.Ui:
  U.Ui.recv(u, o)

def joined() -> U.Ui:
  recv(recv(recv(U.Ui.init(1000n), log(0n, here())), info("[{\"name\":\"Kit\",\"peer\":\"p1\",\"peerName\":\"box\",\"look\":7,\"state\":\"working\"}]")),
    far(0n, 14n, False{}, box()))

def title(t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, ti, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      ti

def count(+u: U.Ui, +th: String) -> String:
  Nat.show(List.length(&2, M.Entry, M.State.mine(U.Ui.state(u), th)))

def rows(rs: List<&2, BU.BRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case BU.BRow{+i, +n, _, _, +m, _, _, r, +p, +l} <> t:
      i ++ "|" ++ n ++ "|" ++ m ++ "|" ++ Bool.show(r) ++ "|" ++ p ++ "|" ++ l ++ ";" ++ rows(t)

def acts(rs: List<&2, BU.BRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case +r <> t:
      BU.BU.act(r) ++ ";" ++ acts(t)

def act_ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def route(m: Maybe<&2, FR.Route>) -> String:
  match m:
    case Some{FR.Route{l, mm, b}}:
      l ++ " " ++ mm ++ " " ++ b
    case None{}:
      "none"

def rpc(+m: String, +p: List<&2, J.KV>) -> J.Json:
  J.Json.obj([J.KV{"id", J.Num{"7"}}, J.KV{"m", J.Str{m}}, J.KV{"p", J.Json.obj(p)}])

# the owning hub's state: Kit's thread and a project's thread
def owner() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "board", "/b", 1n}, M.ThreadCreated{"pt", "p", "layout", M.Local{}, "", "", "claude", "", 2n},
    M.ThreadCreated{"bt1", "", "Kit", M.Local{}, "", "", "claude", "", 3n}, M.BotSet{"b1", "Kit", "bt1", 7n, "", 3n},
    M.AskOpened{"a1", "bt1", "tool", "Bash", "ls", "", 4n}, M.AskOpened{"a2", "pt", "tool", "Bash", "ls", "", 5n}], M.State.empty())

def project.items() -> List<&2, FR.Item>:
  numbered([M.ProjectCreated{"p", "Board", "/board", 1n}, M.ThreadCreated{"pt", "p", "Layout", M.Local{}, "", "", "claude", "", 2n},
    M.MessagePosted{"pt", "m", M.User{}, "shared work", 3n}, M.AskOpened{"pa", "pt", "approval", "Bash", "ls", "", 4n}], 15n)

def phone() -> H.Hubs:
  H.Hubs.after(H.Hubs.recv(H.Hubs.after(H.Hubs.recv(H.Hubs.set(H.Hubs.init(1000n, "c"), ["here:3791", "box:3792"]), "here:3791",
    log(0n, [M.PeerSet{"p1", "box", "http://box:3792", False{}, 1n}]))), "here:3791", far(0n, 18n, False{}, project.items())))

def paired(+h: H.Hubs) -> H.Hubs:
  H.Hubs.after(H.Hubs.recv(h, "box:3792", log(0n, [M.PeerSet{"p1", "here", "http://here:3791", False{}, 1n},
    M.ProjectCreated{"p", "Board", "/board", 1n}, M.ThreadCreated{"pt", "p", "Layout", M.Local{}, "", "", "claude", "", 2n}])))

def settled.phone() -> H.Hubs:
  H.Hubs.after(H.Hubs.recv(H.Hubs.after(H.Hubs.recv(paired(phone()), "box:3792", log(3n, [M.ThreadSettled{"pt", True{}, 995n}]))),
    "here:3791", far(18n, 19n, False{}, numbered([M.ThreadSettled{"pt", True{}, 995n}], 19n))))

def first(+o: J.Json, +k: String) -> J.Json:
  Maybe.default(&2, J.Json, List.head(&2, J.Json, J.Json.elems(J.Json.get_obj(o, k))), J.Null{})

def fields(xs: List<&2, J.Json>, +k: String) -> String:
  match xs:
    case Nil{}:
      ""
    case x <> t:
      J.Json.get_str(x, k, "") ++ ";" ++ fields(t, k)

# the owner's Kit leaves a background shell running after its turn, then
# wakes for it and completes
def work.bg() -> List<&2, FR.Item>:
  numbered([M.WorkSet{"bt1", "bg", "s1\tsleep 8\tc1\tsleep 8\tshell", 103n}, M.TurnChanged{"bt1", M.Completed{}, 104n}], 15n)

def work.wake() -> List<&2, FR.Item>:
  numbered([M.TurnChanged{"bt1", M.Running{}, 105n}, M.WorkSet{"bt1", "bg", "", 105n}], 17n)

def work.done() -> List<&2, FR.Item>:
  numbered([M.TurnChanged{"bt1", M.Completed{}, 106n}], 19n)

def phase(+u: U.Ui, +th: String) -> String:
  M.Phase.label(U.Side.phase(U.Ui.state(u), U.Ui.thread(U.Ui.state(u), th)))

# a far message with box's catalog beside it: "pa" done at 30, "pb" at 12
def cat(+since: Nat, +head: Nat) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(FR.Fr.with.cat(FR.Fr.msg("p1", "box", False{}, since, head, Nil{}),
    "[{\"id\":\"pa\",\"title\":\"Alpha\",\"at\":500,\"n\":30},{\"id\":\"pb\",\"title\":\"Beta\",\"at\":400,\"n\":12}]")), J.Null{})

def machines() -> J.Json:
  J.Json.obj([J.KV{"t", J.Json.str("info")}, J.KV{"info", J.Json.obj([J.KV{"machines", J.Json.str("{\"self\":{\"name\":\"me\",\"url\":\"http://100.1.0.1:3787\",\"domain\":\"\"},\"peers\":[{\"name\":\"box\",\"url\":\"http://box:3787\",\"version\":\"1\"},{\"name\":\"lab\",\"url\":\"http://lab:3787\",\"version\":\"1\"}]}")}])}])

# the text of the request an action sends
def rpc_text(a: U.Act) -> String:
  match a:
    case U.Act{_, U.Send{+t} <> _}:
      t
    case _:
      "none"

def pitems(xs: List<&2, U.PItem>) -> String:
  match xs:
    case Nil{}:
      ""
    case U.PItem{l, a, v} <> t:
      l ++ "|" ++ a ++ "|" ++ v ++ ";" ++ pitems(t)

def pids(rs: List<&2, U.SRow>) -> String:
  String.join(U.Side.pids(rs), ",")

def mates(rs: List<&2, U.MateRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case U.MateRow{n, note, on, key} <> t:
      n ++ " " ++ on ++ " " ++ key ++ ": " ++ note ++ "\n" ++ mates(t)

def main() -> IO(Unit):
  do IO<Unit>:
    +u : U.Ui = joined()
    +w1 : U.Ui = recv(u, far(14n, 16n, False{}, work.bg()))
    +w2 : U.Ui = recv(w1, far(16n, 18n, False{}, work.wake()))
    +w3 : U.Ui = recv(w2, far(18n, 19n, False{}, work.done()))
    C.Check.str("mirror: the owner's work as its phase", phase(w1, "p1~bt1") ++ "|" ++ phase(w2, "p1~bt1") ++ "|" ++ phase(w3, "p1~bt1"), "Monitoring 1 task|Working…|Complete")
    C.Check.str("mirror: an epoch reset drops the work", Nat.show(M.Bg.live(M.Bg.of(U.Ui.state(w1), "p1~bt1"))) ++ "/" ++ Nat.show(M.Bg.live(M.Bg.of(FR.Fr.forget(U.Ui.state(w1), "p1"), "p1~bt1"))), "1/0")
    C.Check.str("forget: the index cut like the one rebuilt", Nat.show(List.length(&2, M.Entry, M.State.links(FR.Fr.forget(FR.Fr.demo.idx(), "p1"))))
      ++ "/" ++ Nat.show(List.length(&2, M.Entry, M.State.mine(FR.Fr.forget(FR.Fr.demo.idx(), "p1"), "p1~t1"))) ++ "/" ++ Nat.show(List.length(&2, M.Entry, M.State.mine(FR.Fr.forget(FR.Fr.demo.idx(), "p1"), "t1"))), "2/1/3")
    C.Check.str("ids", FR.Fr.id("p1", "t~x") ++ " " ++ FR.Fr.link("p1~t~x") ++ " " ++ FR.Fr.local("p1~t~x") ++ " [" ++ FR.Fr.link("t1") ++ "] " ++ FR.Fr.local("t1"),
      "p1~t~x p1 t~x [] t1")
    C.Check.str("thread folded in under its link", count(u, "p1~bt1") ++ " " ++ title(U.Ui.thread(U.Ui.state(u), "p1~bt1")), "1 Kit")
    C.Check.str("its hub's sequence untouched", Nat.show(M.State.seq(U.Ui.state(u))), "3")
    C.Check.str("bot listed once, as a bot with its machine", rows(BU.BU.rows(u)),
      "b9|Pip|sleep|False||;p1~b1|Kit|work|True|box|p1;")
    C.Check.str("rows open as bots", acts(BU.BU.rows(u)), "bot;bot;")
    C.Check.str("title with its machine", U.Ui.far.title(u, "p1~bt1", "Kit") ++ "/" ++ U.Ui.far.title(u, "t1", "Pip"), "Kit · box/Pip")
    +pu : U.Ui = recv(u, far(14n, 18n, False{}, project.items()))
    C.Check.str("ordinary project thread folded with its parent", U.Ui.thread_project(U.Ui.thread(U.Ui.state(pu), "p1~pt")) ++ " " ++ count(pu, "p1~pt"), "p1~p 1")
    C.Check.str("ordinary thread marker", U.Label.title(U.Ui.state(pu), "p1~pt", "Layout"), "Layout · box")
    C.Check.str("far composer draft survives saving", Bool.show(U.Draft.safe("p1~pt")) ++ " " ++ Bool.show(U.Draft.safe("p1~../pt")), "True False")
    C.Check.str("shared approval id", Nat.show(List.length(&2, M.Ask, U.Asks.open(U.Ui.state(pu), "p1~pt"))), "1")
    C.Check.str("phone project marker", J.Json.get_str(first(H.Hubs.screen(phone()), "projects"), "title", ""), "Board · box")
    C.Check.str("phone thread marker", J.Json.get_str(first(first(H.Hubs.screen(phone()), "projects"), "threads"), "title", ""), "Layout · box")
    C.Check.str("phone keeps mirror while owner catalog not installed", Nat.show(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(H.Hubs.screen(phone()), "projects")))), "1")
    C.Check.str("phone paired owner removes mirror project", fields(J.Json.elems(J.Json.get_obj(H.Hubs.screen(paired(phone())), "projects")), "id"), "box:3792|p;")
    C.Check.str("paired owner's project keeps its machine marker", J.Json.get_str(first(H.Hubs.screen(paired(phone())), "projects"), "machine", ""), "box")
    +active : H.Hubs = H.Hubs.after(H.Hubs.act(paired(phone()), "setting", "sidebar.view=active"))
    C.Check.str("phone active has one ordinary thread", fields(J.Json.elems(J.Json.get_obj(H.Hubs.screen(active), "active")), "id"), "box:3792|pt;")
    +settled : H.Hubs = H.Hubs.after(H.Hubs.act(settled.phone(), "setting", "sidebar.view=active"))
    C.Check.str("phone counts hidden settled thread once", Nat.show(J.Json.get_nat(J.Json.get_obj(H.Hubs.screen(settled), "settled"), "count", 0n)), "1")
    +older : H.Hubs = H.Hubs.after(H.Hubs.tick(settled, 1000000n))
    C.Check.str("phone counts hidden older project once", Nat.show(J.Json.get_nat(J.Json.get_obj(H.Hubs.screen(older), "older"), "count", 0n)), "1")
    +bp : H.Hubs = H.Hubs.after(H.Hubs.recv(H.Hubs.after(H.Hubs.recv(H.Hubs.set(H.Hubs.init(1000n, "c"), ["here:3791", "box:3792"]),
      "here:3791", log(0n, [M.PeerSet{"p1", "box", "http://box:3792", False{}, 1n}]))), "here:3791", far(0n, 14n, False{}, box())))
    C.Check.str("phone keeps bot until paired owner installs it", Nat.show(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(H.Hubs.screen(bp), "bots")))), "1")
    +bp2 : H.Hubs = H.Hubs.after(H.Hubs.recv(bp, "box:3792", log(0n, [M.PeerSet{"p1", "here", "http://here:3791", False{}, 1n},
      M.ThreadCreated{"bt1", "", "Kit", M.Local{}, "", "", "claude", "", 100n}, M.BotSet{"b1", "Kit", "bt1", 7n, "helps", 100n}])))
    C.Check.str("phone paired owner lists bot once", Nat.show(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(H.Hubs.screen(bp2), "bots")))), "1")
    # the report of it (Kit@box) opens the mirrored bot, not a hub switch
    +s : U.Ui = act_ui(U.Ui.act(u, "remote", "Kit@box"))
    C.Check.str("remote pick opens the mirror", U.Ui.scratch(s, "@bot") ++ " " ++ U.Ui.sel(s) ++ " [" ++ U.Ui.scratch(s, "@remote") ++ "] [" ++ U.Ui.info(s, "local.machine") ++ "]",
      "p1~b1 p1~bt1 [] []")
    # the same batch again, and an older one, change nothing
    +u2 : U.Ui = recv(recv(u, far(0n, 14n, False{}, box())), far(10n, 13n, False{}, box()))
    C.Check.str("a batch twice is taken once", count(u2, "p1~bt1"), "1")
    # a batch past a gap is left for the pull
    +u3 : U.Ui = recv(u2, far(18n, 21n, False{}, more()))
    C.Check.str("a gap takes nothing", count(u3, "p1~bt1") ++ " " ++ Nat.show(FR.Fr.head(U.Ui.state(u3), "p1")), "1 14")
    +u4 : U.Ui = recv(u3, far(14n, 21n, False{}, more()))
    C.Check.str("the next batch in order", count(u4, "p1~bt1") ++ " " ++ Nat.show(FR.Fr.head(U.Ui.state(u4), "p1")), "2 21")
    # A new authoritative history replaces only its projection. Same
    # epoch on another snapshot remains idempotent, including after load.
    +e1 : U.Ui = recv(u4, Maybe.default(&2, J.Json, J.Json.parse(FR.Fr.msg.epoch("p1", "box", False{}, 0n, 14n, box(), "old")), J.Null{}))
    +e2 : U.Ui = recv(e1, Maybe.default(&2, J.Json, J.Json.parse(FR.Fr.msg.epoch("p2", "other", False{}, 0n, 14n, box(), "other")), J.Null{}))
    +e3 : U.Ui = recv(e2, Maybe.default(&2, J.Json, J.Json.parse(FR.Fr.msg.epoch("p1", "box", False{}, 0n, 1n,
      numbered([M.ThreadCreated{"new", "", "New history", M.Local{}, "", "", "claude", "", 200n}], 1n), "new")), J.Null{}))
    C.Check.str("reset removes old entries and bot", count(e3, "p1~bt1") ++ " " ++ Bool.show(BT.Maybe.is(BT.Bot, BT.World.find.id(BT.World.bot(M.State.bots(U.Ui.state(e3)), "p1~b1")))), "0 False")
    C.Check.str("reset preserves local and other mirror", Nat.show(M.State.seq(U.Ui.state(e3))) ++ " " ++ count(e3, "p2~bt1") ++ " " ++ title(U.Ui.thread(U.Ui.state(e3), "t1")), "3 1 Pip")
    C.Check.str("reset takes the lower new head", Nat.show(FR.Fr.head(U.Ui.state(e3), "p1")) ++ " " ++ title(U.Ui.thread(U.Ui.state(e3), "p1~new")), "1 New history")
    +disk : FR.In = FR.Fr.file.read("{\"epoch\":\"new\"}\n" ++ FR.Fr.file.batch(numbered([M.ThreadCreated{"new", "", "New history", M.Local{}, "", "", "claude", "", 200n}], 1n), 1n), "p1", "box")
    C.Check.str("disk keeps epoch and head", FR.In.epoch(disk) ++ " " ++ Nat.show(FR.In.head(disk)) ++ " " ++ Bool.show(FR.In.away(disk)), "new 1 True")
    C.Check.str("late disk load cannot replace live batch", Nat.show(FR.In.head(FR.In.loaded(FR.In.put(FR.In.new("p1", "box"), 0n, 21n, more()), disk))), "21")
    C.Check.str("reply preserves owner's refusal and request id", FR.Fr.reply(J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{"0"}}, J.KV{"ok", J.Flag{False{}}}, J.KV{"error", J.Str{"cannot start"}}]), "23"),
      "{\"t\":\"reply\",\"id\":23,\"ok\":false,\"error\":\"cannot start\"}")
    # its machine stops answering: kept, away
    +u5 : U.Ui = recv(u4, far(21n, 21n, True{}, Nil{}))
    C.Check.str("away keeps it", count(u5, "p1~bt1") ++ " " ++ rows(BU.BU.rows(u5)) ++ " " ++ U.Ui.far.title(u5, "p1~bt1", "Kit"),
      "2 b9|Pip|sleep|False||;p1~b1|Kit|away|True|box|p1; Kit · box (away)")
    # a reconnect resumes this hub's log: the mirror stays; a fresh log
    # forgets it with its head, and the snapshot brings it back
    +u6 : U.Ui = recv(u5, log(3n, Nil{}))
    C.Check.str("resume keeps it", count(u6, "p1~bt1"), "2")
    +u7 : U.Ui = recv(u6, log(0n, here()))
    C.Check.str("fresh log forgets it", count(u7, "p1~bt1") ++ " " ++ Nat.show(FR.Fr.head(U.Ui.state(u7), "p1")), "0 0")
    +u8 : U.Ui = recv(u7, far(0n, 21n, False{}, List.append(&2, FR.Item, box(), more())))
    C.Check.str("snapshot again", count(u8, "p1~bt1"), "2")
    # requests: one about a thread elsewhere goes to its machine in its ids
    C.Check.str("route turn.start", route(FR.Fr.route(rpc("turn.start", [J.KV{"thread", J.Str{"p1~bt1"}}, J.KV{"text", J.Str{"hi"}}, J.KV{"msg", J.Str{"m9"}}]))),
      "p1 turn.start {\"id\":0,\"m\":\"turn.start\",\"p\":{\"thread\":\"bt1\",\"text\":\"hi\",\"msg\":\"m9\"}}")
    C.Check.str("route ask.answer", route(FR.Fr.route(rpc("ask.answer", [J.KV{"id", J.Str{"p1~a1"}}, J.KV{"answer", J.Str{"yes"}}]))),
      "p1 ask.answer {\"id\":0,\"m\":\"ask.answer\",\"p\":{\"id\":\"a1\",\"answer\":\"yes\"}}")
    C.Check.str("a request here stays", route(FR.Fr.route(rpc("turn.start", [J.KV{"thread", J.Str{"bt1"}}, J.KV{"text", J.Str{"a~b"}}]))), "none")
    # the owning hub: what it shares, and what it lets a linked machine do
    +st : M.State = owner()
    C.Check.str("shared", Bool.show(FR.Fr.shared(st, M.MessagePosted{"bt1", "m", M.User{}, "x", 9n})) ++ " " ++ Bool.show(FR.Fr.shared(st, M.MessagePosted{"pt", "m", M.User{}, "x", 9n}))
      ++ " " ++ Bool.show(FR.Fr.shared(st, M.SettingSet{"k", "v", 9n})) ++ " " ++ Bool.show(FR.Fr.shared(st, M.AskClosed{"a1", "y", 9n})) ++ " " ++ Bool.show(FR.Fr.shared(st, M.AskClosed{"a2", "y", 9n})),
      "True True False True True")
    C.Check.str("allowed", Bool.show(FR.Fr.allowed(st, rpc("turn.start", [J.KV{"thread", J.Str{"bt1"}}]))) ++ " " ++ Bool.show(FR.Fr.allowed(st, rpc("turn.start", [J.KV{"thread", J.Str{"pt"}}])))
      ++ " " ++ Bool.show(FR.Fr.allowed(st, rpc("ask.answer", [J.KV{"id", J.Str{"a1"}}]))) ++ " " ++ Bool.show(FR.Fr.allowed(st, rpc("ask.answer", [J.KV{"id", J.Str{"a2"}}])))
      ++ " " ++ Bool.show(FR.Fr.allowed(st, rpc("setting.set", [J.KV{"key", J.Str{"x"}}]))) ++ " " ++ Bool.show(FR.Fr.allowed(st, rpc("thread.delete", [J.KV{"thread", J.Str{"bt1"}}]))),
      "True True True True False False")
    # create anywhere: a new thread, a fork, pin, archive, delete and a
    # project's folders go to the machine that owns the thing
    C.Check.str("route thread.create", route(FR.Fr.route(rpc("thread.create", [J.KV{"project", J.Str{"p1~p"}}, J.KV{"title", J.Str{"New thread"}}]))),
      "p1 thread.create {\"id\":0,\"m\":\"thread.create\",\"p\":{\"project\":\"p\",\"title\":\"New thread\"}}")
    C.Check.str("route thread.fork", route(FR.Fr.route(rpc("thread.fork", [J.KV{"thread", J.Str{"p1~pt"}}]))), "p1 thread.fork {\"id\":0,\"m\":\"thread.fork\",\"p\":{\"thread\":\"pt\"}}")
    C.Check.str("route pin, archive", route(FR.Fr.route(rpc("thread.pin", [J.KV{"thread", J.Str{"p1~pt"}}]))) ++ "|" ++ route(FR.Fr.route(rpc("thread.archive", [J.KV{"thread", J.Str{"p1~pt"}}]))),
      "p1 thread.pin {\"id\":0,\"m\":\"thread.pin\",\"p\":{\"thread\":\"pt\"}}|p1 thread.archive {\"id\":0,\"m\":\"thread.archive\",\"p\":{\"thread\":\"pt\"}}")
    C.Check.str("route project.add on a machine", route(FR.Fr.route(rpc("project.add", [J.KV{"on", J.Str{"p1"}}, J.KV{"path", J.Str{"/x"}}]))),
      "p1 project.add {\"id\":0,\"m\":\"project.add\",\"p\":{\"on\":\"p1\",\"path\":\"/x\"}}")
    C.Check.str("route fs.list on a machine", route(FR.Fr.route(rpc("fs.list", [J.KV{"path", J.Str{"/x"}}, J.KV{"on", J.Str{"p1"}}]))),
      "p1 fs.list {\"id\":0,\"m\":\"fs.list\",\"p\":{\"path\":\"/x\",\"on\":\"p1\"}}")
    C.Check.str("no machine: stays here", route(FR.Fr.route(rpc("fs.list", [J.KV{"on", J.Str{""}}, J.KV{"path", J.Str{"/x"}}]))) ++ "|" ++ route(FR.Fr.route(rpc("project.add", [J.KV{"path", J.Str{"/x"}}]))), "none|none")
    C.Check.str("allowed: create in a shared project only", Bool.show(FR.Fr.allowed(st, rpc("thread.create", [J.KV{"project", J.Str{"p"}}]))) ++ " " ++ Bool.show(FR.Fr.allowed(st, rpc("thread.create", [J.KV{"project", J.Str{"zz"}}])))
      ++ " " ++ Bool.show(FR.Fr.allowed(st, rpc("thread.create", [J.KV{"project", J.Str{"p1~p"}}]))) ++ " " ++ Bool.show(FR.Fr.allowed(st, rpc("thread.create", Nil{}))),
      "True False False False")
    C.Check.str("allowed: a thread's own", Bool.show(FR.Fr.allowed(st, rpc("thread.fork", [J.KV{"thread", J.Str{"pt"}}]))) ++ " " ++ Bool.show(FR.Fr.allowed(st, rpc("thread.pin", [J.KV{"thread", J.Str{"pt"}}])))
      ++ " " ++ Bool.show(FR.Fr.allowed(st, rpc("thread.archive", [J.KV{"thread", J.Str{"nope"}}]))) ++ " " ++ Bool.show(FR.Fr.allowed(st, rpc("thread.pin", [J.KV{"thread", J.Str{"x~pt"}}]))),
      "True True False False")
    C.Check.str("allowed: projects and folders", Bool.show(FR.Fr.allowed(st, rpc("project.remove", [J.KV{"project", J.Str{"p"}}]))) ++ " " ++ Bool.show(FR.Fr.allowed(st, rpc("project.remove", [J.KV{"project", J.Str{"q"}}])))
      ++ " " ++ Bool.show(FR.Fr.allowed(st, rpc("project.add", [J.KV{"path", J.Str{"/x"}}]))) ++ " " ++ Bool.show(FR.Fr.allowed(st, rpc("fs.list", Nil{})))
      ++ " " ++ Bool.show(FR.Fr.allowed(st, rpc("project.create", [J.KV{"path", J.Str{"/x"}}]))) ++ " " ++ Bool.show(FR.Fr.allowed(st, rpc("thread.reorder", [J.KV{"thread", J.Str{"pt"}}]))),
      "True False True True False False")
    C.Check.str("a pin is shared", Bool.show(FR.Fr.shared(st, M.ThreadPinned{"pt", True{}, 9n})) ++ " " ++ Bool.show(FR.Fr.shared(st, M.ThreadPinned{"zz", True{}, 9n})) ++ " " ++ FR.Fr.scope(st, M.ThreadPinned{"pt", True{}, 9n}), "True False p")
    # the owner's answer names the thread by its id here
    C.Check.str("reply names the new thread here", FR.Fr.reply.on(J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{"0"}}, J.KV{"ok", J.Flag{True{}}}, J.KV{"thread", J.Str{"t5"}}]), "23", "p1"),
      "{\"t\":\"reply\",\"id\":23,\"ok\":true,\"thread\":\"p1~t5\"}")
    C.Check.str("reply keeps a refusal", FR.Fr.reply.on(J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{"0"}}, J.KV{"ok", J.Flag{False{}}}, J.KV{"error", J.Str{"no"}}]), "23", "p1"),
      "{\"t\":\"reply\",\"id\":23,\"ok\":false,\"error\":\"no\"}")
    # a client asks for a thread in a project elsewhere and goes to it when
    # the mirror has it, whichever comes first
    +nt : U.Ui = act_ui(U.Ui.new_thread(pu, "p1~p"))
    +nid : String = Nat.show(U.Made.rid(pu))
    +ans : J.Json = U.Made.demo(nid, "p1~t9")
    +born : J.Json = far(18n, 19n, False{}, numbered([M.ThreadCreated{"t9", "p", "New thread", M.Local{}, "", "", "claude", "", 200n}], 19n))
    C.Check.str("making until its answer", Bool.show(U.Made.waiting(nt)), "True")
    +na : U.Ui = recv(nt, ans)
    C.Check.str("answer first: still making, nothing selected", Bool.show(U.Made.waiting(na)) ++ " [" ++ U.Ui.sel(na) ++ "]", "True []")
    +nb : U.Ui = recv(na, born)
    C.Check.str("then the mirror: selected, done", Bool.show(U.Made.waiting(nb)) ++ " [" ++ U.Ui.sel(nb) ++ "]", "False [p1~t9]")
    +mb : U.Ui = recv(recv(nt, born), ans)
    C.Check.str("mirror first: selected, done", Bool.show(U.Made.waiting(mb)) ++ " [" ++ U.Ui.sel(mb) ++ "]", "False [p1~t9]")
    +nf : U.Ui = recv(nt, J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{nid}}, J.KV{"ok", J.Flag{False{}}}]))
    C.Check.str("a failure ends it", Bool.show(U.Made.waiting(nf)), "False")
    +nl : U.Ui = act_ui(U.Ui.select(na, "p1~pt"))
    C.Check.str("going elsewhere ends it", Bool.show(U.Made.waiting(nl)) ++ " [" ++ U.Ui.sel(nl) ++ "]", "False [p1~pt]")
    # the picker works on another machine: its requests carry it
    C.Check.str("picker machines", String.join(U.Proj.machines(pu), ",") ++ " [" ++ U.Proj.on(pu) ++ "] " ++ U.Proj.next(pu), "p1 [] p1")
    +pm : U.Ui = act_ui(U.Ui.act(pu, "proj-on", "p1"))
    C.Check.str("picker on a machine", U.Proj.on(pm) ++ " " ++ U.Proj.next(pm) ++ "|" ++ U.Proj.on.name(pm) ++ "|" ++ String.join(U.Side.pids(U.Side.sorted(U.Ui.state(pm), 1000n)), ","), "p1 |box|p1~p")
    C.Check.str("picker's request names the machine", rpc_text(U.Proj.rpc(pm, "fs.list", [J.KV{"path", J.Str{"/x"}}])) ++ "|" ++ rpc_text(U.Proj.rpc(pu, "fs.list", [J.KV{"path", J.Str{"/x"}}])),
      "{\"id\":" ++ Nat.show(U.Made.rid(pm)) ++ ",\"m\":\"fs.list\",\"p\":{\"on\":\"p1\",\"path\":\"/x\"}}|{\"id\":" ++ Nat.show(U.Made.rid(pu)) ++ ",\"m\":\"fs.list\",\"p\":{\"path\":\"/x\"}}")
    C.Check.str("picker row for the machine", pitems(U.Proj.machine.row(pm)), "On box (change)|proj-on|;")
    # a catalog: the link's projects listed at once, each catching up until
    # the link's head passes its last number; Settings' machines say so
    +uc : U.Ui = recv(recv(joined(), machines()), cat(14n, 14n))
    C.Check.str("catalog lists every project at once", pids(U.Side.sorted(U.Ui.state(uc), 1000n)), "p1~pa,p1~pb")
    C.Check.str("catalog: the one past the head is syncing", U.Label.title(U.Ui.state(uc), "p1~pa", "Alpha") ++ "|" ++ U.Label.title(U.Ui.state(uc), "p1~pb", "Beta"),
      "Alpha · box (syncing)|Beta · box")
    C.Check.str("machines while catching up", mates(U.Mate.rows(uc)),
      "box on sync.box: Catching up: 1 project still loading. The rest are ready.\nlab on sync.lab: Linking. Its projects will appear here within a minute or two.\n")
    +ud : U.Ui = recv(uc, far(14n, 30n, False{}, Nil{}))
    C.Check.str("caught up", U.Label.title(U.Ui.state(ud), "p1~pa", "Alpha") ++ "|" ++ mates(U.Mate.rows(ud)),
      "Alpha · box|box on sync.box: In sync. Its projects and threads show here as they change.\nlab on sync.lab: Linking. Its projects will appear here within a minute or two.\n")
    +uo : U.Ui = U.Act.ui(U.Ui.setting(ud, "sync.box=off"))
    C.Check.str("off: its projects leave the sidebar", pids(U.Side.sorted(U.Ui.state(uo), 1000n)) ++ "|" ++ mates(U.Mate.rows(uo)),
      "|box off sync.box: Off. Nothing syncs with it.\nlab on sync.lab: Linking. Its projects will appear here within a minute or two.\n")
    # the owning hub's catalog: each project's last shared change
    +ol : List<&2, String> = ["{\"$\":\"MessagePosted\",\"thread\":\"t2\",\"msg\":\"m\",\"role\":\"user\",\"text\":\"x\",\"at\":9}",
      "{\"$\":\"ThreadCreated\",\"id\":\"t2\",\"project\":\"q2\",\"title\":\"T\",\"env\":\"local\",\"branch\":\"\",\"worktree\":\"\",\"provider\":\"claude\",\"model\":\"\",\"at\":8}",
      "{\"$\":\"ProjectCreated\",\"id\":\"q2\",\"title\":\"Two\",\"root\":\"/b\",\"at\":7}",
      "{\"$\":\"ProjectCreated\",\"id\":\"q1\",\"title\":\"One\",\"root\":\"/a\",\"at\":6}"]
    +ost : M.State = M.State.apply(M.State.apply(M.State.apply(M.State.apply(M.State.empty(), M.ProjectCreated{"q1", "One", "/a", 6n}), M.ProjectCreated{"q2", "Two", "/b", 7n}),
      M.ThreadCreated{"t2", "q2", "T", M.Local{}, "", "", "claude", "", 8n}), M.MessagePosted{"t2", "m", M.User{}, "x", 9n})
    C.Check.str("owner's catalog", J.Json.show(FR.Fr.catalog(ost, FR.Feed.lasts(FR.Fr.feed.full(ol, ost)))),
      "[{\"id\":\"q1\",\"title\":\"One\",\"at\":6,\"n\":1},{\"id\":\"q2\",\"title\":\"Two\",\"at\":9,\"n\":4}]")
    # a pull's answer: the page's fields and the catalog side by side
    +lg : J.Json = Maybe.default(&2, J.Json, J.Json.parse(FR.Fr.log.json(3n, FR.Fr.catalog(ost, FR.Feed.lasts(FR.Fr.feed.full(ol, ost))),
      FR.Fr.with.epoch(FR.Page.json(FR.Fr.page(FR.Fr.feed(ol, ost), 0n, 4n, 400000n)), "ep1"))), J.Null{})
    C.Check.str("a pull's answer reads whole", Nat.show(J.Json.get_nat(lg, "since", 9n)) ++ " " ++ Nat.show(J.Json.get_nat(lg, "head", 0n)) ++ " " ++ Nat.show(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(lg, "items"))))
      ++ " " ++ J.Json.get_str(lg, "epoch", "") ++ " " ++ Nat.show(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(lg, "catalog")))) ++ " " ++ Bool.show(J.Json.get_bool(lg, "more", True{})),
      "3 4 4 ep1 2 False")
    # a mirrored bot's space page, served by the mirroring hub
    +pi : FR.In = FR.In.put(FR.In.new("p1", "box"), 0n, 3n, numbered([M.SpacePageSet{"b1", "<p>old</p>", 1n}, M.SpacePageSet{"b2", "<p>other</p>", 2n}, M.SpacePageSet{"b1", "<p>new</p>", 3n}], 1n))
    C.Check.str("mirrored space page", FR.In.page(pi, "b1") ++ "|" ++ FR.In.page(pi, "b9"), "<p>new</p>|")
    # the feed made a slice at a time, and only once asked for (F1)
    +dl : List<&2, String> = FR.Fr.feed.demo.lines()
    +dst : M.State = FR.Fr.feed.demo.st()
    C.Check.str("feed: slices of 2 give the same pages as the whole log",
      FR.Page.json(FR.Fr.page(FR.Feed.lines(FR.Fr.feed.sliced(20n, FR.Fr.feed.start(dl), 2n, dst)), 0n, 7n, 400000n)),
      FR.Page.json(FR.Fr.page(FR.Fr.feed(dl, dst), 0n, 7n, 400000n)))
    C.Check.str("feed: shared lines of the demo", Nat.show(List.length(&2, FR.Line, FR.Fr.feed(dl, dst))), "5")
    +l9 : List<&2, FR.Line> = [FR.Line{9n, "x"}]
    +l1 : FR.Feed = FR.Feed{[FR.Line{1n, "a"}], Map.new(&2, Nat)}
    C.Check.str("feed: a commit before anyone asked is not kept",
      Nat.show(List.length(&2, FR.Line, Maybe.default(&2, List<&2, FR.Line>, FR.Fh.feed(FR.Fh.feed.ready(FR.Fh.feed.add(FR.Fh.empty(), l9), l1)), Nil{}))), "1")
    C.Check.str("feed: once asked, a commit waits beside it",
      Nat.show(List.length(&2, FR.Line, Maybe.default(&2, List<&2, FR.Line>, FR.Fh.feed(FR.Fh.feed.ready(FR.Fh.feed.add(FR.Fh.feed.ask(FR.Fh.empty()), l9), l1)), Nil{}))), "2")
    C.Check.str("feed: not asked at start, asked after", Bool.show(FR.Fh.feed.asked(FR.Fh.empty())) ++ " " ++ Bool.show(FR.Fh.feed.asked(FR.Fh.feed.ask(FR.Fh.empty()))), "False True")
