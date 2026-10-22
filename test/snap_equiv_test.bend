import Base
import ./check.bend as C
import ../src/core/storedb.bend as S
import ../src/core/snapshot.bend as SN
import ../src/core/model.bend as M
import ../src/core/mail.bend as ML
import ../src/core/bot.bend as BT
import ../src/core/json.bend as J
import ../src/core/codec.bend as CD
import ./snapequiv_lib.bend as Q

# The hot boot equals the full replay (S3): the store made of the demo
# log's first n lines (S1's pure materializer), read back by
# snapshot.bend and then folded with the rest, dumps every record the
# whole log's fold dumps, per domain; and what the load derives (entries
# filed by thread, the mail box's counts, orders and headers, room posts)
# is what the fold made. Then the same through the hub's own path: the
# demo log written as HOME/events.jsonl in a temporary folder, store.db
# built from its first k lines and read back by Hot.load, compared per
# domain with the whole log folded (test/snapequiv_lib.bend);
# test/native/snap_equiv.bend does that on a copy of a real home.

def changes(ls: List<&2, S.LL>) -> List<&2, M.Change>:
  match ls:
    case Nil{}:
      Nil{}
    case S.LL{_, _, Some{c}} <> t:
      c <> changes(t)
    case S.LL{_, _, None{}} <> t:
      changes(t)

def fold() -> M.State:
  M.State.replay(S.Demo.changes(), M.State.empty())

def boot(+n: Nat) -> M.State:
  SN.Snap.boot(S.Mat.replay(List.take(&2, S.LL, S.Demo.log(), n), S.Mat.empty()), changes(List.drop(&2, S.LL, S.Demo.log(), n)))

# the same with no entries loaded at all (the hub loads a window): the
# tail's ProviderSet still counts the thread's entries as the store has them
def bare(r: SN.SRows) -> SN.SRows:
  match r:
    case SN.SRows{t}:
      SN.SRows{Map.set(&2, List<&2, List<&2, String>>, t, "entry", Nil{})}

def boot.bare.k(m: S.Mat, tail: List<&2, M.Change>) -> M.State:
  match m:
    case S.Mat{_, +fs, +rs, _, _, _}:
      +rows = bare(SN.Snap.rows(fs, rs))
      +s0 = SN.Snap.state(rows)
      SN.Snap.replay(tail, s0, SN.Snap.counts(SN.Snap.rows(fs, rs)), s0)

def boot.bare(+n: Nat) -> M.State:
  boot.bare.k(S.Mat.replay(List.take(&2, S.LL, S.Demo.log(), n), S.Mat.empty()), changes(List.drop(&2, S.LL, S.Demo.log(), n)))

# per domain: "<table> <equal>/<diff>" for the rows of either dump
def dom(+k: String) -> String:
  Maybe.default(&2, String, List.head(&2, String, String.split(k, '\u{1f}')), "")

def Cnt.get(r: Map<&2, String> & String) -> String:
  (_, v) = r
  v

def Cnt.bump(r: Map<&2, String> & String, +k: String, same: Bool) -> Map<&2, String>:
  (m, v) = r
  Map.set(&2, String, m, k, v ++ Bool.pick(String, same, "=", "x"))

def tally(xs: List<&2, Sigma<&2, &2, String, _ => String>>, +other: Map<&2, String>, acc: Map<&2, String>) -> Map<&2, String>:
  match xs:
    case Nil{}:
      acc
    case (+k, +v) <> t:
      +d = dom(k)
      tally(t, other, Cnt.bump(Map.get(String, "", acc, d), d, String.eq(Cnt.get(Map.get(String, "\u{0}", other, k)), v)))

def marks.n(+s: String, +c: Char, +n: Nat) -> Nat:
  match s:
    case SNil{}:
      n
    case SCon{+h, t}:
      marks.n(t, c, Bool.pick(Nat, Char.is_eq(h, c), 1n+n, n))

def show.rows(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> String:
  match xs:
    case Nil{}:
      ""
    case (+k, +v) <> t:
      k ++ " " ++ Nat.show(marks.n(v, '=', 0n)) ++ "/" ++ Nat.show(marks.n(v, 'x', 0n)) ++ "\n" ++ show.rows(t)

# both ways: a row only one side has is a diff
def per.domain(+a: Map<&2, String>, +b: Map<&2, String>) -> String:
  show.rows(Map.to_list(&2, String, tally(Map.to_list(&2, String, b), a, tally(Map.to_list(&2, String, a), b, Map.new(&2, String)))))

def dump.show.rows(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> String:
  match xs:
    case Nil{}:
      ""
    case (k, v) <> t:
      k ++ " => " ++ v ++ "\n" ++ dump.show.rows(t)

def dump(+s: M.State) -> String:
  dump.show.rows(Map.to_list(&2, String, S.Tab.dump(s)))

def ids(es: List<&2, M.Entry>) -> String:
  match es:
    case Nil{}:
      ""
    case +e <> t:
      M.Idx.id(e) ++ "@" ++ M.Idx.thread(e) ++ " " ++ ids(t)

def heads(ms: List<&2, ML.Mail>) -> String:
  match ms:
    case Nil{}:
      ""
    case +m <> t:
      ML.Mail.id(m) ++ " " ++ heads(t)

def posts(ps: List<&2, BT.Post>) -> String:
  match ps:
    case Nil{}:
      ""
    case BT.Post{i, _, _, _, _, _} <> t:
      i ++ " " ++ posts(t)

# what the load derives, as text
def derived(+s: M.State) -> String:
  +b = M.State.mail(s)
  "mine t1: " ++ ids(M.State.mine(s, "t1")) ++ "\nmine t2: " ++ ids(M.State.mine(s, "t2")) ++ "\nlinks: " ++ ids(M.State.links(s))
    ++ "\nunread b1: " ++ Nat.show(ML.Box.unread(b, "b1")) ++ " total " ++ Nat.show(ML.Box.unread_all(b))
    ++ "\nheaders c1: " ++ heads(ML.Box.headers(b, "c1")) ++ "\nhas ml1: " ++ Bool.show(ML.Box.has(b, "ml1"))
    ++ "\norder *: " ++ String.join(ML.Order.slice(ML.Box.order_of(b, "*"), 0n, 10n), " ") ++ "\norder b1: " ++ String.join(ML.Order.slice(ML.Box.order_of(b, "b1"), 0n, 10n), " ")
    ++ "\nposts: " ++ posts(BT.World.posts(M.State.world.of(s), "r1", 10n)) ++ "\nseq: " ++ Nat.show(M.State.seq(s))

def at(+n: Nat) -> IO(Unit):
  do IO<Unit>:
    C.Check.str("hot boot from line " ++ Nat.show(n) ++ ": records", dump(boot(n)), dump(fold()))
    C.Check.str("hot boot from line " ++ Nat.show(n) ++ ": derived", derived(boot(n)), derived(fold()))
    C.Check.str("hot boot from line " ++ Nat.show(n) ++ " with no entries: records", dump(boot.bare(n)), dump(fold()))

# the demo log as file lines: each change, then a torn write's fragment
# and a line from a newer hub (as S.Demo.log has them)
def Demo.text(cs: List<&2, M.Change>) -> List<&2, String>:
  match cs:
    case Nil{}:
      ["{\"$\":\"Torn", "{\"$\":\"FromALaterHub\",\"at\":30}"]
    case c <> t:
      J.Json.show(CD.Change.encode(c)) <> Demo.text(t)

def Demo.file(ls: List<&2, String>) -> String:
  String.join(ls, "\n") ++ "\n"

# where each cut's line starts
def Demo.cuts(ks: List<&2, Nat>, +ls: List<&2, String>) -> List<&2, Nat>:
  match ks:
    case Nil{}:
      Nil{}
    case k <> t:
      Q.Bytes.at(ls, k, 0n) <> Demo.cuts(t, ls)

# one log through store.db at each cut: no record or window may differ;
# refused: the cuts Hot.load must refuse (none: a torn fragment before
# applied_off is skipped as the store skipped it)
def store.db(+name: String, +cs: List<&2, M.Change>, +cuts: List<&2, Nat>, +refused: Nat) -> IO(Unit):
  do IO<Unit>:
    +ls : List<&2, String> = Demo.text(cs)
    +home : String <- Q.Tmp.dir()
    Q.Wr.file(home ++ "/events.jsonl", Demo.file(ls))
    +o : Q.Out <- Q.Equiv.cuts(Demo.cuts(cuts, ls), home, Q.Out{0n, 0n})
    rm : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("rm", ["-rf", home], "", 4096, 10000)
    C.Check.str(name ++ ": store.db + Hot.load equals the whole log folded at every cut it is used", Nat.show(Q.Out.bad(o)), "0")
    C.Check.str(name ++ ": the cuts it refuses", Nat.show(Q.Out.refused(o)), Nat.show(refused))

# the demo log without its last deletions: threads still open, so their
# entry windows are compared too
def Demo.live() -> List<&2, M.Change>:
  List.take(&2, M.Change, S.Demo.changes(), Nat.sub(List.length(&2, M.Change, S.Demo.changes()), 5n))

# a thread settled when the store was made, reopened in the tail by a new
# message: its window must come from the store too, not only its tail
# entries (the real log's 50% cut found two such threads)
def Demo.reopen() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "P", "/p", 1n}, M.ThreadCreated{"t1", "p1", "A", M.Local{}, "", "", "claude", "", 1n},
   M.MessagePosted{"t1", "m1", M.User{}, "one", 2n}, M.MessagePosted{"t1", "m2", M.Assistant{}, "two", 3n}, M.MessagePosted{"t1", "m3", M.User{}, "three", 4n},
   M.MessagePosted{"t1", "m4", M.Assistant{}, "four", 5n}, M.ActivityLogged{"t1", "u1", "tool", "Bash", "ls", 6n}, M.MessagePosted{"t1", "m5", M.Assistant{}, "five", 7n},
   M.ThreadSettled{"t1", True{}, 8n},
   M.ThreadUnsettled{"t1", 9n}, M.MessagePosted{"t1", "m6", M.User{}, "back again", 9n}, M.MessagePosted{"t1", "m7", M.Assistant{}, "welcome back", 10n}]

def main() -> IO(Unit):
  do IO<Unit>:
    IO.print(per.domain(S.Tab.dump(boot(30n)), S.Tab.dump(fold())))
    at(0n)
    at(1n)
    at(30n)
    at(52n)
    at(60n)
    at(List.length(&2, S.LL, S.Demo.log()))
    store.db("demo", S.Demo.changes(), [0n, 1n, 30n, 52n, 60n, List.length(&2, M.Change, S.Demo.changes()), List.length(&2, S.LL, S.Demo.log())], 0n)
    store.db("a thread reopened after the cut", Demo.reopen(), [9n, 0n, 12n], 0n)
    store.db("demo, threads open", Demo.live(), [0n, 3n, 10n, 30n, 50n, List.length(&2, M.Change, Demo.live()), Nat.add(List.length(&2, M.Change, Demo.live()), 2n)], 0n)
