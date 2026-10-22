import Base
import ../src/server/db.bend as D
import ../src/server/host.bend as H
import ../src/server/hotboot.bend as HOT
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/codec.bend as C
import ../src/core/store.bend as ST
import ../src/core/storedb.bend as S
import ../src/core/utf8.bend as U

# The hot boot's equivalence harness (S3, written by R1): a home's log
# folded whole (S_full) against store.db built from a prefix of it and
# loaded by the hub's own Hot.load (records + the open threads' entry
# window + the log's tail: S_hot). Records are compared per domain with
# S1's Tab.dump; entries only for the open threads, whose window must hold
# the newest Hot.window() of State.mine in order. Used by
# test/snap_equiv_test.bend (the demo log, in a temporary home) and
# test/native/snap_equiv.bend HOME [CUT...] (a copy of a real home).

# Building store.db from a prefix (test/native/snap_build.bend's builder:
# every line that starts before `end` bytes; keep the two in step)
# ----------------------------------------------------------------------

type LN is Data:
  LN{len: Nat, text: String}

type Cut is Data:
  Cut{part: List<&2, U32>, n: Nat, done: List<&2, LN>}

def Cut.step(nl: Bool, +x: U32, c: Cut) -> Cut:
  match nl c:
    case True{} Cut{part, +n, done}:
      Cut{Nil{}, 0n, LN{n, U.Utf8.decode(List.reverse(&2, U32, part))} <> done}
    case False{} Cut{part, +n, done}:
      Cut{x <> part, 1n+n, done}

def Cut.go(bs: List<&2, U32>, c: Cut) -> Cut:
  match bs:
    case Nil{}:
      c
    case +x <> t:
      Cut.go(t, Cut.step(U32.is_eq(x, 10), x, c))

# the complete lines of a chunk, oldest first
def Cut.done(c: Cut) -> List<&2, LN>:
  match c:
    case Cut{_, _, done}:
      List.reverse(&2, LN, done)

def Cut.lines(bs: List<&2, U32>) -> List<&2, LN>:
  Cut.done(Cut.go(bs, Cut{Nil{}, 0n, Nil{}}))

# the fold so far: state, lines numbered, byte offset, next entry ordinal,
# statements to write (newest first)
type B is Data:
  B{st: M.State, seq: Nat, off: Nat, ord: Nat, items: List<&2, List<&2, String>>}

def B.off(b: B) -> Nat:
  match b:
    case B{_, _, o, _, _}:
      o

def Ops.sql(os: List<&2, S.Op>, +acc: List<&2, List<&2, String>>) -> List<&2, List<&2, String>>:
  match os:
    case Nil{}:
      acc
    case S.Put{+t, cols} <> rest:
      Ops.sql(rest, Bool.pick(List<&2, List<&2, String>>, String.eq(t, "term"), acc, S.Sql.of(S.Put{t, cols}) <> acc))
    case o <> rest:
      Ops.sql(rest, S.Sql.of(o) <> acc)

def B.flush(b: B) -> B & List<&2, List<&2, String>>:
  match b:
    case B{st, seq, off, ord, items}:
      (B{st, seq, off, ord, Nil{}}, List.reverse(&2, List<&2, String>, items))

def Rd.done(r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(List<&2, U32>):
  (f, res) = r
  do IO<List<&2, U32>>:
    File.close(f)
    return Result.default(&1, &1, U32 & String, List<&2, U32>, res, Nil{})

def Rd.at.k(r: Result<&1, &1, U32 & String, File>, +at: U32, +n: U32) -> IO(List<&2, U32>):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, List<&2, U32>>, List<&2, U32>, File.read_at(f, at, n), Rd.done)
    case Fail{_}:
      IO.pure(List<&2, U32>, Nil{})

def Rd.at(path: String, +at: U32, +n: U32) -> IO(List<&2, U32>):
  IO.bind(Result<&1, &1, U32 & String, File>, List<&2, U32>, File.open(path, "r"), r => Rd.at.k(r, at, n))

def Rd.size.done(r: File & Result<&1, &1, U32 & String, U32>) -> IO(U32):
  (f, res) = r
  do IO<U32>:
    File.close(f)
    return Result.default(&1, &1, U32 & String, U32, res, 0)

def Rd.size.k(r: Result<&1, &1, U32 & String, File>) -> IO(U32):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, U32>, U32, File.size(f), Rd.size.done)
    case Fail{_}:
      IO.pure(U32, 0)

def Rd.size(path: String) -> IO(U32):
  IO.bind(Result<&1, &1, U32 & String, File>, U32, File.open(path, "r"), Rd.size.k)

def Chunk() -> Nat:
  8388608n

def Write.ok.k(fine: Bool, +r: String, +what: String) -> IO(Unit):
  match fine:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      IO.die(Unit, 1, "snap_equiv build: " ++ what ++ ": " ++ r)

def Write.ok(+r: String, +what: String) -> IO(Unit):
  Write.ok.k(String.starts_with(r, "ok"), r, what)

def Build.write(r: B & List<&2, List<&2, String>>, +h: String, k: B -> IO(B)) -> IO(B):
  (b, items) = r
  do IO<B>:
    w : String <- D.Db.script(h, items)
    Write.ok(w, "facts")
    k(b)

def Build.stuck(same: Bool, +h: String, b: B, k: B -> IO(B)) -> IO(B):
  match same:
    case True{}:
      IO.pure(B, b)
    case False{}:
      Build.write(B.flush(b), h, k)

def Dump.sql(os: List<&2, S.Op>, acc: List<&2, List<&2, String>>) -> List<&2, List<&2, String>>:
  match os:
    case Nil{}:
      List.reverse(&2, List<&2, String>, acc)
    case o <> t:
      Dump.sql(t, S.Sql.of(o) <> acc)

def Build.finish(b: B, +h: String, +id: String) -> IO(Unit):
  match b:
    case B{+st, +seq, +off, _, _}:
      do IO<Unit>:
        w : String <- D.Db.script(h, Dump.sql(S.Dump.ops(st), Nil{}))
        Write.ok(w, "records")
        m : String <- D.Db.script(h, [["INSERT OR REPLACE INTO meta(k, v) VALUES (?, ?)", "schema", "1"], ["INSERT OR REPLACE INTO meta(k, v) VALUES (?, ?)", "codec", Nat.show(S.Sdb.codec())],
          ["INSERT OR REPLACE INTO meta(k, v) VALUES (?, ?)", "applied_seq", Nat.show(seq)], ["INSERT OR REPLACE INTO meta(k, v) VALUES (?, ?)", "applied_off", Nat.show(off)],
          ["INSERT OR REPLACE INTO meta(k, v) VALUES (?, ?)", "build", "snap_build"], ["INSERT OR REPLACE INTO meta(k, v) VALUES (?, ?)", "log_id", id]])
        Write.ok(m, "meta")
        closed : String <- D.Db.close(h)
        IO.print("store.db: " ++ Nat.show(seq) ++ " lines, applied_off " ++ Nat.show(off) ++ ", state seq " ++ Nat.show(M.State.seq(st)))

# The whole log folded, as a cold boot folds it
# ----------------------------------------------

def Full.state(+home: String) -> IO(M.State):
  do IO<M.State>:
    rd : List<&2, String> <- H.EvLog.read_from(home ++ "/events.jsonl", "0")
    return ST.Loaded.st(ST.Store.load(ST.Store.read.body(rd), ST.Loaded{M.State.empty(), Nil{}}))

# Records, per domain
# -------------------
# A dump's key is "<table>\u{1f}<key cols>"; obj rows are counted by their
# domain ("obj.<domain>"). A setting the hot boot leaves cold (S.Sdb.cold)
# missing from S_hot is expected and counted apart; anything else missing,
# extra or differing is a bug.

def Same.go(a: String, b: String, ok: Bool) -> Bool:
  match a b ok:
    case _ _ False{}:
      False{}
    case SNil{} SNil{} True{}:
      True{}
    case SCon{Chr{x}, xt} SCon{Chr{y}, yt} True{}:
      Same.go(xt, yt, U32.is_eq(x, y))
    case _ _ True{}:
      False{}

# equal text, walked in a tail call (Base's String.eq is not one, and a
# setting can be tens of kilobytes)
def Same(a: String, b: String) -> Bool:
  Same.go(a, b, True{})

def Key.first.go(s: String, +acc: String) -> String:
  match s:
    case SNil{}:
      String.reverse(acc)
    case SCon{Chr{+c}, t}:
      Bool.pick(String, U32.is_eq(c, 31), String.reverse(acc), Key.first.go(t, SCon{Chr{c}, acc}))

# the text before the first separator
def Key.first(s: String) -> String:
  Key.first.go(s, SNil{})

def Key.rest(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{c}, +t}:
      Bool.pick(String, U32.is_eq(c, 31), t, Key.rest(t))

def Key.dom.k(obj: Bool, +k: String) -> String:
  match obj:
    case True{}:
      "obj." ++ Key.first(Key.rest(k))
    case False{}:
      Key.first(k)

def Key.dom(+k: String) -> String:
  Key.dom.k(String.eq(Key.first(k), "obj"), k)

def Key.cold(+k: String) -> Bool:
  Bool.and(String.eq(Key.first(k), "setting"), S.Sdb.cold(Key.first(Key.rest(k))))

def Vis.go(s: String, +n: Nat, acc: String) -> String:
  match s n:
    case SNil{} _:
      String.reverse(acc)
    case _ 0n:
      String.reverse(acc) ++ "…"
    case SCon{Chr{+c}, t} 1n+m:
      Vis.go(t, m, SCon{Chr{Bool.pick(U32, U32.is_eq(c, 31), 124, Bool.pick(U32, U32.is_eq(c, 10), 8629, c))}, acc})

# a value shown on one line, cut at 240 characters
def Vis(s: String) -> String:
  Vis.go(s, 240n, SNil{})

type Cn is Data:
  Cn{eq: Nat, miss: Nat, cold: Nat, extra: Nat, diff: Nat, ex: List<&2, String>}

def Cn.zero() -> Cn:
  Cn{0n, 0n, 0n, 0n, 0n, Nil{}}

def Cn.one(+kind: Nat, +k: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_eq(kind, k), 1n, 0n)

# kind: 0 equal, 1 missing, 2 missing and cold, 3 extra, 4 differing,
# 5 nothing; the first three bugs keep a note each
def Cn.add(c: Cn, +kind: Nat, +note: String) -> Cn:
  match c:
    case Cn{e, m, co, x, d, +ex}:
      Cn{Nat.add(e, Cn.one(kind, 0n)), Nat.add(m, Cn.one(kind, 1n)), Nat.add(co, Cn.one(kind, 2n)), Nat.add(x, Cn.one(kind, 3n)), Nat.add(d, Cn.one(kind, 4n)),
        Bool.pick(List<&2, String>, Bool.and(Bool.or(Nat.is_eq(kind, 1n), Bool.or(Nat.is_eq(kind, 3n), Nat.is_eq(kind, 4n))), Nat.is_lt(List.length(&2, String, ex), 3n)), List.append(&2, String, ex, [note]), ex)}

def Cn.bad(c: Cn) -> Nat:
  match c:
    case Cn{_, m, _, x, d, _}:
      Nat.add(m, Nat.add(x, d))

def Tally.put(r: Map<&2, Cn> & Cn, +d: String, +kind: Nat, +note: String) -> Map<&2, Cn>:
  (m, c) = r
  Map.set(&2, Cn, m, d, Cn.add(c, kind, note))

def Got.v(r: Map<&2, String> & String) -> String:
  (_, v) = r
  v

def Absent() -> String:
  "\u{0}"

def B.change(m: Maybe<&2, M.Change>, +st: M.State, +seq: Nat, +off: Nat, +len: Nat, +ord: Nat, items: List<&2, List<&2, String>>) -> B:
  match m:
    case Some{+c}:
      B{M.State.apply(st, c), seq, Nat.add(off, 1n+len), S.Sdb.ord(c, ord), Ops.sql(S.Sdb.facts(st, S.At{seq, off, len, ord}, c), items)}
    case None{}:
      B{st, seq, Nat.add(off, 1n+len), ord, Ops.sql([S.F.line(S.At{seq, off, len, ord}, "")], items)}

def B.parsed(m: Maybe<&2, J.Json>, +len: Nat, b: B) -> B:
  match m b:
    case Some{+o} B{st, +seq, +off, +ord, items}:
      B.change(C.Change.decode(o), st, 1n+seq, off, len, ord, items)
    case None{} B{st, seq, +off, ord, items}:
      B{st, seq, Nat.add(off, 1n+len), ord, items}

def B.line(go: Bool, l: LN, b: B) -> B:
  match go l:
    case True{} LN{+len, +text}:
      B.parsed(J.Json.parse(text), len, b)
    case False{} _:
      b

# lines that start before end
def B.lines(ls: List<&2, LN>, +end: Nat, b: B) -> B:
  match ls:
    case Nil{}:
      b
    case l <> t:
      +b2 = b
      B.lines(t, end, B.line(Nat.is_lt(B.off(b2), end), l, b2))

def Build.chunk(more: Bool, +h: String, +path: String, +end: Nat, +size: Nat, +b: B, k: B -> IO(B)) -> IO(B):
  match more:
    case False{}:
      IO.pure(B, b)
    case True{}:
      do IO<B>:
        +off : Nat = B.off(b)
        bs : List<&2, U32> <- Rd.at(path, U32.from_nat(off), U32.from_nat(Bool.pick(Nat, Nat.is_lt(Nat.sub(size, off), Chunk()), Nat.sub(size, off), Chunk())))
        +b2 : B = B.lines(Cut.lines(bs), end, b)
        IO.print("  at byte " ++ Nat.show(B.off(b2)))
        Build.stuck(Nat.is_eq(B.off(b2), off), h, b2, k)

def Build.loop(fuel: Nat, +h: String, +path: String, +end: Nat, +size: Nat, b: B) -> IO(B):
  match fuel:
    case 0n:
      IO.pure(B, b)
    case 1n+f:
      +b2 = b
      Build.chunk(Nat.is_lt(B.off(b2), end), h, path, end, size, b2, x => Build.loop(f, h, path, end, size, x))

# HOME/store.db made afresh from the lines of HOME/events.jsonl that start
# before byte `end` (as snap_build's main)
def Build.store(+home: String, +end: Nat) -> IO(Unit):
  do IO<Unit>:
    +path : String = home ++ "/events.jsonl"
    sz : U32 <- Rd.size(path)
    +size : Nat = U32.to_nat(sz)
    rm : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("rm", ["-f", home ++ "/store.db", home ++ "/store.db-wal", home ++ "/store.db-shm"], "", 4096, 10000)
    +o : String <- D.Db.open(home ++ "/store.db")
    Write.ok(o, "open")
    +h : String = String.drop(o, 3n)
    s : String <- D.Db.exec(h, S.Sdb.schema())
    Write.ok(s, "schema")
    hd : List<&2, U32> <- Rd.at(path, 0, 4096)
    +id : String = S.Sdb.log_id(hd)
    b : B <- Build.loop(1000000n, h, path, end, size, B{M.State.empty(), 0n, 0n, 1n, Nil{}})
    Build.finish(b, h, id)

def Cmp.kind(+h: String, +v: String, +k: String) -> Nat:
  Bool.pick(Nat, String.eq(h, Absent()), Bool.pick(Nat, Key.cold(k), 2n, 1n), Bool.pick(Nat, Same(h, v), 0n, 4n))

def Cmp.note(+k: String, +v: String, +h: String) -> String:
  Vis(k) ++ "\n        full: " ++ Vis(v) ++ "\n        hot:  " ++ Bool.pick(String, String.eq(h, Absent()), "(none)", Vis(h))

def Cmp.one(+k: String, +v: String, +h: String, acc: Map<&2, Cn>) -> Map<&2, Cn>:
  Tally.put(Map.get(Cn, Cn.zero(), acc, Key.dom(k)), Key.dom(k), Cmp.kind(h, v, k), Cmp.note(k, v, h))

# every row of the full dump, against the hot one
def Cmp.full(xs: List<&2, Sigma<&2, &2, String, _ => String>>, +hot: Map<&2, String>, acc: Map<&2, Cn>) -> Map<&2, Cn>:
  match xs:
    case Nil{}:
      acc
    case (+k, +v) <> t:
      Cmp.full(t, hot, Cmp.one(k, v, Got.v(Map.get(String, Absent(), hot, k)), acc))

def Cmp.extra.one(+absent: Bool, +k: String, +v: String, acc: Map<&2, Cn>) -> Map<&2, Cn>:
  match absent:
    case True{}:
      Tally.put(Map.get(Cn, Cn.zero(), acc, Key.dom(k)), Key.dom(k), 3n, Vis(k) ++ "\n        full: (none)\n        hot:  " ++ Vis(v))
    case False{}:
      acc

# rows only the hot dump has
def Cmp.hot(xs: List<&2, Sigma<&2, &2, String, _ => String>>, +full: Map<&2, String>, acc: Map<&2, Cn>) -> Map<&2, Cn>:
  match xs:
    case Nil{}:
      acc
    case (+k, +v) <> t:
      Cmp.hot(t, full, Cmp.extra.one(String.eq(Got.v(Map.get(String, Absent(), full, k)), Absent()), k, v, acc))

def Cmp.records(+full: Map<&2, String>, +hot: Map<&2, String>) -> Map<&2, Cn>:
  Cmp.hot(Map.to_list(&2, String, hot), full, Cmp.full(Map.to_list(&2, String, full), hot, Map.new(&2, Cn)))

def Rep.ex(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}:
      ""
    case x <> t:
      "      " ++ x ++ "\n" ++ Rep.ex(t)

def Rep.dom(+d: String, c: Cn) -> String:
  match c:
    case Cn{e, +m, +co, +x, +df, ex}:
      Bool.pick(String, Nat.is_eq(Nat.add(m, Nat.add(x, df)), 0n), "  ok   ", "  DIFF ") ++ d ++ ": equal " ++ Nat.show(e) ++ ", missing " ++ Nat.show(m) ++ ", extra " ++ Nat.show(x) ++ ", differing " ++ Nat.show(df)
        ++ Bool.pick(String, Nat.is_eq(co, 0n), "", " (cold settings left out, as meant: " ++ Nat.show(co) ++ ")") ++ "\n" ++ Rep.ex(ex)

# a report's text and how many things in it are wrong
type Rep is Data:
  Rep{text: String, bad: Nat}

def Rep.doms(xs: List<&2, Sigma<&2, &2, String, _ => Cn>>, +bad: Nat, acc: String) -> Rep:
  match xs:
    case Nil{}:
      Rep{acc, bad}
    case (+d, +c) <> t:
      Rep.doms(t, Nat.add(bad, Cn.bad(c)), acc ++ Rep.dom(d, c))

def Rep.text(r: Rep) -> String:
  match r:
    case Rep{t, _}:
      t

def Rep.bad(r: Rep) -> Nat:
  match r:
    case Rep{_, b}:
      b

def Rep.cat(a: Rep, b: Rep) -> Rep:
  match a b:
    case Rep{t1, n1} Rep{t2, n2}:
      Rep{t1 ++ t2, Nat.add(n1, n2)}

def Rep.records(+full: M.State, +hot: M.State) -> Rep:
  Rep.doms(Map.to_list(&2, Cn, Cmp.records(S.Tab.dump(full), S.Tab.dump(hot))), 0n, "")

# Entries: the open threads' window
# --------------------------------
# For each thread the hot state loaded (HOT.Hot.open), State.mine in S_hot
# must start with the newest min(n, HOT.Hot.window()) of State.mine in
# S_full, in order, and hold nothing that S_full does not, in the same
# order (older acts it keeps, terminals and review notes, may follow).

def Ent.key(e: M.Entry) -> String:
  match e:
    case M.Msg{id, th, r, text, at}:
      "msg " ++ id ++ " " ++ th ++ " " ++ C.Role.name(r) ++ " " ++ Nat.show(at) ++ " " ++ text
    case M.Act{id, th, tone, kind, text, at}:
      "act " ++ id ++ " " ++ th ++ " " ++ tone ++ " " ++ kind ++ " " ++ Nat.show(at) ++ " " ++ text

def Ent.len(xs: List<&2, M.Entry>, +n: Nat) -> Nat:
  match xs:
    case Nil{}:
      n
    case _ <> t:
      Ent.len(t, 1n+n)

def Ent.heads(a: List<&2, M.Entry>, b: List<&2, M.Entry>) -> Bool:
  match a b:
    case +x <> _ +y <> _:
      Same(Ent.key(x), Ent.key(y))
    case _ _:
      False{}

# "" when the first n of full are the first n of hot, else where not
# (same: whether their heads are the same entry)
def Ent.pre(n: Nat, full: List<&2, M.Entry>, hot: List<&2, M.Entry>, +i: Nat, same: Bool) -> String:
  match n full hot same:
    case 0n _ _ _:
      ""
    case 1n+_ Nil{} _ _:
      ""
    case 1n+_ _ Nil{} _:
      "hot ends at " ++ Nat.show(i)
    case 1n+m _ <> +ft _ <> +ht True{}:
      Ent.pre(m, ft, ht, 1n+i, Ent.heads(ft, ht))
    case 1n+_ +f <> _ +h <> _ False{}:
      "at " ++ Nat.show(i) ++ ": full " ++ Vis(Ent.key(f)) ++ " | hot " ++ Vis(Ent.key(h))

def Ent.prefix(n: Nat, +full: List<&2, M.Entry>, +hot: List<&2, M.Entry>) -> String:
  Ent.pre(n, full, hot, 0n, Ent.heads(full, hot))

# "" when every entry of hot is in full, in the same order
def Ent.sub(full: List<&2, M.Entry>, hot: List<&2, M.Entry>, same: Bool) -> String:
  match full hot same:
    case _ Nil{} _:
      ""
    case Nil{} +h <> _ _:
      "not in full, in order: " ++ Vis(Ent.key(h))
    case _ <> +ft _ <> +ht True{}:
      Ent.sub(ft, ht, Ent.heads(ft, ht))
    case _ <> +ft +h <> +ht False{}:
      Ent.sub(ft, h <> ht, Ent.heads(ft, h <> ht))

def Ent.min(+a: Nat, +b: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_le(a, b), a, b)

# what is wrong with a thread's window ("" nothing)
def Ent.check(+full: List<&2, M.Entry>, +hot: List<&2, M.Entry>) -> String:
  +p = Ent.prefix(Ent.min(Ent.len(full, 0n), HOT.Hot.window()), full, hot)
  +q = Ent.sub(full, hot, Ent.heads(full, hot))
  Bool.pick(String, String.is_empty(p), q, "newest " ++ p ++ Bool.pick(String, String.is_empty(q), "", "; " ++ q))

def Ent.thread(+full: M.State, +hot: M.State, +th: String, r: Rep) -> Rep:
  +bad = Ent.check(M.State.mine(full, th), M.State.mine(hot, th))
  Rep.cat(r, Bool.pick(Rep, String.is_empty(bad), Rep{"", 0n},
    Rep{"      " ++ th ++ " (full " ++ Nat.show(Ent.len(M.State.mine(full, th), 0n)) ++ ", hot " ++ Nat.show(Ent.len(M.State.mine(hot, th), 0n)) ++ "): " ++ bad ++ "\n", 1n}))

def Ent.step(+open: Bool, +full: M.State, +hot: M.State, +th: String, r: Rep) -> Rep:
  match open:
    case True{}:
      Ent.thread(full, hot, th, r)
    case False{}:
      r

def Ent.threads(ts: List<&2, M.Thread>, +full: M.State, +hot: M.State, +n: Nat, r: Rep) -> Rep:
  match ts:
    case Nil{}:
      Rep.cat(Rep{"  " ++ Nat.show(n) ++ " open threads\n", 0n}, r)
    case +t <> rest:
      Ent.threads(rest, full, hot, Nat.add(n, Bool.pick(Nat, HOT.Hot.open(t), 1n, 0n)), Ent.step(HOT.Hot.open(t), full, hot, M.Thread.id(t), r))

def St.threads(s: M.State) -> List<&2, M.Thread>:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}:
      Map.values(&2, M.Thread, ts)

def Rep.entries(+full: M.State, +hot: M.State) -> Rep:
  Ent.threads(St.threads(hot), full, hot, 0n, Rep{"", 0n})

# The run
# -------

# what the cuts came to: things that differ, and cuts whose store Hot.load
# refused (it falls back to the full replay: slower, never wrong)
type Out is Data:
  Out{bad: Nat, refused: Nat}

def Out.bad(o: Out) -> Nat:
  match o:
    case Out{b, _}:
      b

def Out.refused(o: Out) -> Nat:
  match o:
    case Out{_, r}:
      r

def Out.add(a: Out, b: Out) -> Out:
  match a b:
    case Out{b1, r1} Out{b2, r2}:
      Out{Nat.add(b1, b2), Nat.add(r1, r2)}

def Equiv.loaded(+ok: Bool, +why: String, +full: M.State, +hot: M.State, +cut: Nat) -> IO(Out):
  match ok:
    case False{}:
      do IO<Out>:
        IO.print("cut " ++ Nat.show(cut) ++ ": REFUSED Hot.load did not use the store: " ++ why)
        return Out{0n, 1n}
    case True{}:
      do IO<Out>:
        +r : Rep = Rep.cat(Rep.records(full, hot), Rep.entries(full, hot))
        IO.print("cut " ++ Nat.show(cut) ++ ": " ++ Bool.pick(String, Nat.is_eq(Rep.bad(r), 0n), "ok", "FAIL " ++ Nat.show(Rep.bad(r)) ++ " differences") ++ "\n" ++ Rep.text(r))
        return Out{Rep.bad(r), 0n}

# build store.db from the lines before byte `cut`, Hot.load it and compare
# with the whole log folded. The fold is made after the store is built (and
# its own fold let go): on a real log one folded state is many GB on the JS
# runtime, two at once outgrew a 20 GB cap.
def Equiv.cut(+home: String, +cut: Nat) -> IO(Out):
  do IO<Out>:
    Build.store(home, cut)
    +hot : HOT.Hot <- HOT.Hot.load(home)
    +full : M.State <- Full.state(home)
    IO.print("folded whole: seq " ++ Nat.show(M.State.seq(full)))
    Equiv.loaded(HOT.Hot.ok(hot), HOT.Hot.why(hot), full, HOT.Hot.st(hot), cut)

# the run over several cuts (byte offsets)
def Equiv.cuts(cs: List<&2, Nat>, +home: String, acc: Out) -> IO(Out):
  match cs:
    case Nil{}:
      IO.pure(Out, acc)
    case +c <> rest:
      do IO<Out>:
        n : Out <- Equiv.cut(home, c)
        Equiv.cuts(rest, home, Out.add(acc, n))

# Files
# -----

def Bytes.char(+c: U32) -> Nat:
  Bool.pick(Nat, U32.is_lt(c, 128), 1n, Bool.pick(Nat, U32.is_lt(c, 2048), 2n, Bool.pick(Nat, U32.is_lt(c, 65536), 3n, 4n)))

def Bytes.go(s: String, +n: Nat) -> Nat:
  match s:
    case SNil{}:
      n
    case SCon{Chr{c}, t}:
      Bytes.go(t, Nat.add(n, Bytes.char(c)))

# a line's UTF-8 size with its newline
def Bytes.line(s: String) -> Nat:
  Bytes.go(s, 1n)

# where line k starts (lines from 0)
def Bytes.at(ls: List<&2, String>, k: Nat, +n: Nat) -> Nat:
  match ls k:
    case _ 0n:
      n
    case Nil{} _:
      n
    case l <> t 1n+m:
      Bytes.at(t, m, Nat.add(n, Bytes.line(l)))

def Wr.closed(r: File & Result<&1, &1, U32 & String, Unit>) -> IO(Unit):
  (f, _) = r
  File.close(f)

def Wr.k(r: Result<&1, &1, U32 & String, File>, data: String) -> IO(Unit):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, Unit>, Unit, File.write(f, data), Wr.closed)
    case Fail{_}:
      IO.die(Unit, 1, "snap_equiv: cannot write a file")

def Wr.file(path: String, data: String) -> IO(Unit):
  IO.bind(Result<&1, &1, U32 & String, File>, Unit, File.open(path, "w"), r => Wr.k(r, data))

def Tmp.out(r: Result<&1, &1, U32 & String, U32 & (String & String)>) -> String:
  match r:
    case Done{(_, (out, _))}:
      String.trim(out)
    case Fail{_}:
      ""

# a new temporary folder
def Tmp.dir() -> IO(String):
  IO.bind(Result<&1, &1, U32 & String, U32 & (String & String)>, String, Process.run("mktemp", ["-d"], "", 4096, 10000), r => IO.pure(String, Tmp.out(r)))

# Dumps, for the phased run on a real log (test/tools/snap_equiv.sh): one
# folded state per process, written as JSON lines, compared outside
# (test/tools/snap_equiv_cmp.py). Records: [key, value]; an open thread's
# entries: ["E", thread, key...] newest first (Ent.key); a refusal:
# ["REFUSED", why].

def Wf.got(r: File & Result<&1, &1, U32 & String, Unit>, k: File -> IO(File)) -> IO(File):
  (f, _) = r
  k(f)

def Wf.recs(xs: List<&2, Sigma<&2, &2, String, _ => String>>, f: File) -> IO(File):
  match xs:
    case Nil{}:
      IO.pure(File, f)
    case (k, v) <> t:
      IO.bind(File & Result<&1, &1, U32 & String, Unit>, File, File.write(f, J.Json.show(J.Json.arr([J.Str{k}, J.Str{v}])) ++ "\n"), r => Wf.got(r, f2 => Wf.recs(t, f2)))

def Wf.keys(es: List<&2, M.Entry>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match es:
    case Nil{}:
      List.reverse(&2, J.Json, acc)
    case e <> t:
      Wf.keys(t, J.Str{Ent.key(e)} <> acc)

def Wf.thread(+st: M.State, +th: String) -> String:
  J.Json.show(J.Json.arr(J.Str{"E"} <> J.Str{th} <> Wf.keys(M.State.mine(st, th), Nil{}))) ++ "\n"

def Wf.ents(ths: List<&2, String>, +st: M.State, f: File) -> IO(File):
  match ths:
    case Nil{}:
      IO.pure(File, f)
    case +th <> t:
      IO.bind(File & Result<&1, &1, U32 & String, Unit>, File, File.write(f, Wf.thread(st, th)), r => Wf.got(r, f2 => Wf.ents(t, st, f2)))

def Wf.open(ts: List<&2, M.Thread>, +acc: List<&2, String>) -> List<&2, String>:
  match ts:
    case Nil{}:
      acc
    case +t <> rest:
      Wf.open(rest, Bool.pick(List<&2, String>, HOT.Hot.open(t), M.Thread.id(t) <> acc, acc))

# the threads a hot state loaded entries for
def Wf.opens(+st: M.State) -> List<&2, String>:
  Wf.open(St.threads(st), Nil{})

def Wf.done(r: Result<&1, &1, U32 & String, File>, +st: M.State, +ths: List<&2, String>) -> IO(Unit):
  match r:
    case Done{f}:
      do IO<Unit>:
        f2 : File <- Wf.recs(Map.to_list(&2, String, S.Tab.dump(st)), f)
        f3 : File <- Wf.ents(ths, st, f2)
        File.close(f3)
    case Fail{_}:
      IO.die(Unit, 1, "snap_equiv: cannot write the dump")

# a state's records and the given threads' entries, to path
def Wf.state(+path: String, +st: M.State, +ths: List<&2, String>) -> IO(Unit):
  IO.bind(Result<&1, &1, U32 & String, File>, Unit, File.open(path, "w"), r => Wf.done(r, st, ths))

def Wf.refused(+ok: Bool, +why: String, +path: String, +st: M.State) -> IO(Unit):
  match ok:
    case True{}:
      Wf.state(path, st, Wf.opens(st))
    case False{}:
      Wr.file(path, J.Json.show(J.Json.arr([J.Str{"REFUSED"}, J.Str{why}])) ++ "\n")

# phase hot: Hot.load home, dump what it loaded
def Phase.hot(+home: String, +out: String) -> IO(Unit):
  do IO<Unit>:
    +hot : HOT.Hot <- HOT.Hot.load(home)
    Wf.refused(HOT.Hot.ok(hot), HOT.Hot.why(hot), out, HOT.Hot.st(hot))

# phase full: the whole log folded, dumped with the given threads' entries
def Phase.full(+home: String, +out: String, +ths: List<&2, String>) -> IO(Unit):
  do IO<Unit>:
    full : M.State <- Full.state(home)
    Wf.state(out, full, ths)
