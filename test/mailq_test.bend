import Base
import ./check.bend as C
import ../src/core/storedb.bend as S
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/mail.bend as ML
import ../src/core/mailroom_history.bend as RH
import ../src/server/thinstore.bend as TS
import ../src/server/db.bend as D

# The mail box's header reads from the store (server/thinstore.bend's Ms.*
# over core/mailq.bend) answer what ML.Box answers from memory: every
# conversation's headers in Headers.add's order (late replays included),
# the newest header, a room's page after every cursor at several sizes,
# the room's post texts, and whether an id was seen. The store is S1's
# pure materializer (S.Mat) written to SQLite.

def conv.of.k(+r: Nat, +i: Nat) -> String:
  match r:
    case 0n: "c1"
    case 1n: "c2"
    case 2n: "c1"
    case _: "c3"

def conv.of(+i: Nat) -> String:
  conv.of.k(Nat.mod(i, 4n), i)

# times that go back and forth, so some headers arrive late
def at.of(+i: Nat) -> Nat:
  Nat.add(100n, Nat.mod(Nat.mul(i, 37n), 23n))

def mail.of(+i: Nat) -> M.Change:
  M.MailPosted{"m" ++ Nat.show(Nat.mod(i, 41n)), conv.of(i), "person", "bot:b1", "note", "", Bool.pick(String, Nat.is_eq(Nat.mod(i, 3n), 0n), "act", "fyi"), "head " ++ Nat.show(i), i, at.of(i)}

def post.of(+i: Nat) -> M.Change:
  M.RoomPosted{"p" ++ Nat.show(Nat.mod(i, 29n)), Bool.pick(String, Nat.is_eq(Nat.mod(i, 2n), 0n), "r1", "r2"), "b1", "post text " ++ Nat.show(i), 0n, at.of(i)}

def changes.go(+n: Nat, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match n:
    case 0n: acc
    case 1n+k: changes.go(k, mail.of(k) <> post.of(k) <> acc)

def changes() -> List<&2, M.Change>:
  M.ConvSet{"c1", "Subj", "pair", "bot:b1\nperson", "p1", "", 1n} <> M.ConvSet{"c9", "Empty", "pair", "bot:b1\nperson", "p1", "", 1n} <> changes.go(60n, Nil{})

def lines(cs: List<&2, M.Change>) -> List<&2, S.LL>:
  match cs:
    case Nil{}: Nil{}
    case c <> t: S.LL{20n, True{}, Some{c}} <> lines(t)

def table.of(+k: String) -> String:
  Maybe.default(&2, String, List.head(&2, String, String.split(k, '\u{1f}')), "")

def items(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> List<&2, List<&2, String>>:
  match xs:
    case Nil{}: Nil{}
    case (+k, +v) <> t: S.Sql.of(S.Put{table.of(k), String.split(v, '\u{1f}')}) <> items(t)

def dir.of(r: Result<&1, &1, U32 & String, U32 & (String & String)>) -> IO(String):
  match r:
    case Done{(_, (out, _))}: IO.pure(String, String.trim(out))
    case Fail{(_, e)}: IO.die(String, 1, "mailq_test: mktemp failed: " ++ e)

type Acc is Data:
  Acc{n: Nat, bad: Nat, first: String}

def Acc.see.k(same: Bool, +a: Acc, name: String, got: String, want: String) -> Acc:
  match same a:
    case True{} Acc{n, b, f}: Acc{1n+n, b, f}
    case False{} Acc{n, b, f}: Acc{1n+n, 1n+b, Bool.pick(String, String.is_empty(f), name ++ "\n got  " ++ got ++ "\n want " ++ want, f)}

def Acc.see(+a: Acc, name: String, +got: String, +want: String) -> Acc:
  Acc.see.k(C.Check.same(got, want), a, name, got, want)

def Acc.report(+name: String, a: Acc) -> IO(Unit):
  match a:
    case Acc{n, 0n, _}: IO.print("ok " ++ name ++ " (" ++ Nat.show(n) ++ " compared)")
    case Acc{n, b, f}: IO.print("FAIL " ++ name ++ ": " ++ Nat.show(b) ++ " of " ++ Nat.show(n) ++ " differ; first: " ++ f)

def show.mail(+m: ML.Mail) -> String:
  match m:
    case ML.Mail{id, c, f, t, k, r, w, h, sz, at}:
      String.join([id, c, f, t, k, r, w, h, Nat.show(sz), Nat.show(at)], "|")

def show.mails(ms: List<&2, ML.Mail>) -> String:
  match ms:
    case Nil{}: ""
    case +m <> t: show.mail(m) ++ ";" ++ show.mails(t)

def show.page(p: Maybe<&2, RH.RoomPage>) -> String:
  match p:
    case None{}: "none"
    case Some{RH.RoomPage{hs, more}}: show.mails(hs) ++ Bool.pick(String, more, " more", " end")

def show.texts(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> String:
  match xs:
    case Nil{}: ""
    case (k, v) <> t: k ++ "=" ++ v ++ ";" ++ show.texts(t)

def sizes() -> List<&2, Nat>:
  [0n, 1n, 2n, 5n, 300n]

def pages.n(ns: List<&2, Nat>, +h: String, +st: M.State, +conv: String, +before: String, +acc: Acc) -> IO(Acc):
  match ns:
    case Nil{}: IO.pure(Acc, acc)
    case +n <> t:
      do IO<Acc>:
        +want : Maybe<&2, RH.RoomPage> = RH.RoomHistory.page(st, conv, before, n)
        +got : Maybe<&2, RH.RoomPage> <- TS.Ms.room.page(h, conv, before, n)
        +a : Acc = Acc.see(acc, "page " ++ conv ++ " after " ++ before ++ " n " ++ Nat.show(n), show.page(got), show.page(want))
        pages.n(t, h, st, conv, before, a)

# after every header (and none, and an unknown id)
def pages(cursors: List<&2, String>, +h: String, +st: M.State, +conv: String, +acc: Acc) -> IO(Acc):
  match cursors:
    case Nil{}: IO.pure(Acc, acc)
    case +c <> t:
      do IO<Acc>:
        a : Acc <- pages.n(sizes(), h, st, conv, c, acc)
        pages(t, h, st, conv, a)

def ids(ms: List<&2, ML.Mail>) -> List<&2, String>:
  match ms:
    case Nil{}: Nil{}
    case +m <> t: ML.Mail.id(m) <> ids(t)

def convs(cs: List<&2, String>, +h: String, +st: M.State, +acc: Acc) -> IO(Acc):
  match cs:
    case Nil{}: IO.pure(Acc, acc)
    case +conv <> t:
      do IO<Acc>:
        +box : ML.Box = M.State.mail(st)
        +hs : List<&2, ML.Mail> <- TS.Ms.headers(h, conv, "", 1000n)
        +a1 : Acc = Acc.see(acc, "headers " ++ conv, show.mails(hs), show.mails(ML.Box.headers(box, conv)))
        +nw : ML.Mail <- TS.Ms.newest(h, conv)
        +a2 : Acc = Acc.see(a1, "newest " ++ conv, show.mail(nw), show.mail(ML.Box.newest(box, conv)))
        a3 : Acc <- pages("" <> "nope" <> ids(ML.Box.headers(box, conv)), h, st, conv, a2)
        +src : Map<&2, String> <- TS.Ms.room.sources(h, conv, ML.Box.headers(box, conv))
        +a4 : Acc = Acc.see(a3, "sources " ++ conv, show.texts(Map.to_list(&2, String, src)), show.texts(Map.to_list(&2, String, RH.RoomHistory.sources(st, conv, ML.Box.headers(box, conv)))))
        convs(t, h, st, a4)

def has(xs: List<&2, String>, +h: String, +st: M.State, +acc: Acc) -> IO(Acc):
  match xs:
    case Nil{}: IO.pure(Acc, acc)
    case +id <> t:
      do IO<Acc>:
        +got : Bool <- TS.Ms.has(h, id)
        has(t, h, st, Acc.see(acc, "has " ++ id, Bool.pick(String, got, "yes", "no"), Bool.pick(String, ML.Box.has(M.State.mail(st), id), "yes", "no")))

def main() -> IO(Unit):
  do IO<Unit>:
    made : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("mktemp", ["-d"], "", 4096, 10000)
    +dir : String <- dir.of(made)
    +h0 : String <- D.Db.open(dir ++ "/store.db")
    +h : String = String.drop(h0, 3n)
    sc : String <- D.Db.exec(h, S.Sdb.schema())
    C.Check.str("schema", sc, "ok")
    +m : S.Mat = S.Mat.replay(lines(changes()), S.Mat.empty())
    w : String <- D.Db.script(h, items(List.append(&2, Sigma<&2, &2, String, _ => String>, Map.to_list(&2, String, S.Mat.facts(m)), Map.to_list(&2, String, S.Mat.recs(m)))))
    C.Check.str("store written", String.take(w, 2n), "ok")
    +st : M.State = M.State.replay(changes(), M.State.empty())
    a1 : Acc <- convs(["c1", "c2", "c3", "c9", "room:r1", "room:r2", "room:zz", "zz"], h, st, Acc{0n, 0n, ""})
    Acc.report("headers, newest, room pages and sources", a1)
    a2 : Acc <- has(["m0", "m7", "m40", "m41", "p3", "p28", "zz", ""], h, st, Acc{0n, 0n, ""})
    Acc.report("ids seen", a2)
    cl : String <- D.Db.close(h)
    rm : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("rm", ["-rf", dir], "", 4096, 10000)
    return Unit{}
