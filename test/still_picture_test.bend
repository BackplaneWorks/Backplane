import Base
import ./check.bend as C
import ./sidebar_wire_test.bend as F
import ../src/core/json.bend as J
import ../src/core/thin-live.bend as TL
import ../src/core/i18n.bend as I
import ../src/server/thin-view.bend as V
import ../src/server/thin-actor.bend as A
import ../src/server/thin-still-picture.bend as S

law still_disabled:
  for key: String
  for res: J.Json
  {S.Picture.bounds(False{}, key, res) == False{} : Bool}
def still_disabled(key, res): {==}
law still_miss:
  for res: J.Json
  {S.Picture.select(False{}, res) == None{} : Maybe<&2, J.Json>}
def still_miss(res): {==}
law still_exact:
  for +res: J.Json
  {S.Picture.select(True{}, res) == Some{res} : Maybe<&2, J.Json>}
def still_exact(res): {==}

def request() -> J.Json:
  J.Json.obj([J.KV{"target", V.Target.json("thread", "self")}, J.KV{"action", J.Str{""}}, J.KV{"session", J.Json.obj([J.KV{"drafts", J.Json.obj([J.KV{"self", J.Str{"日本語 IME"}}])}])}])
def stream() -> TL.Stream:
  TL.Stream.append(TL.Stream.blank(), "fresh streamed text", 1n)
def yes(b: Bool) -> String: Bool.pick(String, b, "yes", "no")
def show(m: Maybe<&2, J.Json>) -> String:
  match m:
    case None{}: "none"
    case Some{v}: J.Json.show(v)
def read(update: Bool, key: String, clock: Nat, stream: TL.Stream) -> String:
  show(S.Picture.read(update, S.Picture{"owned", 10n, F.picture()}, key, clock, stream))
def allowed(web: Bool, p: J.Json, res: J.Json, stream: TL.Stream) -> String:
  yes(S.Picture.allowed(web, p, res, stream, "owned"))
def key(seq: Nat, info: String, p: J.Json, anchor: Nat, view: String, thread: String) -> String:
  A.Memo.key(seq, info, p, anchor, view, True{}, thread)
def same(a: String, b: String) -> String: yes(String.eq(a, b))
def saved.actor(r: A.Rendered) -> A.Actor:
  match r:
    case A.Rendered{a, _}: a
def saved() -> A.Actor:
  saved.actor(A.Rendered.remember.clock(A.Rendered{F.seed(), F.picture()}, "owned", 10001n, 10n, TL.Stream.blank(), True{}, request()))
def count(ms: List<&2, A.Memo>) -> Nat:
  match ms:
    case Nil{}: 0n
    case A.Still{_} <> rest: Nat.add(1n, count(rest))
    case _ <> rest: count(rest)
def reused(a: A.Actor) -> String:
  show(A.Memo.reply.clock(A.Sidebar.memos(a), True{}, "owned", 10002n, 10n, True{}, TL.Stream.blank(), "self", "ep", V.Target.json("thread", "self"), I.En{}))
def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("ordinary web-thread no-stream picture eligible", allowed(True{}, request(), F.picture(), TL.Stream.blank()), "yes")
    C.Check.str("mobile does not acquire still picture", allowed(False{}, request(), F.picture(), TL.Stream.blank()), "no")
    C.Check.str("action never acquires still picture", allowed(True{}, V.Obj.put(request(), "action", J.Str{"send"}), F.picture(), TL.Stream.blank()), "no")
    C.Check.str("bot never acquires still picture", allowed(True{}, V.Obj.put(request(), "target", V.Target.json("bot", "self")), F.picture(), TL.Stream.blank()), "no")
    C.Check.str("stream never acquires still picture", allowed(True{}, request(), F.picture(), stream()), "no")
    C.Check.str("commands prevent reuse", allowed(True{}, request(), V.Obj.put(F.picture(), "cmds", J.Json.arr([J.Str{"command"}])), TL.Stream.blank()), "no")
    C.Check.str("unready result prevents reuse", allowed(True{}, request(), V.Obj.put(F.picture(), "remoteReady", J.Flag{False{}}), TL.Stream.blank()), "no")
    C.Check.str("navigation result prevents reuse", allowed(True{}, request(), V.Obj.put(F.picture(), "nextTarget", V.Target.json("thread", "other")), TL.Stream.blank()), "no")
    C.Check.str("exact clock and complete key return exact full reply", read(True{}, "owned", 10n, TL.Stream.blank()), J.Json.show(F.picture()))
    C.Check.str("changed key misses", read(True{}, "other", 10n, TL.Stream.blank()), "none")
    C.Check.str("a clock in the same view-clock step hits", read(True{}, "owned", 11n, TL.Stream.blank()), J.Json.show(F.picture()))
    C.Check.str("the next view-clock step misses", read(True{}, "owned", 30n, TL.Stream.blank()), "none")
    # (0 and 10 share the 30 s step: a picture saved a step later, read at 10)
    C.Check.str("an earlier step misses", show(S.Picture.read(True{}, S.Picture{"owned", 40n, F.picture()}, "owned", 10n, TL.Stream.blank())), "none")
    C.Check.str("non-update misses", read(False{}, "owned", 10n, TL.Stream.blank()), "none")
    C.Check.str("new live stream misses", read(True{}, "owned", 10n, stream()), "none")
    C.Check.str("connection stores one still picture", Nat.show(count(A.Sidebar.memos(saved()))), "1")
    C.Check.str("surviving own actor can reuse", reused(saved()), J.Json.show(F.picture()))
    C.Check.str("fresh actor cannot borrow still picture", reused(F.seed()), "none")
    C.Check.str("dirty actor evicts picture", reused(A.Actor.mark_dirty(saved())), "none")
    C.Check.str("targeted info evicts picture", reused(A.Actor.info(saved(), J.Json.obj(Nil{}))), "none")
    C.Check.str("sheet notification evicts picture", reused(A.Actor.sheets(saved(), J.Json.obj(Nil{}))), "none")
    C.Check.str("full sequence remains in picture key", same(key(1n, "info", request(), 0n, "view", "self"), key(2n, "info", request(), 0n, "view", "self")), "no")
    C.Check.str("equal-length current info differs", same(key(1n, "old", request(), 0n, "view", "self"), key(1n, "new", request(), 0n, "view", "self")), "no")
    C.Check.str("anchor remains in picture key", same(key(1n, "info", request(), 0n, "view", "self"), key(1n, "info", request(), 1n, "view", "self")), "no")
    C.Check.str("view remains in picture key", same(key(1n, "info", request(), 0n, "view", "self"), key(1n, "info", request(), 0n, "other-view", "self")), "no")
    C.Check.str("draft remains in picture key", same(key(1n, "info", request(), 0n, "view", "self"), key(1n, "info", V.Obj.put(request(), "session", J.Str{"new draft"}), 0n, "view", "self")), "no")
