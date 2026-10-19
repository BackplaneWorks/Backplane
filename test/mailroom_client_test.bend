import Base
import ./check.bend as C
import ./mailroom_parity_test.bend as PT
import ../src/core/client.bend as U
import ../src/core/mailview.bend as MV
import ../src/core/model.bend as M
import ../src/core/mail.bend as ML
import ../src/core/json.bend as J

# Replies are tied to the request, not to whichever filter is now shown.
def reply(+id: String, result: J.Json) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{id}}, J.KV{"ok", J.Flag{True{}}}, J.KV{"result", result}])

def list_result() -> J.Json:
  J.Json.obj([J.KV{"convs", J.Json.arr([J.Json.obj([J.KV{"id", J.Str{"c"}}, J.KV{"subject", J.Str{"Deploy report"}}, J.KV{"head", J.Str{"Finished"}}, J.KV{"last", J.Json.nat(100n)}])])}, J.KV{"total", J.Json.nat(1n)}])

def read_result() -> J.Json:
  J.Json.obj([J.KV{"conv", J.Json.obj([J.KV{"id", J.Str{"c"}}, J.KV{"subject", J.Str{"Deploy report"}}, J.KV{"parts", J.Json.arr([J.Str{"person"}, J.Str{"bot:b1"}, J.Str{"thread:t1"}])}])}, J.KV{"mails", J.Json.arr([J.Json.obj([J.KV{"id", J.Str{"m1"}}, J.KV{"from", J.Str{"bot:b1"}}, J.KV{"text", J.Str{"Ready"}}, J.KV{"at", J.Json.nat(100n)}])])}, J.KV{"more", J.Flag{True{}}}])

def first(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}: ""
    case U.Send{x} <> _: J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(x), J.Null{}), "m", "")
    case _ <> t: first(t)

def method(a: U.Act) -> String:
  first(U.Act.cmds(a))

def ui() -> U.Ui:
  U.Ui.fold([M.ConvSet{"c", "Deploy report", "pair", "person\nbot:b1", "", "", 100n}, M.MailPosted{"seed", "c", "bot:b1", "person", "thread", "", "fyi", "Finished", 8n, 100n}], U.Ui.far.st(PT.ui(), M.State.apply(U.Ui.state(PT.ui()), M.SettingSet{"mail.on", "on", 200000n})))

def indexed() -> U.Ui:
  U.Ui.fold([M.ConvSet{"act-c", "Action", "pair", "person\nbot:b1\nbot:b2", "project", "", 1n}, M.MailPosted{"a1", "act-c", "person", "bot:b1", "thread", "", "act", "Do it", 5n, 2n},
    M.ConvSet{"fyi-c", "News", "pair", "person\nbot:b2", "project", "", 3n}, M.MailPosted{"f1", "fyi-c", "person", "bot:b2", "thread", "", "fyi", "News", 4n, 4n}], U.Ui.init(100n))

def index_ids(+u: U.Ui, +bot: String, +kind: String) -> String:
  String.join(ML.Order.slice(U.Mailv.index.order(U.Mailv.index(u), U.Mailv.index.key(bot, kind, "project")), 0n, 10n), ",")

def main() -> IO(Unit):
  do IO<Unit>:
    +a : U.Act = U.Mailv.sync(U.Act{ui(), Nil{}})
    +u : U.Ui = U.Act.ui(a)
    C.Check.str("open pane requests list", method(a), "mail.list")
    C.Check.str("one list in flight", method(U.Mailv.sync(U.Act{u, Nil{}})), "")
    +id : String = U.Ui.scratch(u, "@mr.list")
    +got : U.Ui = U.Ui.recv(u, reply(id, list_result()))
    C.Check.str("reply parsed once into typed page", MV.Mv.ids(MV.Mv.rows(U.Mailv.cache(got), U.Mailv.key(got), 0n, 10n)), "c,")
    C.Check.str("fresh page no request", method(U.Mailv.sync(U.Act{got, Nil{}})), "")
    +switched : U.Ui = U.Mailv.chip(u, "b2")
    +stale : U.Ui = U.Ui.recv(switched, reply(id, list_result()))
    C.Check.str("old reply stays in old filter", Nat.show(MV.Mv.total(U.Mailv.cache(stale), U.Mailv.key(stale))), "0")
    C.Check.str("new filter fetches", method(U.Mailv.sync(U.Act{stale, Nil{}})), "mail.list")
    +opened : U.Act = U.Ui.act(got, "mail-open", "c")
    +rd : U.Ui = U.Ui.recv(U.Act.ui(opened), reply(U.Ui.scratch(U.Act.ui(opened), "@mr.read"), read_result()))
    C.Check.str("conversation requests bodies", method(opened), "mail.read")
    C.Check.str("conversation bubbles cached", Nat.show(List.length(&2, MV.Bub, U.Mailv.bubbles(rd))), "1")
    C.Check.str("recipient picker excludes person", Nat.show(List.length(&2, MV.Opt, U.Mailv.targets(rd))), "2")
    C.Check.str("load earlier cursor", U.Mailv.before(rd), "m1")
    +marked : U.Ui = U.Mailv.see(rd, M.MailMarked{"b1", "c", "open", 100n, 0n, 200001n})
    C.Check.str("reading a mark cannot create a read loop", U.Ui.scratch(marked, "@mr.read.dirty"), "")
    C.Check.str("body arrival dirties open read", U.Ui.scratch(U.Mailv.see(rd, M.MailPosted{"m2", "c", "bot:b2", "bot:b1", "thread", "", "fyi", "new", 3n, 200001n}), "@mr.read.dirty"), "1")
    +typed : U.Ui = U.Act.ui(U.Mailv.sync(U.Act{U.Ui.scratch_set(got, "@f.mrsearch", "deploy"), Nil{}}))
    C.Check.str("header matches immediately", MV.Mv.ids(MV.Find.hits(U.Mailv.find(typed))), "c,")
    C.Check.str("reconnect drops in-flight list slot", U.Ui.scratch(U.Mailv.join(True{}, u), "@mr.list"), "")
    C.Check.str("mail send requires recipient", method(U.Ui.act(U.Ui.scratch_set(rd, "@f.mrreply", "hello"), "mail-send", "")), "")
    C.Check.str("Act order is indexed by bot and project", index_ids(indexed(), "b1", "act"), "act-c")
    C.Check.str("All mail order includes both kinds", index_ids(indexed(), "*", ""), "fyi-c,act-c")
    C.Check.str("FYI order is indexed separately", index_ids(indexed(), "b2", "fyi"), "fyi-c")
    C.Check.str("All unread index aggregates recipient marks", index_ids(indexed(), "*", "unread"), "fyi-c,act-c")
    C.Check.str("act unread counter excludes FYI", Nat.show(U.Mailv.act_unread(indexed())), "2")
    C.Check.str("read decrements the act counter", Nat.show(U.Mailv.act_unread(U.Ui.fold([M.MailMarked{"b1", "act-c", "open", 2n, 0n, 5n}], indexed()))), "1")
    C.Check.str("read of one recipient keeps other unread", index_ids(U.Ui.fold([M.MailMarked{"b1", "act-c", "open", 2n, 0n, 5n}], indexed()), "*", "unread"), "fyi-c,act-c")
    C.Check.str("duplicate header cannot duplicate the order", index_ids(U.Ui.fold([M.MailPosted{"a1", "act-c", "person", "bot:b1", "thread", "", "act", "Do it", 5n, 2n}], indexed()), "b1", "act"), "act-c")
    C.Check.str("mail send uses selected recipient", method(U.Ui.act(U.Ui.scratch_set(U.Ui.scratch_set(rd, "@mr.to", "thread:t1"), "@f.mrreply", "hello"), "mail-send", "")), "mail.send")
