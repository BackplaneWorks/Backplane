import Base
import ./db.bend as D
import ../core/json.bend as J
import ../core/model.bend as M
import ../core/store.bend as ST
import ../core/storedb.bend as SDB
import ../core/snapshot.bend as SN
import ./host.bend as H

# The hot boot (S3; bp-coord/store/S1-SCHEMA.md)
# ----------------------------------------------
# The hub starts from store.db's records instead of replaying the whole
# event log: every project, thread, task, ask, queue, setting but the cold
# ones (SDB.Sdb.cold), device, sidebar nav, the bots' world, the mail
# box's records and headers, and a recent window of entries for the open
# threads only (Hot.window per thread, with their terminals and review
# notes). Then only the log's tail after meta.applied_off is folded on.
# A missing or stale store (no file, another codec, a log shorter than
# the store says, lines that do not add up) answers why, and the server
# replays the log as before.

# lines of the file between two byte offsets, newest first, empty ones
# and those starting in a skipped range left out (skips: [a1, b1, ...],
# decimal, a <= start < b; effects/hotboot.js)
def Hot.lines(path: String, from: U32, to: U32, skips: List<&2, String>) -> IO(List<&2, String>):
  import "./effects/hotboot.js"

# what a boot starts from: the read model, the lines clients may see
# (newest first, as ST.Loaded.wire), and what closes off a torn last line
# (ST.Store.read.tail); ok False: why the store was not used
type Hot is Data:
  Hot{ok: Bool, why: String, st: M.State, ls: List<&2, String>, torn: String}

def Hot.no(+why: String) -> Hot:
  Hot{False{}, why, M.State.empty(), Nil{}, ""}

def Hot.ok(h: Hot) -> Bool:
  match h:
    case Hot{ok, _, _, _, _}:
      ok

def Hot.why(h: Hot) -> String:
  match h:
    case Hot{_, w, _, _, _}:
      w

def Hot.st(h: Hot) -> M.State:
  match h:
    case Hot{_, _, st, _, _}:
      st

def Hot.ls(h: Hot) -> List<&2, String>:
  match h:
    case Hot{_, _, _, ls, _}:
      ls

def Hot.torn(h: Hot) -> String:
  match h:
    case Hot{_, _, _, _, t}:
      t

def Hot.path(+home: String) -> String:
  home ++ "/store.db"

def Hot.log(+home: String) -> String:
  home ++ "/events.jsonl"

# entries loaded per open thread
def Hot.window() -> Nat:
  200n

def Hot.big() -> U32:
  4000000000

# Meta
# ----

def Hot.meta.go(rows: List<&2, List<&2, String>>, m: Map<&2, String>) -> Map<&2, String>:
  match rows:
    case Nil{}:
      m
    case +r <> t:
      Hot.meta.go(t, Map.set(&2, String, m, SN.Snap.col(r, 0n), SN.Snap.col(r, 1n)))

def Hot.get.k(r: Map<&2, String> & String) -> String:
  (_, v) = r
  v

def Hot.get(+m: Map<&2, String>, +k: String) -> String:
  Hot.get.k(Map.get(String, "", m, k))

# a query answered with its error row
def Hot.failed(rows: List<&2, List<&2, String>>) -> String:
  match rows:
    case (+c <> +w <> _) <> _:
      Bool.pick(String, String.eq(c, "\u{0}fail"), w, "")
    case _:
      ""

# why the store's meta rules it out ("" when it does not)
def Hot.stale(+m: Map<&2, String>) -> String:
  +codec = Hot.get(m, "codec")
  Bool.pick(String, Bool.not(String.eq(codec, Nat.show(SDB.Sdb.codec()))), "store.db codec " ++ codec ++ " is not " ++ Nat.show(SDB.Sdb.codec()),
    Bool.pick(String, Bool.or(String.is_empty(Hot.get(m, "applied_off")), String.is_empty(Hot.get(m, "applied_seq"))), "store.db has no applied_off/applied_seq", ""))

# The open threads
# ----------------

def Hot.open.k(s: M.Shelf, busy: Bool) -> Bool:
  match s busy:
    case _ True{}:
      True{}
    case M.Pinned{} _:
      True{}
    case M.Active{} _:
      True{}
    case M.Snoozed{} _:
      True{}
    case _ _:
      False{}

# a thread whose entries the hub loads: on the Pinned or Active shelf, or
# busy (a running turn, something waiting on the person)
def Hot.open(+t: M.Thread) -> Bool:
  match t:
    case M.Thread{_, _, _, _, _, +tu, +pe, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Hot.open.k(M.Shelf.of(t), M.Thread.busy(tu, pe))

# Queries
# -------

def Hot.q.window() -> String:
  "SELECT seq, ord, id, thread, src, kind, role, akind, text, at FROM entry WHERE seq IN ("
    ++ "SELECT seq FROM (SELECT seq, row_number() OVER (PARTITION BY thread ORDER BY seq DESC) AS rn FROM entry WHERE thread IN (SELECT value FROM json_each(?1))) WHERE rn <= ?2"
    ++ " UNION SELECT seq FROM (SELECT seq, row_number() OVER (PARTITION BY src ORDER BY seq DESC) AS rn FROM entry WHERE src <> '' AND src IN (SELECT value FROM json_each(?1))) WHERE rn <= ?2"
    ++ " UNION SELECT seq FROM entry WHERE kind = 'act' AND role IN ('term', 'review') AND thread IN (SELECT value FROM json_each(?1))"
    ++ ") ORDER BY seq"

# the open threads' cold settings (their run inputs) after all
def Hot.q.cold() -> String:
  "SELECT k, v, cold FROM setting WHERE cold = 1 AND k IN (SELECT 'launch.input.' || value FROM json_each(?1) UNION SELECT 'fallback.input.' || value FROM json_each(?1))"

def Hot.q.cols(+t: String) -> String:
  "SELECT " ++ String.join(SDB.Sdb.cols(t), ", ") ++ " FROM " ++ t

def Hot.put(m: Map<&2, List<&2, List<&2, String>>>, +name: String, rows: List<&2, List<&2, String>>) -> Map<&2, List<&2, List<&2, String>>>:
  Map.set(&2, List<&2, List<&2, String>>, m, name, rows)

# record tables read whole, and obj by domain
def Hot.tables() -> List<&2, String>:
  ["project", "thread", "task", "ask", "queue", "bot", "room", "routine", "conv", "mark"]

def Hot.doms() -> List<&2, String>:
  ["device", "nav", "hook", "mem", "peer", "space", "page", "skill", "hops", "talk", "run", "origin", "charter", "decision", "mailname", "state"]

def Hot.read.tables(ts: List<&2, String>, +h: String, m: Map<&2, List<&2, List<&2, String>>>) -> IO(Map<&2, List<&2, List<&2, String>>>):
  match ts:
    case Nil{}:
      IO.pure(Map<&2, List<&2, List<&2, String>>>, m)
    case +t <> rest:
      do IO<Map<&2, List<&2, List<&2, String>>>>:
        rows : List<&2, List<&2, String>> <- D.Db.query(h, Hot.q.cols(t), [], Hot.big())
        Hot.read.tables(rest, h, Hot.put(m, t, rows))

def Hot.read.doms(ds: List<&2, String>, +h: String, m: Map<&2, List<&2, List<&2, String>>>) -> IO(Map<&2, List<&2, List<&2, String>>>):
  match ds:
    case Nil{}:
      IO.pure(Map<&2, List<&2, List<&2, String>>>, m)
    case +d <> rest:
      do IO<Map<&2, List<&2, List<&2, String>>>>:
        rows : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT domain, key, json FROM obj WHERE domain = ?", [d], Hot.big())
        Hot.read.doms(rest, h, Hot.put(m, "obj." ++ d, rows))

def Hot.cat(a: List<&2, List<&2, String>>, b: List<&2, List<&2, String>>) -> List<&2, List<&2, String>>:
  List.append(&2, List<&2, String>, a, b)

# everything but the entries: records, hot settings, posts, headers, counts
def Hot.read.recs(+h: String) -> IO(Map<&2, List<&2, List<&2, String>>>):
  do IO<Map<&2, List<&2, List<&2, String>>>>:
    m0 : Map<&2, List<&2, List<&2, String>>> <- Hot.read.tables(Hot.tables(), h, Map.new(&2, List<&2, List<&2, String>>))
    m1 : Map<&2, List<&2, List<&2, String>>> <- Hot.read.doms(Hot.doms(), h, m0)
    sg : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT k, v, cold FROM setting WHERE cold = 0", [], Hot.big())
    ps : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT seq, id, room, src, text, hop, at FROM post ORDER BY seq", [], Hot.big())
    ms : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT seq, id, conv, src, dst, kind, ref, want, head, size, at FROM mail ORDER BY seq", [], Hot.big())
    cs : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT thread, count(*) FROM entry GROUP BY thread", [], Hot.big())
    return Hot.put(Hot.put(Hot.put(Hot.put(m1, "setting", sg), "post", ps), "mail", ms), "count", cs)

def Hot.add.cold.k(r: Map<&2, List<&2, List<&2, String>>> & List<&2, List<&2, String>>, cold: List<&2, List<&2, String>>) -> Map<&2, List<&2, List<&2, String>>>:
  (m, cur) = r
  Map.set(&2, List<&2, List<&2, String>>, m, "setting", Hot.cat(cur, cold))

def Hot.add.cold(m: Map<&2, List<&2, List<&2, String>>>, cold: List<&2, List<&2, String>>) -> Map<&2, List<&2, List<&2, String>>>:
  Hot.add.cold.k(Map.get(List<&2, List<&2, String>>, Nil{}, m, "setting"), cold)

# the open threads' entries and run inputs, onto the records
def Hot.read.open(+h: String, +ids: String, m: Map<&2, List<&2, List<&2, String>>>) -> IO(SN.SRows):
  do IO<SN.SRows>:
    es : List<&2, List<&2, String>> <- D.Db.query.t(h, Hot.q.window(), [D.DbText{ids}, D.Db.nat(Hot.window())], Hot.big())
    cold : List<&2, List<&2, String>> <- D.Db.query(h, Hot.q.cold(), [ids], Hot.big())
    return SN.SRows{Hot.put(Hot.add.cold(m, cold), "entry", es)}

# Lines
# -----
# The byte ranges before applied_off the store numbered no line in (a torn
# fragment that never parsed, empty lines): the gaps between its line rows
# and after the last one. The lines left are the store's, in its order.

def Hot.q.gaps() -> String:
  "SELECT pe, off FROM (SELECT off, lag(off + len + 1, 1, 0) OVER (ORDER BY seq) AS pe FROM line) WHERE off > pe"
    ++ " UNION ALL SELECT coalesce(max(off + len + 1), 0), ?1 FROM line HAVING coalesce(max(off + len + 1), 0) < ?1"

def Hot.gaps(rows: List<&2, List<&2, String>>) -> List<&2, String>:
  match rows:
    case Nil{}:
      Nil{}
    case +r <> t:
      SN.Snap.col(r, 0n) <> SN.Snap.col(r, 1n) <> Hot.gaps(t)

# Hub `ls` for now (S1's lines-by-offset replaces it): the lines before
# applied_off read outside Bend, each a line clients saw. The store's line
# table numbers only lines that parse; when the non-empty lines are not
# exactly those (a torn fragment in the middle), the store is not used.
# A push registration's line goes out as its wire text (line.wire).

def Wire.swap.hit(reps: List<&2, List<&2, String>>, +n: Nat, +i: Nat) -> Bool:
  match reps:
    case +r <> _:
      Nat.is_eq(Nat.sub(n, SN.Snap.num(SN.Snap.col(r, 0n), 0n)), i)
    case Nil{}:
      False{}

def Wire.go(ls: List<&2, String>, hit: Bool, reps: List<&2, List<&2, String>>, +n: Nat, +i: Nat, acc: List<&2, String>) -> List<&2, String>:
  match ls hit reps:
    case Nil{} _ _:
      List.reverse(&2, String, acc)
    case _ <> t True{} +r <> rest:
      Wire.go(t, Wire.swap.hit(rest, n, 1n+i), rest, n, 1n+i, SN.Snap.col(r, 1n) <> acc)
    case +l <> t _ +reps2:
      Wire.go(t, Wire.swap.hit(reps2, n, 1n+i), reps2, n, 1n+i, l <> acc)

# the n lines clients may see, newest first (ls: the log's lines, newest
# first; reps: line rows "seq, wire" with a wire text, seq descending)
def Wire.of(ls: List<&2, String>, +reps: List<&2, List<&2, String>>, +n: Nat) -> List<&2, String>:
  Wire.go(ls, Wire.swap.hit(reps, n, 0n), reps, n, 0n, Nil{})

# Files
# -----

def Hot.size.done(r: File & Result<&1, &1, U32 & String, U32>) -> IO(U32):
  (f, res) = r
  do IO<U32>:
    File.close(f)
    return Result.default(&1, &1, U32 & String, U32, res, 0)

def Hot.size.k(r: Result<&1, &1, U32 & String, File>) -> IO(U32):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, U32>, U32, File.size(f), Hot.size.done)
    case Fail{_}:
      IO.pure(U32, 0)

# a file's size, 0 when it cannot be opened
def Hot.size(path: String) -> IO(U32):
  IO.bind(Result<&1, &1, U32 & String, File>, U32, File.open(path, "r"), Hot.size.k)

def Hot.exists.k(r: Result<&1, &1, U32 & String, File>) -> IO(Bool):
  match r:
    case Done{f}:
      do IO<Bool>:
        File.close(f)
        return True{}
    case Fail{_}:
      IO.pure(Bool, False{})

def Hot.exists(path: String) -> IO(Bool):
  IO.bind(Result<&1, &1, U32 & String, File>, Bool, File.open(path, "r"), Hot.exists.k)

# Loading
# -------

def Hot.ids.put(yes: Bool, id: String, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match yes:
    case True{}:
      J.Str{id} <> acc
    case False{}:
      acc

def Hot.ids.rows(rows: List<&2, List<&2, String>>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match rows:
    case Nil{}:
      acc
    case +r <> t:
      +th = SN.Snap.thread(SN.Snap.last(r))
      Hot.ids.rows(t, Hot.ids.put(Hot.open(th), M.Thread.id(th), acc))

# the open threads' ids, from the thread rows, as a JSON array
def Hot.ids(+rs: SN.SRows) -> String:
  J.Json.show(J.Json.arr(Hot.ids.rows(SN.SRows.of(rs, "thread"), Nil{})))

def Hot.loaded(ld: ST.Loaded, torn: String) -> Hot:
  match ld:
    case ST.Loaded{st, ls}:
      Hot{True{}, "", st, ls, torn}

# the lines add up: fold the tail onto the records
def Hot.lined(same: Bool, +h: String, +home: String, +rows: SN.SRows, ls: List<&2, String>, +seq: Nat, +off: U32, +size: U32) -> IO(Hot):
  match same:
    case False{}:
      do IO<Hot>:
        cl : String <- D.Db.close(h)
        return Hot.no("the log's lines before applied_off are not the store's " ++ Nat.show(seq))
    case True{}:
      do IO<Hot>:
        reps : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT seq, wire FROM line WHERE wire <> '' ORDER BY seq DESC", [], Hot.big())
        cl : String <- D.Db.close(h)
        +s0 : M.State = SN.Snap.state(rows)
        +rd : List<&2, String> <- H.EvLog.read_from(Hot.log(home), U32.show(off))
        return Hot.loaded(SN.Snap.tail(ST.Store.read.body(rd), s0, SN.Snap.counts(rows), ST.Loaded{s0, Wire.of(ls, reps, seq)}), ST.Store.read.tail(rd))

def Hot.read(+h: String, +home: String, +m: Map<&2, String>, +size: U32) -> IO(Hot):
  do IO<Hot>:
    +recs : Map<&2, List<&2, List<&2, String>>> <- Hot.read.recs(h)
    +ids : String = Hot.ids(SN.SRows{recs})
    +rows : SN.SRows <- Hot.read.open(h, ids, recs)
    +seq : Nat = SN.Snap.num(Hot.get(m, "applied_seq"), 0n)
    +off : U32 = U32.from_nat(SN.Snap.num(Hot.get(m, "applied_off"), 0n))
    gaps : List<&2, List<&2, String>> <- D.Db.query.t(h, Hot.q.gaps(), [D.DbInt{U32.show(off)}], Hot.big())
    +ls : List<&2, String> <- Hot.lines(Hot.log(home), 0, off, Hot.gaps(gaps))
    Hot.lined(Nat.is_eq(List.length(&2, String, ls), seq), h, home, rows, ls, seq, off, size)

# why the store and the log do not go together ("": they do): its meta,
# the log's identity (SDB.Sdb.log_id: a store made from another log), and
# a log shorter than the store has read
def Hot.check(+m: Map<&2, String>, +size: U32, +id: String) -> String:
  +why = Hot.stale(m)
  Bool.pick(String, String.is_empty(why),
    Bool.pick(String, Bool.not(String.eq(Hot.get(m, "log_id"), id)), "store.db's log_id " ++ Hot.get(m, "log_id") ++ " is not the log's " ++ id,
      Bool.pick(String, Nat.is_gt(SN.Snap.num(Hot.get(m, "applied_off"), 0n), U32.to_nat(size)), "the log is shorter than store.db's applied_off", "")), why)

def Hot.head.done(r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(List<&2, U32>):
  (f, res) = r
  do IO<List<&2, U32>>:
    File.close(f)
    return Result.default(&1, &1, U32 & String, List<&2, U32>, res, Nil{})

def Hot.head.k(r: Result<&1, &1, U32 & String, File>) -> IO(List<&2, U32>):
  match r:
    case Done{f}:
      IO.bind(File & Result<&1, &1, U32 & String, List<&2, U32>>, List<&2, U32>, File.read_at(f, 0, 4096), Hot.head.done)
    case Fail{_}:
      IO.pure(List<&2, U32>, Nil{})

# the log's first 4096 bytes (its identity's)
def Hot.head(path: String) -> IO(List<&2, U32>):
  IO.bind(Result<&1, &1, U32 & String, File>, List<&2, U32>, File.open(path, "r"), Hot.head.k)

def Hot.checked.k(fine: Bool, +why: String, +h: String, +home: String, +m: Map<&2, String>, +size: U32) -> IO(Hot):
  match fine:
    case True{}:
      Hot.read(h, home, m, size)
    case False{}:
      do IO<Hot>:
        cl : String <- D.Db.close(h)
        return Hot.no(why)

def Hot.checked(+why: String, +h: String, +home: String, +m: Map<&2, String>, +size: U32) -> IO(Hot):
  Hot.checked.k(String.is_empty(why), why, h, home, m, size)

def Hot.opened(ok: Bool, +h: String, +o: String, +home: String) -> IO(Hot):
  match ok:
    case False{}:
      IO.pure(Hot, Hot.no("store.db: " ++ o))
    case True{}:
      do IO<Hot>:
        meta : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT k, v FROM meta", [], 1000)
        +m : Map<&2, String> = Hot.meta.go(meta, Map.new(&2, String))
        +size : U32 <- Hot.size(Hot.log(home))
        hd : List<&2, U32> <- Hot.head(Hot.log(home))
        Hot.checked(Hot.check(m, size, SDB.Sdb.log_id(hd)), h, home, m, size)

def Hot.load.k(there: Bool, +home: String) -> IO(Hot):
  match there:
    case False{}:
      IO.pure(Hot, Hot.no("no store.db"))
    case True{}:
      do IO<Hot>:
        +o : String <- D.Db.open(Hot.path(home))
        Hot.opened(String.starts_with(o, "ok "), String.drop(o, 3n), o, home)

# the read model from home's store.db and its log's tail, or why not
def Hot.load(+home: String) -> IO(Hot):
  do IO<Hot>:
    there : Bool <- Hot.exists(Hot.path(home))
    Hot.load.k(there, home)
