import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/thin.bend as T
import ../src/core/thin-chat.bend as TC
import ../src/core/thin-wire.bend as W
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/storedb.bend as S
import ../src/core/ubop.bend as UO
import ../src/server/thinstore.bend as TS
import ../src/server/thin-view.bend as V
import ../src/server/db.bend as D

# Authored handoff fixture, NOT run by the packet author. It opens only a
# fresh temporary SQLite store; no hub, provider, live input or recovery.
def yes(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def seq.down(+n: Nat, +low: Nat) -> List<&2, String>:
  match n:
    case 0n: Nil{}
    case 1n+k: Bool.pick(List<&2, String>, Nat.is_ge(1n+k, low), Nat.show(1n+k) <> seq.down(k, low), Nil{})

def posts(+n: Nat, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match n:
    case 0n: acc
    case 1n+k: posts(k, M.MessagePosted{"bub-chat", "m" ++ Nat.show(1n+k), M.Assistant{}, "meaningful history", 7n} <> acc)

def receipts(+n: Nat, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match n:
    case 0n: acc
    case 1n+k: receipts(k, M.ActivityLogged{"bub-chat", "ride:" ++ Nat.show(1n+k), "info", "ride", "wording is irrelevant", 7n} <> acc)

def guards() -> List<&2, M.Change>:
  [M.ActivityLogged{"bub-chat", "ride:warning", "warn", "ride", "warning", 7n},
   M.ActivityLogged{"bub-chat", "ride:error", "error", "ride", "failure", 7n},
   M.ActivityLogged{"bub-chat", "ride:session", "info", "session", "OpenCode lost context", 7n},
   M.ActivityLogged{"bub-chat", "lost-context", "info", "lost", "lost context notice", 7n},
   M.ActivityLogged{"bub-chat", "custom-ride", "info", "ride", "nonproducer", 7n},
   M.ActivityLogged{"bub-chat", "Ride:uppercase", "info", "ride", "case sensitive prefix", 7n},
   M.MessagePosted{"elsewhere", M.Link.id("bub-chat", "47"), M.User{}, "actual outgoing thread link", 7n},
   M.MessagePosted{"bub-chat", "literal-user", M.User{}, "Delivered with a new turn: 1 waiting message(s)", 7n},
   M.MessagePosted{"bub-chat", "ride:assistant", M.Assistant{}, "same words and id are still a message", 7n}]

def body() -> List<&2, M.Change>:
  List.append(&2, M.Change, posts(40n, Nil{}), List.append(&2, M.Change, guards(), receipts(96n, Nil{})))

def metadata() -> List<&2, M.Change>:
  [M.ThreadCreated{"bub-chat", "", "Bub", M.Local{}, "", "", "codex", "", 1n},
   M.BotSet{"b1790292962-8017", "Bub", "bub-chat", 0n, "", 1n},
   M.UberSet{"b1790292962-8017", "fixture-uber", 2n}, M.SettingSet{"machine.id", "here", 2n}]

def changes() -> List<&2, M.Change>:
  List.append(&2, M.Change, metadata(), body())

def request() -> J.Json:
  J.Json.obj([J.KV{"target", V.Target.json("bot", "b1790292962-8017")}, J.KV{"anchor", J.Json.obj(Nil{})}])

def page(+i: T.Index, +before: Nat) -> TS.Page:
  +p = TC.Chat.index(True{}, i, "bub-chat", before, 32n)
  TS.Page{p, T.Index.page(p, "bub-chat", before, 32n)}

def show(+p: TS.Page) -> String:
  J.Json.show(V.Page.window(TS.Page.index(p), "bub-chat", request(), "ep", TS.Page.ids(p)))

def entries.ids(es: List<&2, M.Entry>) -> String:
  match es:
    case Nil{}: ""
    case e <> t: M.Idx.id(e) ++ ";" ++ entries.ids(t)

def predicate(es: List<&2, M.Entry>, ok: Bool) -> Bool:
  match es:
    case Nil{}: ok
    case +e <> t: predicate(t, Bool.and(ok, String.eq(yes(TC.Chat.receipt(e)), yes(U.Page.delivery.receipt(e)))))

def lines(cs: List<&2, M.Change>) -> List<&2, S.LL>:
  match cs:
    case Nil{}: Nil{}
    case c <> t: S.LL{20n, True{}, Some{c}} <> lines(t)

def read.error(r: V.MessageRead) -> String:
  match r:
    case V.MessageRead{error, _, _}: error

def receipt.request() -> J.Json:
  J.Json.obj([J.KV{"entryId", J.Str{"ride:96"}}])

def table.of(k: String) -> String:
  Maybe.default(&2, String, List.head(&2, String, String.split(k, '\u{1f}')), "")

def items(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> List<&2, List<&2, String>>:
  match xs:
    case Nil{}: Nil{}
    case (+k, v) <> t: S.Sql.of(S.Put{table.of(k), String.split(v, '\u{1f}')}) <> items(t)

def dir.of(r: Result<&1, &1, U32 & String, U32 & (String & String)>) -> IO(String):
  match r:
    case Done{(_, (out, _))}: IO.pure(String, String.trim(out))
    case Fail{(_, e)}: IO.die(String, 1, "thin_chat_receipts_test: " ++ e)

def parity(anchors: List<&2, Nat>, +h: String, +i: T.Index) -> IO(Unit):
  match anchors:
    case Nil{}: IO.pure(Unit, Unit{})
    case +b <> t:
      do IO<Unit>:
        +p : TS.Page <- TS.Ts.page.chat(True{}, h, "bub-chat", b, 32n, T.Index.recent_only(i))
        C.Check.str("SQL/memory window+lookahead+cursor at " ++ Nat.show(b), show(p), show(page(i, b)))
        C.Check.str("SQL/memory retained entries at " ++ Nat.show(b),
          entries.ids(T.Page.entries(TS.Page.ids(p), TS.Page.index(p), Nil{})),
          entries.ids(T.Page.entries(TS.Page.ids(page(i, b)), TS.Page.index(page(i, b)), Nil{})))
        parity(t, h, i)

def main() -> IO(Unit):
  do IO<Unit>:
    +st : M.State = M.State.replay(changes(), M.State.empty())
    +i : T.Index = T.Index.of(st)
    +first : TS.Page = page(i, 0n)
    +second : TS.Page = page(i, 18n)
    C.Check.str("same exact entry predicate as loaded/delta web pager", yes(predicate(M.State.entries.all(st), True{})), "yes")
    C.Check.str("bot chat enabled", yes(TC.Chat.enabled(st, "bub-chat")), "yes")
    C.Check.str("actual bot target resolves to source conversation", V.Target.thread(st, J.Json.get_obj(request(), "target")), "bub-chat")
    C.Check.str("ordinary thread unchanged", yes(TC.Chat.enabled(st, "elsewhere")), "no")
    C.Check.str("empty route unchanged", yes(TC.Chat.enabled(st, "")), "no")
    +remote : M.State = M.State.replay([M.UberOp{"fixture-uber", "there", 1n, UO.Ub.stamp.make(3n, 0n, "there"), UO.Ub.b.bind("there", "remote-bot", "remote-chat", "Bub"), 3n}, M.SettingSet{"uber.link.fixture-uber.there", "peer", 4n}], st)
    C.Check.str("canonical Uber replica route enabled", yes(TC.Chat.enabled(remote, "peer~remote-chat")), "yes")
    C.Check.str("96 newest receipts do not consume 32 slots", String.join(TS.Page.ids(first), ","), String.join(seq.down(49n, 18n), ","))
    C.Check.str("only 33 rows materialized including lookahead", Nat.show(List.length(&2, T.Row, Map.values(&2, T.Row, T.Index.rows(TS.Page.index(first))))), "33")
    +w : J.Json = V.Page.window(TS.Page.index(first), "bub-chat", request(), "ep", TS.Page.ids(first))
    C.Check.str("visible older row sets hasMore", yes(J.Json.get_bool(w, "hasMore", False{})), "yes")
    # the older page starts halfway up this one (thin-view.bend's Page.mid):
    # the 16th source ordinal of 49..18
    C.Check.str("cursor round trip retains the SOURCE ordinal halfway up", Nat.show(Maybe.default(&2, Nat, W.Cursor.read(J.Json.get_str(w, "older", ""), "ep", J.Json.get_obj(request(), "target"), ""), 0n)), "34")
    C.Check.str("second page has no duplicate first-page rows", String.join(TS.Page.ids(second), ","), String.join(seq.down(17n, 1n), ","))
    C.Check.str("older receipt anchor is still a source ordinal", String.join(TS.Page.ids(page(i, 100n)), ","), String.join(TS.Page.ids(first), ","))
    C.Check.str("no empty tail cursor", J.Json.get_str(V.Page.window(TS.Page.index(second), "bub-chat", request(), "ep", TS.Page.ids(second)), "older", "bad"), "")
    C.Check.str("zero requested page still bounded empty", Nat.show(List.length(&2, String, T.Index.page(TC.Chat.window(i, "bub-chat", 0n, 0n), "bub-chat", 0n, 0n))), "0")
    C.Check.str("ordinary adapter uses original raw page", String.join(T.Index.page(TC.Chat.index(False{}, i, "bub-chat", 0n, 32n), "bub-chat", 0n, 32n), ","), String.join(seq.down(145n, 114n), ","))
    C.Check.str("source index still holds all receipts", Nat.show(T.Index.n(i)), "145")
    C.Check.str("source history still holds all receipts", Nat.show(List.length(&2, M.Entry, M.State.entries.all(st))), "145")
    C.Check.str("direct memory source read still names hidden receipt", read.error(V.Message.read(i, "bub-chat", receipt.request(), "ep", T.Index.page(i, "bub-chat", 0n, 32n), st)), "")
    +only : T.Index = T.Index.changes(receipts(96n, Nil{}), T.Index.empty())
    C.Check.str("receipt-only history gives no rows or empty spacer", String.join(TS.Page.ids(page(only, 0n)), ","), "")
    +new : T.Index = T.Index.change(i, M.ActivityLogged{"bub-chat", "ride:delta", "info", "ride", "new receipt", 8n})
    C.Check.str("new receipt does not shift loaded page/cursor", show(page(new, 0n)), show(first))
    +notice : T.Index = T.Index.change(new, M.ActivityLogged{"bub-chat", "ride:new-session", "info", "session", "OpenCode reset", 9n})
    C.Check.str("new session notice remains newest", Maybe.default(&2, String, List.head(&2, String, TS.Page.ids(page(notice, 0n))), ""), "147")
    made : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("mktemp", ["-d"], "", 4096, 10000)
    +dir : String <- dir.of(made)
    +opened : String <- D.Db.open(dir ++ "/store.db")
    +h : String = String.drop(opened, 3n)
    schema : String <- D.Db.exec(h, S.Sdb.schema())
    C.Check.str("temp schema", schema, "ok")
    +mat : S.Mat = S.Mat.replay(lines(changes()), S.Mat.empty())
    written : String <- D.Db.script(h, items(Map.to_list(&2, String, S.Mat.facts(mat))))
    C.Check.str("fixture store written", String.take(written, 2n), "ok")
    parity([0n, 18n, 100n, 1n, 999n], h, i)
    +raw : TS.Page <- TS.Ts.page.chat(False{}, h, "bub-chat", 0n, 32n, T.Index.recent_only(i))
    C.Check.str("ordinary SQL retains receipts", String.join(TS.Page.ids(raw), ","), String.join(seq.down(145n, 114n), ","))
    +whole : TS.Page <- TS.Ts.whole(h, "bub-chat", 0n, "ride:96", raw)
    C.Check.str("direct stored source read still names hidden receipt", read.error(V.Message.read(TS.Page.index(whole), "bub-chat", receipt.request(), "ep", TS.Page.ids(raw), st)), "")
    +sent : TS.Page <- TS.Ts.page.chat(False{}, h, "elsewhere", 0n, 32n, T.Index.recent_only(i))
    C.Check.str("actual thread link remains in destination history", String.join(TS.Page.ids(sent), ","), "47")
    +after : S.Mat = S.Mat.replay(lines([M.ActivityLogged{"bub-chat", "ride:delta", "info", "ride", "new receipt", 8n},
      M.ActivityLogged{"bub-chat", "ride:new-session", "info", "session", "OpenCode reset", 9n}]), mat)
    updated : String <- D.Db.script(h, items(Map.to_list(&2, String, S.Mat.facts(after))))
    C.Check.str("new receipt and session stored without discarding source", String.take(updated, 2n), "ok")
    parity([0n, 18n], h, notice)
    closed : String <- D.Db.close(h)
    removed : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("rm", ["-rf", dir], "", 4096, 10000)
    return Unit{}
