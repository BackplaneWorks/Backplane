import Base
import ./json.bend as J
import ./model.bend as M
import ./codec.bend as C
import ./mail.bend as ML
import ./thin.bend as T

# Thin views from the store (pure)
# --------------------------------
# The hub's thin views (a thread's page, a message read whole, search) used
# to read an index of every entry held in memory (thin.bend's Index). With
# the store (store.db, core/storedb.bend: entry, term) the hub asks SQL for
# the few rows a request needs and hands the pure renderers a page index
# (T.Index.window) holding just them, so every answer is the one the whole
# index gave. Here: the SQL text (constant; parameters only from the
# request) and the rows it answers read back. server/thinstore.bend runs
# them. Ordinals are entry.ord (the insertion count, T.Row.n); since ord
# grows with seq, "ord < X" is "seq < the seq of the first ord >= X".

# "no bound": 2^47, past any ordinal (the runtime's Nat stops at 2^48)
def Q.top() -> Nat:
  Nat.mul(65536n, 2147483648n)

# a number as a query parameter: NUL then its digits, which no id, word
# or text parameter starts with; the server binds it as an integer
# (server/thinstore.bend's Ts.val: D.Db's String params are always text)
def Q.nul() -> String:
  SCon{Chr{0}, SNil{}}

def Q.n(+n: Nat) -> String:
  Q.nul() ++ Nat.show(n)

def Q.num(s: String) -> Nat:
  M.Switch.num(s, 0n)

# an exclusive ordinal bound: 0 is none (the newest page)
def Q.before(+before: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_eq(before, 0n), Q.top(), before)

# the seq of the first entry at or after an ordinal (none: no bound)
def Q.sql.seq_at() -> String:
  "SELECT coalesce((SELECT seq FROM entry WHERE ord >= ?1 ORDER BY ord LIMIT 1), 140737488355328)"

def Q.cols.prefix() -> String:
  "seq, ord, id, thread, src, kind, role, akind, CASE WHEN kind = 'msg' THEN text ELSE substr(text, 1, 513) END AS text, at"

def Q.cols.full() -> String:
  "seq, ord, id, thread, src, kind, role, akind, text, at"

# a thread's rows (its own and the messages it sent elsewhere) older than
# seq ?2, newest first, at most ?3: T.Index's thread order, one window of it.
# An activity's text is cut to 513 characters: enough for its first 512-byte
# block and to know whether there is another (T.Blocks); a message's comes
# whole, as the thin view sends it (T.Row.preview: only activities fold).
def Q.sql.page() -> String:
  "SELECT * FROM (SELECT * FROM (SELECT " ++ Q.cols.prefix() ++ " FROM entry WHERE thread = ?1 AND seq < ?2 ORDER BY seq DESC LIMIT ?3)" ++
  " UNION ALL SELECT * FROM (SELECT " ++ Q.cols.prefix() ++ " FROM entry WHERE src = ?1 AND src <> '' AND seq < ?2 ORDER BY seq DESC LIMIT ?3))" ++
  " ORDER BY seq DESC LIMIT ?3"

# Fixed event-field predicate, case sensitive and independent of summary
# wording. Apply inside both branches BEFORE their limits and the union.
def Q.sql.chat.empty.assistant() -> String:
  "(kind = 'msg' AND role = 'assistant' AND NOT (substr(id, 1, 2) = 't:' AND instr(substr(id, 3), ':') > 1)" ++
  " AND trim(text, char(9) || char(10) || char(11) || char(12) || char(13) || char(32)) = '')"

def Q.sql.chat.keep() -> String:
  " AND NOT ((kind = 'act' AND role = 'info' AND akind = 'ride' AND substr(id, 1, 5) = 'ride:') OR " ++ Q.sql.chat.empty.assistant() ++ ")"

def Q.sql.page.chat() -> String:
  "SELECT * FROM (SELECT * FROM (SELECT " ++ Q.cols.prefix() ++ " FROM entry WHERE thread = ?1 AND seq < ?2" ++ Q.sql.chat.keep() ++ " ORDER BY seq DESC LIMIT ?3)" ++
  " UNION ALL SELECT * FROM (SELECT " ++ Q.cols.prefix() ++ " FROM entry WHERE src = ?1 AND src <> '' AND seq < ?2" ++ Q.sql.chat.keep() ++ " ORDER BY seq DESC LIMIT ?3))" ++
  " ORDER BY seq DESC LIMIT ?3"

# how many rows of a thread are older than seq ?2
def Q.sql.below() -> String:
  "SELECT (SELECT count(*) FROM entry WHERE thread = ?1 AND seq < ?2) + (SELECT count(*) FROM entry WHERE src = ?1 AND src <> '' AND seq < ?2)"

# whole rows: the ordinal ?1, and any row named ?2 among the ordinals ?3
# (a JSON array of numbers)
def Q.sql.whole() -> String:
  "SELECT " ++ Q.cols.full() ++ " FROM entry WHERE ord = ?1 OR (id = ?2 AND ord IN (SELECT value FROM json_each(?3))) ORDER BY ord DESC"

# a term's posting count
def Q.sql.count() -> String:
  "SELECT count(*) FROM term WHERE word = ?1"

# a term's postings older than ordinal ?2, newest first, at most 129 (the
# walk looks at 128; a 129th says there are more), each with its row and
# which of the query's words (?3, a JSON array) it has, joined by \u001f
def Q.sql.search.cols() -> String:
  "SELECT e.seq, e.ord, e.id, e.thread, e.src, e.kind, e.role, e.akind, CASE WHEN e.kind = 'msg' THEN e.text ELSE substr(e.text, 1, 513) END, e.at," ++
  " (SELECT group_concat(u.word, char(31)) FROM term u WHERE u.ord = t.ord AND u.word IN (SELECT value FROM json_each(?3)))"

def Q.sql.search() -> String:
  Q.sql.search.cols() ++ " FROM term t JOIN entry e ON e.ord = t.ord WHERE t.word = ?1 AND t.ord < ?2 ORDER BY t.ord DESC LIMIT 129"

# the same within one thread ?4 (its own rows and what it sent): the open
# thread's results, found even when a common word's newest uses are all
# in other threads (T.Index.search.in)
def Q.sql.search.in() -> String:
  Q.sql.search.cols() ++ " FROM term t JOIN entry e ON e.ord = t.ord WHERE t.word = ?1 AND t.ord < ?2 AND (e.thread = ?4 OR (e.src = ?4 AND e.src <> '')) ORDER BY t.ord DESC LIMIT 129"

# every thread with entries and its newest message from the person (0:
# none): the recent order's ranks (T.Index.recent), before threads' seeds
def Q.sql.recent() -> String:
  "SELECT thread, max(CASE WHEN kind = 'msg' AND role = 'user' THEN at ELSE 0 END) FROM entry GROUP BY thread"

# Rows read back
# --------------

def Q.entry.k(msg: Bool, id: String, th: String, r: String, ak: String, text: String, +at: Nat) -> M.Entry:
  match msg:
    case True{}: M.Msg{id, th, C.Role.of(r), text, at}
    case False{}: M.Act{id, th, r, ak, text, at}

def Q.terms(ws: List<&2, String>, m: Map<&2, Bool>) -> Map<&2, Bool>:
  match ws:
    case Nil{}: m
    case w <> t: Q.terms(t, Map.set(&2, Bool, m, w, True{}))

def Q.words(s: String) -> Map<&2, Bool>:
  Q.terms(List.filter(String, w => Bool.not(String.is_empty(w)), String.split(s, '\u{1f}')), Map.new(&2, Bool))

# a row [seq, ord, id, thread, src, kind, role, akind, text, at, words?]
def Q.row(cs: List<&2, String>) -> T.Row:
  match cs:
    case _ <> ord <> id <> th <> _ <> +k <> r <> ak <> +text <> at <> Nil{}:
      T.Row{Q.num(ord), Q.entry.k(String.eq(k, "msg"), id, th, r, ak, text, Q.num(at)), Map.new(&2, Bool), T.Blocks.of(text)}
    case _ <> ord <> id <> th <> _ <> +k <> r <> ak <> +text <> at <> ws <> _:
      T.Row{Q.num(ord), Q.entry.k(String.eq(k, "msg"), id, th, r, ak, text, Q.num(at)), Q.words(ws), T.Blocks.of(text)}
    case _:
      T.Row.blank()

def Q.rows(xs: List<&2, List<&2, String>>) -> List<&2, T.Row>:
  match xs:
    case Nil{}: Nil{}
    case x <> t: Q.row(x) <> Q.rows(t)

def Q.seq(cs: List<&2, String>) -> Nat:
  match cs:
    case s <> _: Q.num(s)
    case _: 0n

# the oldest row's seq (the last of a page)
def Q.last_seq(xs: List<&2, List<&2, String>>, +acc: Nat) -> Nat:
  match xs:
    case Nil{}: acc
    case x <> t: Q.last_seq(t, Q.seq(x))

def Q.first(xs: List<&2, List<&2, String>>) -> String:
  match xs:
    case (s <> _) <> _: s
    case _: ""

# a query's error row (Db.query: ["\u0000fail", why])
def Q.failed(xs: List<&2, List<&2, String>>) -> Bool:
  String.eq(Q.first(xs), "\u{0}fail")

def Q.json.nats(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case x <> t: J.Num{x} <> Q.json.nats(t)

def Q.json.strs(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case x <> t: J.Str{x} <> Q.json.strs(t)

def Q.words.json(ts: List<&2, Sigma<&2, &2, String, _ => Bool>>) -> List<&2, J.Json>:
  match ts:
    case Nil{}: Nil{}
    case (w, _) <> t: J.Str{w} <> Q.words.json(t)

# Search
# ------

# the term whose postings the walk takes (T.Search.base: the fewest, the
# first of equals), from each term's count in the query's order
def Q.base.go(ts: List<&2, Sigma<&2, &2, String, _ => Bool>>, ns: List<&2, Nat>, +best: String, +n: Nat) -> String:
  match ts ns:
    case Nil{} _: best
    case _ Nil{}: best
    case (+w, _) <> t +k <> r: Q.base.go(t, r, Bool.pick(String, Nat.is_lt(k, n), w, best), Bool.pick(Nat, Nat.is_lt(k, n), k, n))

def Q.base.first(ns: List<&2, Nat>, +w: String, t: List<&2, Sigma<&2, &2, String, _ => Bool>>) -> String:
  match ns:
    case +k <> r: Q.base.go(t, r, w, k)
    case Nil{}: ""

def Q.base(ts: List<&2, Sigma<&2, &2, String, _ => Bool>>, ns: List<&2, Nat>) -> String:
  match ts:
    case (+w, _) <> t: Q.base.first(ns, w, t)
    case Nil{}: ""

# the candidates as the posting order and an index of their rows
def Q.order(rs: List<&2, T.Row>, o: ML.Order) -> ML.Order:
  match rs:
    case Nil{}: o
    case +r <> t: Q.order(t, ML.Order.put(o, T.Row.n(r), Nat.show(T.Row.n(r))))

def Q.cands(rs: List<&2, T.Row>, i: T.Index) -> T.Index:
  match rs:
    case Nil{}: i
    case +r <> t: Q.cands(t, T.Index.window.put(i, "", r))

# the search answered over the fetched candidates (T.Index.search.on)
def Q.search.cap(ts: List<&2, Sigma<&2, &2, String, _ => Bool>>, +rs: List<&2, T.Row>, th: String, +hide: Map<&2, Bool>, +cap: Nat, +before: Nat) -> T.SearchPage:
  T.Index.search.cap(Q.order(rs, ML.OEmpty{}), Q.cands(rs, T.Index.empty()), ts, th, hide, cap, before)

def Q.search.hid(ts: List<&2, Sigma<&2, &2, String, _ => Bool>>, +rs: List<&2, T.Row>, th: String, +hide: Map<&2, Bool>, +before: Nat) -> T.SearchPage:
  Q.search.cap(ts, rs, th, hide, 25n, before)

def Q.search(ts: List<&2, Sigma<&2, &2, String, _ => Bool>>, +rs: List<&2, T.Row>, th: String, +before: Nat) -> T.SearchPage:
  Q.search.hid(ts, rs, th, Map.new(&2, Bool), before)

def Q.search.index(+rs: List<&2, T.Row>) -> T.Index:
  Q.cands(rs, T.Index.empty())

def Q.terms.of(q: String) -> List<&2, Sigma<&2, &2, String, _ => Bool>>:
  Map.to_list(&2, Bool, T.Words.of(q))

# Recent
# ------

def Q.recent.rows(xs: List<&2, List<&2, String>>, i: T.Index) -> T.Index:
  match xs:
    case Nil{}: i
    case (th <> n <> _) <> t: Q.recent.rows(t, T.Index.recent_put(i, th, Q.num(n)))
    case _ <> t: Q.recent.rows(t, i)

def Q.recent.seed(ts: List<&2, M.Thread>, i: T.Index) -> T.Index:
  match ts:
    case Nil{}: i
    case +t <> tail: Q.recent.seed(tail, T.Index.recent_put(i, M.Thread.id(t), M.Thread.rank(t)))

# the recent-only index T.Index.of(st) holds, from the store's ranks
def Q.recent(xs: List<&2, List<&2, String>>, +st: M.State) -> T.Index:
  Q.recent.seed(T.State.threads(st), Q.recent.rows(xs, T.Index.empty()))
