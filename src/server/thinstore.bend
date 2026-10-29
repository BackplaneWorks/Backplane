import Base
import ../core/json.bend as J
import ../core/model.bend as M
import ../core/thin.bend as T
import ../core/thin-chat.bend as TC
import ../core/thinq.bend as Q
import ../core/mail.bend as ML
import ../core/mailq.bend as MQ
import ../core/mailroom_history.bend as RH
import ../core/presentation.bend as P
import ./db.bend as D

# Thin views from the store (IO): the queries core/thinq.bend writes, run on
# a store.db handle (D.Db), each answer a page index the pure renderers read
# as they read the whole index. Nothing here keeps rows between requests.
# (S1's server/storedb.bend Sto.page/Sto.search/Sto.entry will take these
# over with the same shapes.)

# Query parameters typed for D.Db: a number Q.n made (NUL then digits)
# binds as an integer, anything else as text
def Ts.val.k(nul: Bool, +s: String) -> D.DbVal:
  match nul:
    case True{}: D.DbInt{String.drop(s, 1n)}
    case False{}: D.DbText{s}

def Ts.val(+s: String) -> D.DbVal:
  Ts.val.k(String.starts_with(s, Q.Q.nul()), s)

def Ts.vals(ps: List<&2, String>, acc: List<&2, D.DbVal>) -> List<&2, D.DbVal>:
  match ps:
    case Nil{}: List.reverse(&2, D.DbVal, acc)
    case p <> t: Ts.vals(t, Ts.val(p) <> acc)

def Ts.query(h: String, sql: String, ps: List<&2, String>, limit: U32) -> IO(List<&2, List<&2, String>>):
  D.Db.query.t(h, sql, Ts.vals(ps, Nil{}), limit)

type Page is Data:
  Page{index: T.Index, ids: List<&2, String>}

def Page.index(p: Page) -> T.Index:
  match p:
    case Page{i, _}: i

def Page.ids(p: Page) -> List<&2, String>:
  match p:
    case Page{_, ids}: ids

def Ts.nat(xs: List<&2, List<&2, String>>) -> Nat:
  Q.Q.num(Q.Q.first(xs))

def Ts.seq_at(+h: String, +before: Nat) -> IO(Nat):
  do IO<Nat>:
    r : List<&2, List<&2, String>> <- Ts.query(h, Q.Q.sql.seq_at(), [Q.Q.n(Q.Q.before(before))], 1)
    return Ts.nat(r)

# the thread's rows older than the page's oldest, when the page is full
# (one more than the page: a row older than it exists)
def Ts.below(full: Bool, +h: String, +th: String, +seq: Nat) -> IO(Nat):
  match full:
    case False{}: IO.pure(Nat, 0n)
    case True{}:
      do IO<Nat>:
        r : List<&2, List<&2, String>> <- Ts.query(h, Q.Q.sql.below(), [th, Q.Q.n(seq)], 1)
        return Ts.nat(r)

# a thread's page older than ordinal `before` (0: the newest), at most
# `limit` (capped at 32) and one more, as T.Index.page would give it, and
# what Page.window asks of the rest
def Ts.page(+h: String, +th: String, +before: Nat, +limit: Nat, +base: T.Index) -> IO(Page):
  +lim = Nat.add(Bool.pick(Nat, Nat.is_le(limit, 32n), limit, 32n), 1n)
  do IO<Page>:
    +s : Nat <- Ts.seq_at(h, before)
    +xs : List<&2, List<&2, String>> <- Ts.query(h, Q.Q.sql.page(), [th, Q.Q.n(s), Q.Q.n(lim)], U32.from_nat(lim))
    +n : Nat <- Ts.below(Nat.is_eq(List.length(&2, List<&2, String>, xs), lim), h, th, Q.Q.last_seq(xs, 0n))
    +i : T.Index = T.Index.window(Q.Q.rows(xs), th, n, base)
    return Page{i, T.Index.page(i, th, before, limit)}

# Presentation-only bot page. Keep a visible lookahead in the bounded page
# index so the normal window/cursor helper cannot offer receipt-only pages.
# Ordinary materialization, whole-entry reads and search remain unchanged.
def Ts.page.chat(enabled: Bool, +h: String, +th: String, +before: Nat, +limit: Nat, +base: T.Index) -> IO(Page):
  match enabled:
    case False{}: Ts.page(h, th, before, limit, base)
    case True{}:
      +lim = Nat.add(1n, TC.Chat.limit(limit))
      do IO<Page>:
        +s : Nat <- Ts.seq_at(h, before)
        +xs : List<&2, List<&2, String>> <- Ts.query(h, Q.Q.sql.page.chat(), [th, Q.Q.n(s), Q.Q.n(lim)], U32.from_nat(lim))
        +i : T.Index = T.Index.window(Q.Q.rows(xs), th, 0n, base)
        return Page{i, T.Index.page(i, th, before, limit)}

# Web body-window capability only. The extra row remains a real chronology
# lookahead and no large body is materialized by this ordinary page query.
def Ts.page.body(+h: String, +th: String, +before: Nat, +base: T.Index, +hide: Bool) -> IO(Page):
  do IO<Page>:
    +s : Nat <- Ts.seq_at(h, before)
    +xs : List<&2, List<&2, String>> <- Ts.query(h, Q.Q.sql.page.body(Bool.pick(String, hide, Q.Q.sql.chat.keep(), "")), [th, Q.Q.n(s), Q.Q.n(33n)], 33)
    +n : Nat <- Ts.below(Nat.is_eq(List.length(&2, List<&2, String>, xs), 33n), h, th, Q.Q.last_seq(xs, 0n))
    +i : T.Index = T.Index.window(Q.Q.rows(xs), th, n, base)
    return Page{i, T.Index.page(i, th, before, 32n)}

# A message read whole: the rows the read may name (the cursor's ordinal,
# or the entry `id` among the page's rows), whole, in the page's index
def Ts.whole(+h: String, +th: String, +ord: Nat, +id: String, +p: Page) -> IO(Page):
  match p:
    case Page{+i, +ids}:
      do IO<Page>:
        +xs : List<&2, List<&2, String>> <- Ts.query(h, Q.Q.sql.whole(), [Q.Q.n(ord), id, J.Json.show(J.Json.arr(Q.Q.json.nats(ids)))], 64)
        return Page{T.Index.window(Q.Q.rows(xs), th, 0n, i), ids}

# the ordinal a message read's cursor names ("message:<n>"; 0: none)
def Ts.cursor.ord.q(ok: Bool, +q: String) -> Nat:
  match ok:
    case False{}: 0n
    case True{}: Maybe.default(&2, Nat, P.Session.nat(String.drop(q, 8n), P.Session.nat.max()), 0n)

def Ts.cursor.ord(+p: J.Json) -> Nat:
  +q = J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(J.Json.get_str(p, "cursor", "")), J.Null{}), "query", "")
  Ts.cursor.ord.q(String.starts_with(q, "message:"), q)

def Ts.counts(ts: List<&2, Sigma<&2, &2, String, _ => Bool>>, +h: String) -> IO(List<&2, Nat>):
  match ts:
    case Nil{}: IO.pure(List<&2, Nat>, Nil{})
    case (+w, _) <> t:
      do IO<List<&2, Nat>>:
        r : List<&2, List<&2, String>> <- Ts.query(h, Q.Q.sql.count(), [w], 1)
        rest : List<&2, Nat> <- Ts.counts(t, h)
        return Ts.nat(r) <> rest

type Found is Data:
  Found{page: T.SearchPage, index: T.Index}

def Found.page(f: Found) -> T.SearchPage:
  match f:
    case Found{p, _}: p

def Found.index(f: Found) -> T.Index:
  match f:
    case Found{_, i}: i

def Ts.search.terms(empty: Bool, +ts: List<&2, Sigma<&2, &2, String, _ => Bool>>, +h: String, +th: String, +hide: Map<&2, Bool>, +before: Nat) -> IO(Found):
  match empty:
    case True{}: IO.pure(Found, Found{T.SearchPage{Nil{}, 0n, "", False{}}, T.Index.empty()})
    case False{}:
      do IO<Found>:
        ns : List<&2, Nat> <- Ts.counts(ts, h)
        +xs : List<&2, List<&2, String>> <- Ts.query(h, Q.Q.sql.search(), [Q.Q.base(ts, ns), Q.Q.n(Q.Q.before(before)), J.Json.show(J.Json.arr(Q.Q.words.json(ts)))], 129)
        +rs : List<&2, T.Row> = Q.Q.rows(xs)
        return Found{Q.Q.search.hid(ts, rs, th, hide, before), Q.Q.search.index(rs)}

# T.Index.search over the store: the same rows, visits, cursor and more;
# hide: threads left out (T.Index.search.all)
def Ts.search.hid(+h: String, +q: String, +th: String, +hide: Map<&2, Bool>, +before: Nat) -> IO(Found):
  +ts = Q.Q.terms.of(q)
  Ts.search.terms(List.is_empty(&2, Sigma<&2, &2, String, _ => Bool>, ts), ts, h, th, hide, before)

def Ts.search(+h: String, +q: String, +th: String, +before: Nat) -> IO(Found):
  Ts.search.hid(h, q, th, Map.new(&2, Bool), before)

# the open thread's candidates (on the first page only)
def Ts.rows.in(first: Bool, +h: String, +base: String, +words: String, +th: String) -> IO(List<&2, T.Row>):
  match first:
    case False{}: IO.pure(List<&2, T.Row>, Nil{})
    case True{}:
      do IO<List<&2, T.Row>>:
        +xs : List<&2, List<&2, String>> <- Ts.query(h, Q.Q.sql.search.in(), [base, Q.Q.n(Q.Q.top()), words, th], 129)
        return Q.Q.rows(xs)

def Ts.search.top.terms(empty: Bool, +ts: List<&2, Sigma<&2, &2, String, _ => Bool>>, +h: String, +th: String, +hide: Map<&2, Bool>, +before: Nat) -> IO(Found):
  match empty:
    case True{}: IO.pure(Found, Found{T.Search.none(), T.Index.empty()})
    case False{}:
      do IO<Found>:
        ns : List<&2, Nat> <- Ts.counts(ts, h)
        +base : String = Q.Q.base(ts, ns)
        +words : String = J.Json.show(J.Json.arr(Q.Q.words.json(ts)))
        +here : List<&2, T.Row> <- Ts.rows.in(Bool.and(Nat.is_eq(before, 0n), Bool.not(String.is_empty(th))), h, base, words, th)
        +hp : T.SearchPage = Q.Q.search.cap(ts, here, th, Map.new(&2, Bool), T.Search.here(), 0n)
        +xs : List<&2, List<&2, String>> <- Ts.query(h, Q.Q.sql.search(), [base, Q.Q.n(Q.Q.before(before)), words], 129)
        +rs : List<&2, T.Row> = Q.Q.rows(xs)
        return Found{T.Search.join(hp, Q.Q.search.cap(ts, rs, "", T.Search.gone.but(hide, th), T.Search.left(hp), before)),
          Q.Q.search.index(List.append(&2, T.Row, here, rs))}

# the top bar's search over the store (T.Search.join): the open thread's
# results first (on the first page), then every other thread's but the
# deleted ones (hide)
def Ts.search.top(+h: String, +q: String, +th: String, +hide: Map<&2, Bool>, +before: Nat) -> IO(Found):
  +ts = Q.Q.terms.of(q)
  Ts.search.top.terms(List.is_empty(&2, Sigma<&2, &2, String, _ => Bool>, ts), ts, h, th, hide, before)

# the recent-only index (Index.of's recent order) from the store and st
def Ts.recent(+h: String, +st: M.State) -> IO(T.Index):
  do IO<T.Index>:
    xs : List<&2, List<&2, String>> <- Ts.query(h, Q.Q.sql.recent(), Nil{}, 4294967295)
    return Q.Q.recent(xs, st)

# The mail box's headers from the store (core/mailq.bend)
# --------------------------------------------------------

# Box.newest(box, conv)
def Ms.newest(+h: String, +conv: String) -> IO(ML.Mail):
  do IO<ML.Mail>:
    xs : List<&2, List<&2, String>> <- Ts.query(h, MQ.Mq.sql.newest(), [conv], 1)
    return MQ.Mq.first(xs)

# the header with this id in the conversation (Mail.id "" when none)
def Ms.one(+h: String, +conv: String, +id: String) -> IO(ML.Mail):
  do IO<ML.Mail>:
    xs : List<&2, List<&2, String>> <- Ts.query(h, MQ.Mq.sql.one(), [conv, id], 1)
    return MQ.Mq.first(xs)

# Box.has(box, id)
def Ms.has(+h: String, +id: String) -> IO(Bool):
  do IO<Bool>:
    xs : List<&2, List<&2, String>> <- Ts.query(h, MQ.Mq.sql.has(), [id], 1)
    return Nat.is_gt(MQ.Mq.count(xs), 0n)

# Box.headers(box, conv) after the header `after` ("": from the head), at
# most n, in the box's order
def Ms.headers(+h: String, +conv: String, +after: String, +n: Nat) -> IO(List<&2, ML.Mail>):
  do IO<List<&2, ML.Mail>>:
    xs : List<&2, List<&2, String>> <- Ts.query(h, MQ.Mq.sql.page(), [conv, after, Q.Q.n(n)], U32.from_nat(n))
    return MQ.Mq.mails(xs)

def Ms.known(+h: String, +sql: String, +ps: List<&2, String>) -> IO(Bool):
  do IO<Bool>:
    xs : List<&2, List<&2, String>> <- Ts.query(h, sql, ps, 1)
    return Nat.is_gt(MQ.Mq.count(xs), 0n)

def Ms.cursor(empty: Bool, +h: String, +conv: String, +before: String) -> IO(Bool):
  match empty:
    case True{}: IO.pure(Bool, True{})
    case False{}: Ms.known(h, "SELECT count(*) FROM mail WHERE conv = ?1 AND id = ?2", [conv, before])

# RH.RoomHistory.page over the store
def Ms.room.page(+h: String, +conv: String, +before: String, +n: Nat) -> IO(Maybe<&2, RH.RoomPage>):
  do IO<Maybe<&2, RH.RoomPage>>:
    +known : Bool <- Ms.known(h, MQ.Mq.sql.conv(), [conv])
    +at : Bool <- Ms.cursor(String.is_empty(before), h, conv, before)
    xs : List<&2, List<&2, String>> <- Ts.query(h, MQ.Mq.sql.page(), [conv, before, Q.Q.n(Nat.add(MQ.Mq.cap(n), 1n))], U32.from_nat(Nat.add(MQ.Mq.cap(n), 1n)))
    return MQ.Mq.page(known, at, xs, n)

def Ms.ids(ms: List<&2, ML.Mail>) -> List<&2, J.Json>:
  match ms:
    case Nil{}: Nil{}
    case m <> t: J.Str{ML.Mail.id(m)} <> Ms.ids(t)

# RH.RoomHistory.sources over the store's posts
def Ms.room.sources(+h: String, +conv: String, ms: List<&2, ML.Mail>) -> IO(Map<&2, String>):
  do IO<Map<&2, String>>:
    xs : List<&2, List<&2, String>> <- Ts.query(h, MQ.Mq.sql.sources(), [String.drop(conv, 5n), J.Json.show(J.Json.arr(Ms.ids(ms)))], 4294967295)
    return MQ.Mq.sources(xs, Map.new(&2, String))
