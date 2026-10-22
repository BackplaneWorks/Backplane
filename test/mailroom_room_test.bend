import Base
import ./check.bend as C
import ./mailfile_test.bend as F
import ./mail_ui_quiet_test.bend as Q
import ./mailstore_test.bend as ST
import ../src/core/hub.bend as H
import ../src/core/bothub.bend as BH
import ../src/core/model.bend as M
import ../src/core/mail.bend as ML
import ../src/core/mailbody.bend as MB
import ../src/core/mailroom_history.bend as RH
import ../src/core/json.bend as J
import ../src/core/client.bend as U
import ../src/core/mailview.bend as MV
import ../src/core/mcp.bend as MC
import ../src/server/server.bend as S
import ../src/server/mailstore.bend as MS

# Public synthetic events only. The reproduction mirrors the observed OFF
# RoomPosted/header split, without copying or replaying live user data.
def initial() -> M.State:
  M.State.apply(F.disabled(), M.RoomSet{"r", "Public room", "bb\nbc", 1n})

def historic() -> M.State:
  M.State.replay([M.RoomPosted{"p1", "r", "Beta", "first é🙂\nfull text", 0n, 1n}, M.RoomPosted{"p3", "r", "Gamma", "third 漢字", 0n, 3n}], initial())

def filed(+s: M.State, +id: String, +text: String, +at: Nat) -> MB.Body:
  MB.Body{ML.Box.conv(M.State.mail(s), "room:r"), ML.Mail{id, "room:r", "person", "room:r", "room", "r", "fyi", ML.Mail.head(text), H.Mail.bytes(text, 0n), at}, text}

def mixed() -> M.State:
  M.State.replay([M.RoomPosted{"p1", "r", "Beta", "first é🙂\nfull text", 0n, 1n}, MB.Body.header(MB.Body.mail(filed(initial(), "f2", "second 🧪", 2n))), M.RoomPosted{"p3", "r", "Gamma", "third 漢字", 0n, 3n}, MB.Body.header(MB.Body.mail(filed(initial(), "f4", "fourth\nUnicode 👩‍🔬", 4n)))], initial())

def ids(ms: List<&2, ML.Mail>, acc: String) -> String:
  match ms:
    case Nil{}: acc
    case h <> t: ids(t, acc ++ ML.Mail.id(h) ++ ",")

def plan(p: Maybe<&2, RH.RoomPage>) -> String:
  match p:
    case None{}: "error"
    case Some{RH.RoomPage{hs, more}}: ids(hs, "") ++ "/" ++ Bool.show(more)

def rpc(+s: M.State, +to: String) -> H.Step:
  BH.Bots.hub(s, 10n, 7, J.Json.obj([J.KV{"id", J.Num{"31"}}, J.KV{"m", J.Str{"mail.send"}}, J.KV{"p", J.Json.obj([J.KV{"to", J.Str{to}}, J.KV{"text", J.Str{"quiet @Gamma é🙂"}}, J.KV{"want", J.Str{"act"}}])}]))

def check_send(+s: M.State, +label: String) -> IO(Unit):
  do IO<Unit>:
    +st : H.Step = rpc(s, "room:r")
    +body : MB.Body = Q.body(F.outs(st))
    Q.b(label ++ " human room ACK", J.Json.get_bool(Q.reply(F.outs(st)), "ok", False{}), True{})
    Q.n(label ++ " exactly one append", F.appends(F.outs(st)), 1n)
    Q.n(label ++ " no eager changes or RoomPosted", List.length(&2, M.Change, H.Queue.changes(st)), 0n)
    Q.n(label ++ " ACK and append only", List.length(&2, H.Out, F.outs(st)), 2n)
    C.Check.str(label ++ " canonical room conv", ML.Mail.conv(MB.Body.mail(body)), "room:r")
    C.Check.str(label ++ " exact Unicode body", MB.Body.text(body), "quiet @Gamma é🙂")
    C.Check.str(label ++ " person sender", ML.Mail.from(MB.Body.mail(body)), "person")
    C.Check.str(label ++ " room is FYI", ML.Mail.wanted(MB.Body.mail(body)), "fyi")
    C.Check.str(label ++ " participant membership", String.join(ML.Conv.parts(MB.Body.conv(body)), ","), "bot:bb,bot:bc,person")
    +done : M.State = M.State.replay(F.pending(F.outs(st)), s)
    +again : M.State = M.State.replay(F.pending(F.outs(st)), done)
    Q.n(label ++ " committed header folds once", ML.Conv.n(ML.Box.conv(M.State.mail(again), "room:r")), 1n)
    Q.n(label ++ " OFF commit routes no changes", List.length(&2, M.Change, H.Queue.changes(H.Mail.route(M.State.apply(done, M.SettingSet{"mail.on", "off", 11n}), 11n, body))), 0n)
    Q.n(label ++ " OFF commit routes no output", List.length(&2, H.Out, F.outs(H.Mail.route(M.State.apply(done, M.SettingSet{"mail.on", "off", 11n}), 11n, body))), 0n)

def invalid(+label: String, +s: M.State, to: String) -> IO(Unit):
  do IO<Unit>:
    +st : H.Step = rpc(s, to)
    Q.b(label ++ " explicit error", J.Json.get_bool(Q.reply(F.outs(st)), "ok", True{}), False{})
    Q.n(label ++ " no append", F.appends(F.outs(st)), 0n)
    Q.n(label ++ " no mutation", List.length(&2, M.Change, H.Queue.changes(st)), 0n)

def rowids(rows: List<&2, J.Json>, acc: String) -> String:
  match rows:
    case Nil{}: acc
    case h <> t: rowids(t, acc ++ J.Json.get_str(h, "id", "") ++ ",")

def result(r: MS.StoreResult<J.Json>) -> J.Json:
  match r:
    case MS.Stored{j}: j
    case _: J.Null{}

def error(r: MS.StoreResult<J.Json>) -> String:
  match r:
    case MS.StoreError{_, e}: e
    case MS.Missing{_}: "missing"
    case MS.Stored{_}: "success"

def io_page(+home: String, +s: M.State, +before: String, +n: Nat, +want: String, +more: Bool) -> IO(Unit):
  do IO<Unit>:
    +r : MS.StoreResult<J.Json> <- S.MailJob.page("", home, s, "room:r", "", before, n)
    C.Check.str("actual room page " ++ before ++ "/" ++ Nat.show(n), rowids(J.Json.elems(J.Json.get_obj(result(r), "mails")), ""), want)
    Q.b("actual more " ++ before ++ "/" ++ Nat.show(n), J.Json.get_bool(result(r), "more", Bool.not(more)), more)

def index_ids(+u: U.Ui, +bot: String, +kind: String) -> String:
  String.join(ML.Order.slice(U.Mailv.index.order(U.Mailv.index(u), U.Mailv.index.key(bot, kind, "")), 0n, 20n), ",")

def client() -> IO(Unit):
  do IO<Unit>:
    +u0 : U.Ui = U.Ui.scratch_set(U.Ui.init(100n), "@mr.conv", "room:r")
    +u1 : U.Ui = U.Ui.fold([M.RoomSet{"r", "Public room", "bb\nbc", 1n}, M.RoomPosted{"p1", "r", "Beta", "first é🙂\nfull text", 0n, 2n}], u0)
    C.Check.str("canonical OFF aggregate index sees room", index_ids(u1, "*", ""), "room:r")
    C.Check.str("canonical OFF member FYI index sees room", index_ids(u1, "bc", "fyi"), "room:r")
    C.Check.str("canonical OFF Act excludes room", index_ids(u1, "*", "act"), "")
    C.Check.str("canonical OFF aggregate FYI sees room", index_ids(u1, "*", "fyi"), "room:r")
    C.Check.str("room arrival dirties open body", U.Ui.scratch(u1, "@mr.read.dirty"), "1")
    C.Check.str("room has affected conversation key", U.Mailv.change_conv(M.RoomPosted{"p", "r", "Beta", "x", 0n, 3n}), "room:r")
    +re : U.Ui = U.Ui.fold([M.RoomPosted{"p1", "r", "Beta", "first é🙂\nfull text", 0n, 2n}], u1)
    C.Check.str("replayed room post indexed once", index_ids(re, "*", ""), "room:r")
    +renamed : U.Ui = U.Ui.fold([M.RoomSet{"r", "Renamed", "bb", 3n}], U.Mailv.chip(u1, "bc"))
    C.Check.str("removed member has no stale room index", index_ids(renamed, "bc", ""), "")
    C.Check.str("removed member has no stale FYI", index_ids(renamed, "bc", "fyi"), "")
    C.Check.str("remaining member keeps room", index_ids(renamed, "bb", ""), "room:r")
    +on : U.Ui = U.Ui.fold([M.SettingSet{"mail.on", "on", 1n}, M.RoomSet{"r", "Public", "bb", 1n}, M.RoomPosted{"p", "r", "Beta", "waiting disk", 0n, 2n}], u0)
    C.Check.str("ON RoomPosted waits durable body before read", U.Ui.scratch(on, "@mr.read.dirty"), "")
    Q.n("room-only changes do not increase Act count", U.Mailv.act_unread(u1), 0n)
    +params : J.Json = J.Json.obj([J.KV{"bot", J.Str{"*"}}, J.KV{"view_status", J.Str{"fyi"}}, J.KV{"n", J.Num{"100"}}])
    +cache : MV.Cache = U.Mailv.list.result(u1, params, J.Json.obj([]))
    C.Check.str("refresh uses canonical room index", MV.Mv.ids(MV.Mv.rows(cache, MV.Mv.key(MV.Filter{"*", "fyi", "", ""}), 0n, 100n)), "room:r,")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("historical headers newest first", plan(RH.RoomHistory.page(historic(), "room:r", "", 3n)), "p3,p1,/False")
    C.Check.str("mixed first page", plan(RH.RoomHistory.page(mixed(), "room:r", "", 2n)), "f4,p3,/True")
    C.Check.str("cursor can be legacy-only", plan(RH.RoomHistory.page(mixed(), "room:r", "p3", 2n)), "f2,p1,/False")
    C.Check.str("cursor can be file-backed", plan(RH.RoomHistory.page(mixed(), "room:r", "f2", 1n)), "p1,/False")
    C.Check.str("unknown cursor explicit", plan(RH.RoomHistory.page(mixed(), "room:r", "unknown", 2n)), "error")
    C.Check.str("unknown room explicit", plan(RH.RoomHistory.page(mixed(), "room:missing", "", 2n)), "error")
    check_send(initial(), "OFF")
    check_send(M.State.apply(initial(), M.SettingSet{"mail.on", "on", 1n}), "ON")
    invalid("unknown room", initial(), "room:unknown")
    invalid("deleted room", M.State.apply(initial(), M.RoomDeleted{"r", 2n}), "room:r")
    invalid("empty room id", initial(), "room:")
    invalid("malformed target", initial(), "garbage")
    invalid("foreign thread", initial(), "thread:foreign")
    Q.b("MCP remains OFF for room send", MC.Res.good(MC.Mcp.mail.call(40n, initial(), 10n, "a", J.Json.obj([J.KV{"to", J.Str{"room:r"}}, J.KV{"text", J.Str{"never"}}]))), False{})
    client()
    +home : String <- ST.home()
    io_page(home, historic(), "", 3n, "p3,p1,", False{})
    +hist : MS.StoreResult<J.Json> <- S.MailJob.page("", home, historic(), "room:r", "", "", 3n)
    C.Check.str("historic complete Unicode source", J.Json.get_str(Maybe.default(&2, J.Json, List.head(&2, J.Json, J.Json.elems(J.Json.get_obj(result(hist), "mails"))), J.Null{}), "text", ""), "third 漢字")
    +a : MS.StoreResult<Bool> <- MS.MailStore.append(home, filed(initial(), "f2", "second 🧪", 2n))
    C.Check.str("file append f2", ST.status(Bool, a), "ok")
    +b : MS.StoreResult<Bool> <- MS.MailStore.append(home, filed(initial(), "f4", "fourth\nUnicode 👩‍🔬", 4n))
    C.Check.str("file append f4", ST.status(Bool, b), "ok")
    +dup : MS.StoreResult<Bool> <- MS.MailStore.append(home, filed(initial(), "p3", "third 漢字", 3n))
    C.Check.str("same legacy ID also stored", ST.status(Bool, dup), "ok")
    io_page(home, mixed(), "", 2n, "f4,p3,", True{})
    io_page(home, mixed(), "p3", 2n, "f2,p1,", False{})
    io_page(home, mixed(), "f4", 1n, "p3,", True{})
    io_page(home, mixed(), "f2", 1n, "p1,", False{})
    io_page(home, mixed(), "p1", 2n, "", False{})
    +exact : MS.StoreResult<MB.Body> <- MS.MailStore.exact(home, "room:r", "f2")
    C.Check.str("indexed exact lookup preserves body", ST.status(MB.Body, exact), "ok")
    +missing : MS.StoreResult<J.Json> <- S.MailJob.page("", home, M.State.apply(initial(), MB.Body.header(MB.Body.mail(filed(initial(), "absent", "no source", 5n)))), "room:r", "", "", 1n)
    Q.b("header without canonical or disk body reports diagnostic", String.contains(error(missing), "mail body unavailable"), True{})
    +badcursor : MS.StoreResult<J.Json> <- S.MailJob.page("", home, mixed(), "room:r", "", "unknown", 3n)
    Q.b("actual unknown cursor diagnostic", String.contains(error(badcursor), "cursor"), True{})
    +nonroom : MS.StoreResult<J.Json> <- S.MailJob.page("", home, mixed(), "not-room", "", "", 3n)
    C.Check.str("non-room missing behavior unchanged", rowids(J.Json.elems(J.Json.get_obj(result(nonroom), "mails")), ""), "")
