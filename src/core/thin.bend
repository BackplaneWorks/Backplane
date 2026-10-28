import Base
import ./model.bend as M
import ./mail.bend as ML

# Derived, process-local query indexes. Nothing here changes persisted events
# or claims that a client applied the canonical log. Entries are ordered by
# insertion ordinal, including equal timestamps and duplicate message IDs.
# Fixed UTF8 blocks are made once with the index, not rescanned per chunk.
type Blocks is Data:
  Blocks{count: Nat, parts: Map<&2, String>}

type BlockBuild is Data:
  BlockBuild{index: Nat, reversed: String, bytes: Nat, parts: Map<&2, String>}

def Bytes.char(+c: U32) -> Nat:
  Bool.pick(Nat, U32.is_lt(c, 128), 1n, Bool.pick(Nat, U32.is_lt(c, 2048), 2n, Bool.pick(Nat, U32.is_lt(c, 65536), 3n, 4n)))

def Blocks.step(fits: Bool, +c: U32, +size: Nat, +i: Nat, text: String, bytes: Nat, parts: Map<&2, String>) -> BlockBuild:
  match fits:
    case True{}: BlockBuild{i, SCon{Chr{c}, text}, Nat.add(bytes, size), parts}
    case False{}: BlockBuild{Nat.add(i, 1n), SCon{Chr{c}, SNil{}}, size, Map.set(&2, String, parts, Nat.show(i), String.reverse(text))}

def Blocks.go(s: String, b: BlockBuild) -> Blocks:
  match s b:
    case SNil{} BlockBuild{+i, text, _, parts}: Blocks{Nat.add(i, 1n), Map.set(&2, String, parts, Nat.show(i), String.reverse(text))}
    case SCon{Chr{+c}, tail} BlockBuild{i, text, +n, parts}: Blocks.go(tail, Blocks.step(Nat.is_le(Nat.add(n, Bytes.char(c)), 512n), c, Bytes.char(c), i, text, n, parts))

def Blocks.of(s: String) -> Blocks:
  Blocks.go(s, BlockBuild{0n, "", 0n, Map.new(&2, String)})

def Blocks.n(b: Blocks) -> Nat:
  match b:
    case Blocks{n, _}: n

def Blocks.get.k(p: Map<&2, String> & String) -> String:
  (_, text) = p
  text

def Blocks.get(b: Blocks, n: Nat) -> String:
  match b:
    case Blocks{_, parts}: Blocks.get.k(Map.get(String, "", parts, Nat.show(n)))

def Blocks.join(k: Nat, +b: Blocks, +n: Nat, acc: List<&2, String>) -> String:
  match k:
    case 0n: String.join(List.reverse(&2, String, acc), "")
    case 1n+rest: Blocks.join(rest, b, Nat.add(n, 1n), Blocks.get(b, n) <> acc)

type Row is Data:
  Row{ordinal: Nat, entry: M.Entry, terms: Map<&2, Bool>, blocks: Blocks}

# below: per thread, how many of its rows are older than every row of it
# the index holds (empty for an index of all history; a page index built
# from the store holds a window of rows and counts the rest here)
type Index is Data:
  Index{next: Nat, rows: Map<&2, Row>, threads: Map<&2, ML.Order>, words: Map<&2, ML.Order>, recent: ML.Order, thread_at: Map<&2, Nat>, below: Map<&2, Nat>}

def Index.empty() -> Index:
  Index{0n, Map.new(&2, Row), Map.new(&2, ML.Order), Map.new(&2, ML.Order), ML.OEmpty{}, Map.new(&2, Nat), Map.new(&2, Nat)}

def Row.blank() -> Row:
  Row{0n, M.Msg{"", "", M.Assistant{}, "", 0n}, Map.new(&2, Bool), Blocks.of("")}

def Row.entry(r: Row) -> M.Entry:
  match r:
    case Row{_, e, _, _}: e

def Row.n(r: Row) -> Nat:
  match r:
    case Row{n, _, _, _}: n

def Row.terms(r: Row) -> Map<&2, Bool>:
  match r:
    case Row{_, _, ts, _}: ts

def Index.rows(i: Index) -> Map<&2, Row>:
  match i:
    case Index{_, rs, _, _, _, _, _}: rs

def Index.n(i: Index) -> Nat:
  match i:
    case Index{n, _, _, _, _, _, _}: n

def Index.row.k(p: Map<&2, Row> & Row) -> Row:
  (_, r) = p
  r

def Index.row(+i: Index, n: Nat) -> Row:
  Index.row.k(Map.get(Row, Row.blank(), Index.rows(i), Nat.show(n)))

def Entry.text(e: M.Entry) -> String:
  match e:
    case M.Msg{_, _, _, s, _}: s
    case M.Act{_, _, _, _, s, _}: s

def Word.char(+c: U32) -> Bool:
  Bool.or(U32.is_ge(c, 128), Bool.or(Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122)), Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57))))

def Words.put(valid: Bool, ts: Map<&2, Bool>, word: String) -> Map<&2, Bool>:
  match valid:
    case False{}: ts
    case True{}: Map.set(&2, Bool, ts, String.reverse(word), True{})

type WordState is Data:
  WordState{word: String, n: Nat, terms: Map<&2, Bool>}

def Words.letter(short: Bool, c: U32, w: String, n: Nat, ts: Map<&2, Bool>) -> WordState:
  match short:
    case True{}: WordState{SCon{Chr{c}, w}, Nat.add(n, 1n), ts}
    case False{}: WordState{w, 65n, ts}

def Words.step(letter: Bool, c: U32, w: String, +n: Nat, ts: Map<&2, Bool>) -> WordState:
  match letter:
    case True{}: Words.letter(Nat.is_lt(n, 64n), c, w, n, ts)
    case False{}: WordState{"", 0n, Words.put(Bool.and(Nat.is_gt(n, 0n), Nat.is_le(n, 64n)), ts, w)}

def Words.code(c: Char) -> U32:
  match c:
    case Chr{u}: u

def Words.at(+c: U32, w: String, +n: Nat, ts: Map<&2, Bool>) -> WordState:
  Words.step(Word.char(c), c, w, n, ts)

# the walk over a String already lowered whole: what Words.go does
# (law thin_words_lower_each)
def Words.go.whole(s: String, ws: WordState) -> Map<&2, Bool>:
  match s ws:
    case SNil{} WordState{w, +n, ts}: Words.put(Bool.and(Nat.is_gt(n, 0n), Nat.is_le(n, 64n)), ts, w)
    case SCon{Chr{+c}, tail} WordState{w, n, ts}: Words.go.whole(tail, Words.step(Word.char(c), c, w, n, ts))

# each character lowered as it is read: String.to_lower builds its result
# outside its recursive call, so on the JS target a long message overflowed
# the stack while the index was built at start-up
def Words.go(s: String, ws: WordState) -> Map<&2, Bool>:
  match s ws:
    case SNil{} WordState{w, +n, ts}: Words.put(Bool.and(Nat.is_gt(n, 0n), Nat.is_le(n, 64n)), ts, w)
    case SCon{h, tail} WordState{w, n, ts}: Words.go(tail, Words.at(Words.code(Char.to_lower(h)), w, n, ts))

def Words.of(s: String) -> Map<&2, Bool>:
  Words.go(s, WordState{"", 0n, Map.new(&2, Bool)})

def Orders.get.k(p: Map<&2, ML.Order> & ML.Order) -> ML.Order:
  (_, o) = p
  o

def Orders.get(os: Map<&2, ML.Order>, k: String) -> ML.Order:
  Orders.get.k(Map.get(ML.Order, ML.OEmpty{}, os, k))

def Orders.put(+os: Map<&2, ML.Order>, +k: String, +n: Nat) -> Map<&2, ML.Order>:
  Map.set(&2, ML.Order, os, k, ML.Order.put(Orders.get(os, k), n, Nat.show(n)))

def Words.index(xs: List<&2, Sigma<&2, &2, String, _ => Bool>>, +n: Nat, os: Map<&2, ML.Order>) -> Map<&2, ML.Order>:
  match xs:
    case Nil{}: os
    case (word, _) <> tail: Words.index(tail, n, Orders.put(os, word, n))

def Recent.old.k(p: Map<&2, Nat> & Nat) -> Nat:
  (_, n) = p
  n

def Recent.old(at: Map<&2, Nat>, k: String) -> Nat:
  Recent.old.k(Map.get(Nat, 0n, at, k))

def Recent.update(+o: ML.Order, +at: Map<&2, Nat>, +th: String, +n: Nat) -> ML.Order:
  ML.Order.put(ML.Order.drop(o, Recent.old(at, th), th), n, th)

def Index.sender(has: Bool, os: Map<&2, ML.Order>, th: String, n: Nat) -> Map<&2, ML.Order>:
  match has:
    case False{}: os
    case True{}: Orders.put(os, th, n)

def Entry.rank(e: M.Entry) -> Nat:
  match e:
    case M.Msg{_, _, M.User{}, _, at}: at
    case _: 0n

def Index.recent_put(+i: Index, +thread: String, +rank: Nat) -> Index:
  match i:
    case Index{n, rs, ths, ws, recent, +at, bl}:
      +best = M.Time.max(Recent.old(at, thread), rank)
      Index{n, rs, ths, ws, Recent.update(recent, at, thread, best), Map.set(&2, Nat, at, thread, best), bl}

def Index.post.row(+i: Index, +e: M.Entry, +terms: Map<&2, Bool>) -> Index:
  match i:
    case Index{+next, rs, +ths, ws, +recent, +at, bl}:
      +n = Nat.add(next, 1n)
      +th = M.Idx.thread(e)
      +from = M.Link.from(M.Idx.id(e))
      Index{n, Map.set(&2, Row, rs, Nat.show(n), Row{n, e, terms, Blocks.of(Entry.text(e))}),
        Index.sender(Bool.and(Bool.not(String.is_empty(from)), Bool.not(String.eq(from, th))), Orders.put(ths, th, n), from, n),
        Words.index(Map.to_list(&2, Bool, terms), n, ws), Recent.update(recent, at, th, M.Time.max(Recent.old(at, th), Entry.rank(e))), Map.set(&2, Nat, at, th, M.Time.max(Recent.old(at, th), Entry.rank(e))), bl}

def Index.post(+i: Index, +e: M.Entry) -> Index:
  Index.post.row(i, e, Words.of(Entry.text(e)))

# Boot exactly once over old entries in their original insertion order.
# Commit updates use only the three real State.post producers below.
def Index.load(xs: List<&2, M.Entry>, i: Index) -> Index:
  match xs:
    case Nil{}: i
    case e <> tail: Index.load(tail, Index.post(i, e))

def Index.thread_seeds(ts: List<&2, M.Thread>, index: Index) -> Index:
  match ts:
    case Nil{}: index
    case +t <> tail: Index.thread_seeds(tail, Index.recent_put(index, M.Thread.id(t), M.Thread.rank(t)))

def State.threads(st: M.State) -> List<&2, M.Thread>:
  match st:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}: Map.values(&2, M.Thread, ts)

def Index.of(+st: M.State) -> Index:
  Index.thread_seeds(State.threads(st), Index.load(List.reverse(&2, M.Entry, M.State.entries.all(st)), Index.empty()))

def Index.change(i: Index, c: M.Change) -> Index:
  match c:
    case M.ThreadCreated{id, _, _, _, _, _, _, _, at}: Index.recent_put(i, id, at)
    case M.MessagePosted{th, id, role, text, at}: Index.post(i, M.Msg{id, th, role, text, at})
    case M.ActivityLogged{th, id, tone, kind, text, at}: Index.post(i, M.Act{id, th, tone, kind, text, at})
    case M.Reminded{id, _, th, text, +at}: Index.post(i, M.Act{"remind:" ++ id ++ ":" ++ Nat.show(at), th, "remind", "reminder", text, at})
    case _: i

def Index.changes(cs: List<&2, M.Change>, i: Index) -> Index:
  match cs:
    case Nil{}: i
    case c <> tail: Index.changes(tail, Index.change(i, c))

def Index.thread(i: Index, th: String) -> ML.Order:
  match i:
    case Index{_, _, ths, _, _, _, _}: Orders.get(ths, th)

def Index.word(i: Index, word: String) -> ML.Order:
  match i:
    case Index{_, _, _, ws, _, _, _}: Orders.get(ws, word)

# Rank of an immutable insertion anchor, bounded by AVL height. New entries
# ahead of the anchor cannot duplicate/skip the next older page.
def Rank.go(+t: ML.Order, c: Cmp, +n: Nat, +id: String, +acc: Nat) -> Nat:
  match t c:
    case ML.OEmpty{} _: acc
    case ML.ONode{_, _, _, _, +l, _} LT{}: Rank.go(l, ML.Order.root_cmp(l, n, id), n, id, acc)
    case ML.ONode{_, _, _, _, +l, _} EQ{}: Nat.add(acc, ML.Order.count(l))
    case ML.ONode{_, _, _, _, +l, +r} GT{}: Rank.go(r, ML.Order.root_cmp(r, n, id), n, id, Nat.add(acc, Nat.add(ML.Order.count(l), 1n)))

def Rank.of(+t: ML.Order, +n: Nat, +id: String) -> Nat:
  Rank.go(t, ML.Order.root_cmp(t, n, id), n, id, 0n)

def Page.skip(before: Nat, +o: ML.Order) -> Nat:
  match before:
    case 0n: 0n
    case +n: Rank.of(o, n, Nat.show(n))

# An exclusive ordinal anchor need not still be a member of a filtered tree.
# Its insertion rank excludes newer rows even if a new post arrived meanwhile.
def Page.after(+o: ML.Order, +before: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_eq(before, 0n), 0n, Rank.of(o, Nat.sub(before, 1n), Nat.show(Nat.sub(before, 1n))))

def Page.ids(+o: ML.Order, +before: Nat, +limit: Nat) -> List<&2, String>:
  ML.Order.slice(o, Page.after(o, before), Bool.pick(Nat, Nat.is_le(limit, 32n), limit, 32n))

def Index.page(+i: Index, th: String, before: Nat, limit: Nat) -> List<&2, String>:
  Page.ids(Index.thread(i, th), before, limit)

def Index.recent(i: Index) -> List<&2, String>:
  match i:
    case Index{_, _, _, _, o, _, _}: ML.Order.slice(o, 0n, 24n)

# Query selects the smallest posting list, then tests membership in already
# indexed term sets. It never walks message text or the full event log.
def Search.base.pick(smaller: Bool, best: ML.Order, next: ML.Order) -> ML.Order:
  match smaller:
    case True{}: next
    case False{}: best

def Search.base.go(terms: List<&2, Sigma<&2, &2, String, _ => Bool>>, +i: Index, +best: ML.Order) -> ML.Order:
  match terms:
    case Nil{}: best
    case (word, _) <> tail:
      +next = Index.word(i, word)
      Search.base.go(tail, i, Search.base.pick(Nat.is_lt(ML.Order.count(next), ML.Order.count(best)), best, next))

def Search.base(terms: List<&2, Sigma<&2, &2, String, _ => Bool>>, +i: Index) -> ML.Order:
  match terms:
    case Nil{}: ML.OEmpty{}
    case (word, _) <> tail: Search.base.go(tail, i, Index.word(i, word))

def Word.has.k(p: Map<&2, Bool> & Bool) -> Bool:
  (_, b) = p
  b

def Word.has(ts: Map<&2, Bool>, word: String) -> Bool:
  Word.has.k(Map.get(Bool, False{}, ts, word))

def Search.all(terms: List<&2, Sigma<&2, &2, String, _ => Bool>>, +ts: Map<&2, Bool>, valid: Bool) -> Bool:
  match terms valid:
    case _ False{}: False{}
    case Nil{} True{}: True{}
    case (word, _) <> tail True{}: Search.all(tail, ts, Word.has(ts, word))

def Search.owner(+th: String, +e: M.Entry) -> Bool:
  Bool.or(String.is_empty(th), Bool.or(String.eq(th, M.Idx.thread(e)), String.eq(th, M.Link.from(M.Idx.id(e)))))

type SearchPage is Data:
  SearchPage{ids: List<&2, String>, visited: Nat, last: String, more: Bool}

def Search.include(keep: Bool, id: String, acc: List<&2, String>) -> List<&2, String>:
  match keep:
    case True{}: id <> acc
    case False{}: acc

# hide: threads whose rows a search leaves out (a search over every
# thread: the deleted ones, Search.gone); left: results still to find (a
# page holds 25)
def Search.walk(xs: List<&2, String>, +i: Index, +terms: List<&2, Sigma<&2, &2, String, _ => Bool>>, +th: String, +hide: Map<&2, Bool>, +n: Nat, +left: Nat, +last: String, acc: List<&2, String>) -> SearchPage:
  match xs left:
    case _ 0n: SearchPage{List.reverse(&2, String, acc), n, last, True{}}
    case Nil{} _: SearchPage{List.reverse(&2, String, acc), n, last, False{}}
    case +id <> tail _:
      +r = Index.row.k(Map.get(Row, Row.blank(), Index.rows(i), id))
      +keep = Bool.and(Bool.and(Search.owner(th, Row.entry(r)), Bool.not(Word.has(hide, M.Idx.thread(Row.entry(r))))), Search.all(terms, Row.terms(r), True{}))
      Search.walk(tail, i, terms, th, hide, Nat.add(n, 1n), Nat.sub(left, Bool.pick(Nat, keep, 1n, 0n)), id, Search.include(keep, id, acc))

def Search.bound(page: SearchPage, full: Bool) -> SearchPage:
  match page:
    case SearchPage{ids, n, last, more}: SearchPage{ids, n, last, Bool.or(more, full)}

# the walk over a posting list o (the smallest of the terms'); a store
# hands in the candidates it fetched (ord < before, at most 129) and an
# index holding just those rows
def Index.search.cap(+o: ML.Order, +i: Index, +terms: List<&2, Sigma<&2, &2, String, _ => Bool>>, th: String, +hide: Map<&2, Bool>, +cap: Nat, +before: Nat) -> SearchPage:
  +skip = Page.after(o, before)
  Search.bound(Search.walk(ML.Order.slice(o, skip, 128n), i, terms, th, hide, 0n, cap, "", Nil{}), Nat.is_gt(ML.Order.count(o), Nat.add(skip, 128n)))

def Index.search.hid(+o: ML.Order, +i: Index, +terms: List<&2, Sigma<&2, &2, String, _ => Bool>>, th: String, +hide: Map<&2, Bool>, +before: Nat) -> SearchPage:
  Index.search.cap(o, i, terms, th, hide, 25n, before)

def Index.search.on(+o: ML.Order, +i: Index, +terms: List<&2, Sigma<&2, &2, String, _ => Bool>>, th: String, +before: Nat) -> SearchPage:
  Index.search.hid(o, i, terms, th, Map.new(&2, Bool), before)

def Index.search.terms(+i: Index, +terms: List<&2, Sigma<&2, &2, String, _ => Bool>>, th: String, +hide: Map<&2, Bool>, +cap: Nat, +before: Nat) -> SearchPage:
  Index.search.cap(Search.base(terms, i), i, terms, th, hide, cap, before)

def Index.search(i: Index, query: String, th: String, before: Nat) -> SearchPage:
  Index.search.terms(i, Map.to_list(&2, Bool, Words.of(query)), th, Map.new(&2, Bool), 25n, before)

# every thread's rows but those of threads deleted (a search from the top
# bar finds a word wherever it was said, Search.gone), at most cap
def Index.search.rest(i: Index, query: String, +hide: Map<&2, Bool>, +cap: Nat, before: Nat) -> SearchPage:
  Index.search.terms(i, Map.to_list(&2, Bool, Words.of(query)), "", hide, cap, before)

def Index.search.all(i: Index, query: String, +hide: Map<&2, Bool>, before: Nat) -> SearchPage:
  Index.search.rest(i, query, hide, 25n, before)

# a thread's own rows (and what it sent) that have the query's words:
# the walk goes over the word's uses in that thread, not over its newest
# uses anywhere (in a busy hub those may all be elsewhere); the store asks
# the same (thinq.bend's Q.sql.search.in)
def Search.in.put(keep: Bool, +n: Nat, o: ML.Order) -> ML.Order:
  match keep:
    case True{}: ML.Order.put(o, n, Nat.show(n))
    case False{}: o

def Search.in.go(xs: List<&2, String>, +i: Index, +th: String, o: ML.Order) -> ML.Order:
  match xs:
    case Nil{}: o
    case id <> tail:
      +r = Index.row.k(Map.get(Row, Row.blank(), Index.rows(i), id))
      Search.in.go(tail, i, th, Search.in.put(Search.owner(th, Row.entry(r)), Row.n(r), o))

# the open thread's results on the top bar's first page: at most ten, so
# the other threads' newest still show beside them
def Search.here() -> Nat:
  10n

def Search.in.terms(+i: Index, +terms: List<&2, Sigma<&2, &2, String, _ => Bool>>, +th: String, +o: ML.Order) -> SearchPage:
  Index.search.cap(Search.in.go(ML.Order.slice(o, 0n, ML.Order.count(o)), i, th, ML.OEmpty{}), i, terms, th, Map.new(&2, Bool), Search.here(), 0n)

def Index.search.in(+i: Index, query: String, +th: String) -> SearchPage:
  +terms = Map.to_list(&2, Bool, Words.of(query))
  Search.in.terms(i, terms, th, Search.base(terms, i))

# The top bar's search: the open thread's results (here: its first page),
# then every other thread's (rest: their results but the ones here had);
# the rest's walk goes on from its cursor (last, more) for the next pages
def Search.not.in(xs: List<&2, String>, +seen: Map<&2, Bool>, acc: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}: List.reverse(&2, String, acc)
    case +id <> tail: Search.not.in(tail, seen, Search.include(Bool.not(Word.has(seen, id)), id, acc))

def Search.seen(xs: List<&2, String>, acc: Map<&2, Bool>) -> Map<&2, Bool>:
  match xs:
    case Nil{}: acc
    case id <> tail: Search.seen(tail, Map.set(&2, Bool, acc, id, True{}))

# a page is 25 results and 128 rows looked at, here's and the rest's
# together (the rest asks for Search.left(here))
def Search.join(+here: SearchPage, +rest: SearchPage) -> SearchPage:
  match here rest:
    case SearchPage{+hs, +hn, _, _} SearchPage{rs, +rn, last, more}:
      SearchPage{List.append(&2, String, hs, Search.not.in(rs, Search.seen(hs, Map.new(&2, Bool)), Nil{})), Bool.pick(Nat, Nat.is_gt(Nat.add(hn, rn), 128n), 128n, Nat.add(hn, rn)), last, more}

def Search.left(+here: SearchPage) -> Nat:
  match here:
    case SearchPage{hs, _, _, _}: Nat.sub(25n, List.length(&2, String, hs))

def Search.none() -> SearchPage:
  SearchPage{Nil{}, 0n, "", False{}}

# the threads the rest leaves out: the deleted ones and the open one
def Search.gone.but(+gone: Map<&2, Bool>, +th: String) -> Map<&2, Bool>:
  Bool.pick(Map<&2, Bool>, String.is_empty(th), gone, Map.set(&2, Bool, gone, th, True{}))

# the threads a search over every thread leaves out: deleted ones
def Search.gone.k(gone: Bool, +id: String, acc: Map<&2, Bool>) -> Map<&2, Bool>:
  match gone:
    case True{}: Map.set(&2, Bool, acc, id, True{})
    case False{}: acc

def Search.gone.go(ts: List<&2, M.Thread>, acc: Map<&2, Bool>) -> Map<&2, Bool>:
  match ts:
    case Nil{}: acc
    case M.Thread{+id, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, de} <> tail: Search.gone.go(tail, Search.gone.k(de, id, acc))

def Search.gone(+st: M.State) -> Map<&2, Bool>:
  Search.gone.go(State.threads(st), Map.new(&2, Bool))

def Row.blocks(r: Row) -> Blocks:
  match r:
    case Row{_, _, _, b}: b

def Entry.with_text(e: M.Entry, text: String) -> M.Entry:
  match e:
    case M.Msg{id, th, role, _, at}: M.Msg{id, th, role, text, at}
    case M.Act{id, th, tone, kind, _, at}: M.Act{id, th, tone, kind, text, at}

# Only activity rows fold: a message's text is always sent whole (the
# person's order: no "read rest of message"); an activity shows its first
# block and window.partial advertises a chunk continuation for the rest.
def Row.folds(+e: M.Entry) -> Bool:
  match e:
    case M.Msg{_, _, _, _, _}: False{}
    case M.Act{_, _, _, _, _, _}: True{}

def Row.preview.k(fold: Bool, +r: Row) -> M.Entry:
  match fold:
    case True{}: Entry.with_text(Row.entry(r), Blocks.get(Row.blocks(r), 0n))
    case False{}: Row.entry(r)

def Row.preview(+r: Row) -> M.Entry:
  Row.preview.k(Row.folds(Row.entry(r)), r)

# a row shown cut: an activity longer than one block
def Row.partial(+r: Row) -> Bool:
  Bool.and(Row.folds(Row.entry(r)), Nat.is_gt(Blocks.n(Row.blocks(r)), 1n))

def Page.entries(ids: List<&2, String>, +i: Index, acc: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match ids:
    case Nil{}: List.reverse(&2, M.Entry, acc)
    case id <> tail: Page.entries(tail, i, Row.preview(Index.row.k(Map.get(Row, Row.blank(), Index.rows(i), id))) <> acc)

# rows of a thread older than ordinal `before`, counting those the index
# does not hold (below); what a page's "remaining" says
def Index.below(i: Index, th: String) -> Nat:
  match i:
    case Index{_, _, _, _, _, _, bl}: Recent.old(bl, th)

def Index.remaining(+i: Index, +th: String, +before: Nat) -> Nat:
  Nat.add(Nat.sub(ML.Order.count(Index.thread(i, th)), Page.after(Index.thread(i, th), before)), Index.below(i, th))

# ---- an index of the recent threads alone ----
# When the store holds the rows, the hub keeps of the index only what
# grows with threads, not with history: the recent order (Index.recent).
# These are Index.change's effects on it, and nothing else.
def Index.touch(+i: Index, +e: M.Entry) -> Index:
  Index.recent_put(i, M.Idx.thread(e), Entry.rank(e))

def Index.change.recent(i: Index, c: M.Change) -> Index:
  match c:
    case M.ThreadCreated{id, _, _, _, _, _, _, _, at}: Index.recent_put(i, id, at)
    case M.MessagePosted{th, id, role, text, at}: Index.touch(i, M.Msg{id, th, role, text, at})
    case M.ActivityLogged{th, id, tone, kind, text, at}: Index.touch(i, M.Act{id, th, tone, kind, text, at})
    case M.Reminded{id, _, th, text, +at}: Index.touch(i, M.Act{"remind:" ++ id ++ ":" ++ Nat.show(at), th, "remind", "reminder", text, at})
    case _: i

def Index.changes.recent(cs: List<&2, M.Change>, i: Index) -> Index:
  match cs:
    case Nil{}: i
    case c <> tail: Index.changes.recent(tail, Index.change.recent(i, c))

# A page index: an index holding a few rows the store gave (each with its
# terms when a search needs them), on the recent order of `base` (the
# hub's recent-only index). rows: newest first; below: the thread's rows
# older than all of them.
def Index.window.put(+i: Index, +th: String, +r: Row) -> Index:
  match i:
    case Index{next, rs, ths, ws, recent, at, bl}:
      Index{next, Map.set(&2, Row, rs, Nat.show(Row.n(r)), r), Orders.put(ths, th, Row.n(r)), ws, recent, at, bl}

def Index.window.rows(rs: List<&2, Row>, +th: String, +base: Index) -> Index:
  match rs:
    case Nil{}: base
    case +r <> tail:
      Index.window.rows(tail, th, Index.window.put(base, th, r))

def Index.window.below(+i: Index, +th: String, +n: Nat) -> Index:
  match i:
    case Index{next, rs, ths, ws, recent, at, _}: Index{next, rs, ths, ws, recent, at, Map.set(&2, Nat, Map.new(&2, Nat), th, n)}

def Index.recent_only(i: Index) -> Index:
  match i:
    case Index{next, _, _, _, recent, at, _}: Index{next, Map.new(&2, Row), Map.new(&2, ML.Order), Map.new(&2, ML.Order), recent, at, Map.new(&2, Nat)}

def Index.window(rs: List<&2, Row>, +th: String, +below: Nat, base: Index) -> Index:
  Index.window.rows(rs, th, Index.window.below(Index.recent_only(base), th, below))

# ---- the index on demand ----
# Building the index walks every entry's text (words, postings, blocks): a
# minute on a big log, so the hub must not do it before it listens. Later
# keeps the state the hub booted with and the changes committed since, in
# order; Lazy.force builds from them the index the eager boot (Index.of)
# followed by each commit (Index.changes) would have held (laws
# thin_lazy_*). A store can stand behind Lazy later.
type Lazy is Data:
  Built{index: Index}
  Later{st: M.State, done: List<&2, M.Change>}

def Lazy.of(st: M.State) -> Lazy:
  Later{st, Nil{}}

def Lazy.changes(cs: List<&2, M.Change>, l: Lazy) -> Lazy:
  match l:
    case Built{i}: Built{Index.changes(cs, i)}
    case Later{st, done}: Later{st, List.append(&2, M.Change, done, cs)}

def Lazy.force(l: Lazy) -> Index:
  match l:
    case Built{i}: i
    case Later{st, done}: Index.changes(done, Index.of(st))

def Lazy.ready(l: Lazy) -> Bool:
  match l:
    case Built{_}: True{}
    case Later{_, _}: False{}

# the state a background build starts from (Lazy.warm takes its result)
def Lazy.base(l: Lazy) -> M.State:
  match l:
    case Built{_}: M.State.empty()
    case Later{st, _}: st

# `i` is Index.of(Lazy.base(l)), built away from the hub: the commits it
# missed are applied on arrival; an index already built is kept
def Lazy.warm(i: Index, l: Lazy) -> Lazy:
  match l:
    case Built{b}: Built{b}
    case Later{_, done}: Built{Index.changes(done, i)}
