import Base
import ./json.bend as J
import ./menu.bend as MN
import ./mail.bend as ML

# The Mailroom's view model, pure, so the window, web and phones only draw.
# Every decision lives here: which rows of a list show, how a search
# narrows, how a conversation groups into bubbles, who a reply can go to,
# what a badge says. Shapes in are spec-2b section 3 (mail.list, mail.read).
#
# Times are epoch seconds. Rows are a fixed 52 px. A page is 50 rows and
# starts at a multiple of 50 (the clients ask for pages that way).

def Mv.rh() -> Nat:
  52n

def Mv.page() -> Nat:
  50n

# One conversation in the list.
type Row is Data:
  Row{id: String, subject: String, who: String, head: String, at: Nat, unread: Bool, status: String, project: String, live: Bool, hay: String}

def Row.id(+r: Row) -> String:
  match r:
    case Row{+v, _, _, _, _, _, _, _, _, _}:
      v

def Row.subject(+r: Row) -> String:
  match r:
    case Row{_, +v, _, _, _, _, _, _, _, _}:
      v

def Row.who(+r: Row) -> String:
  match r:
    case Row{_, _, +v, _, _, _, _, _, _, _}:
      v

def Row.head(+r: Row) -> String:
  match r:
    case Row{_, _, _, +v, _, _, _, _, _, _}:
      v

def Row.at(+r: Row) -> Nat:
  match r:
    case Row{_, _, _, _, +v, _, _, _, _, _}:
      v

def Row.unread(+r: Row) -> Bool:
  match r:
    case Row{_, _, _, _, _, +v, _, _, _, _}:
      v

def Row.status(+r: Row) -> String:
  match r:
    case Row{_, _, _, _, _, _, +v, _, _, _}:
      v

def Row.project(+r: Row) -> String:
  match r:
    case Row{_, _, _, _, _, _, _, +v, _, _}:
      v

def Row.live(+r: Row) -> Bool:
  match r:
    case Row{_, _, _, _, _, _, _, _, +v, _}:
      v

def Row.hay(+r: Row) -> String:
  match r:
    case Row{_, _, _, _, _, _, _, _, _, +v}:
      v

def Mv.make(+id: String, +subject: String, +who: String, +head: String, +at: Nat, +unread: Bool, +status: String, +project: String) -> Row:
  Row{id, subject, who, head, at, unread, status, project, False{}, String.to_lower(subject ++ " " ++ who ++ " " ++ head ++ " " ++ project)}

def Mv.empty_row() -> Row:
  Mv.make("", "", "", "", 0n, False{}, "", "")

def Mv.stub(+id: String, +at: Nat) -> Row:
  Mv.make(id, id, "", "", at, False{}, "open", "")

# Visible window
# --------------
# The rows to build for a list `view` px high scrolled `scroll` px, over
# `total` rows: the rows in view plus 2 above and 2 below. O(1).

type Span is Data:
  Span{first: Nat, count: Nat}

def Mv.first_raw(+scroll: Nat) -> Nat:
  Nat.sub(Nat.div(scroll, 52n), 2n)

# rows from first_raw to two past the last row any pixel of the view touches
def Mv.want(+scroll: Nat, +view: Nat) -> Nat:
  Nat.sub(Nat.add(Nat.div(Nat.add(Nat.add(scroll, view), 51n), 52n), 2n), Mv.first_raw(scroll))

def Mv.count(+total: Nat, +scroll: Nat, +view: Nat) -> Nat:
  MN.Menu.min(Mv.want(scroll, view), total)

def Mv.first(+total: Nat, +scroll: Nat, +view: Nat) -> Nat:
  MN.Menu.off(Mv.first_raw(scroll), total, Mv.count(total, scroll, view))

def Mv.visible(+total: Nat, +scroll: Nat, +view: Nat) -> Span:
  Span{Mv.first(total, scroll, view), Mv.count(total, scroll, view)}

# the list's content height, and the y of row i
def Mv.height(+total: Nat) -> Nat:
  Nat.mul(total, 52n)

def Mv.y(+i: Nat) -> Nat:
  Nat.mul(i, 52n)

# Filters
# -------
# The chips. bot "*" is All mail. The server filters on bot, status and
# project; q is the local search and is not part of the cache key.

type Filter is Data:
  Filter{bot: String, status: String, project: String, q: String}

def Filter.bot(+f: Filter) -> String:
  match f:
    case Filter{+v, _, _, _}:
      v

def Filter.status(+f: Filter) -> String:
  match f:
    case Filter{_, +v, _, _}:
      v

def Filter.project(+f: Filter) -> String:
  match f:
    case Filter{_, _, +v, _}:
      v

def Filter.q(+f: Filter) -> String:
  match f:
    case Filter{_, _, _, +v}:
      v

def Mv.filter() -> Filter:
  Filter{"*", "", "", ""}

def Mv.for_bot(+bot: String) -> Filter:
  Filter{bot, "", "", ""}

# Length-prefixed, so two different (bot, status, project) never give one key.
def Mv.key(+f: Filter) -> String:
  match f:
    case Filter{+bot, +status, +project, _}:
      Nat.show(String.length(bot)) ++ ":" ++ bot ++ Nat.show(String.length(status)) ++ ":" ++ status ++ project

def Mv.with_bot(+f: Filter, +v: String) -> Filter:
  match f:
    case Filter{_, +s, +p, +q}:
      Filter{v, s, p, q}

def Mv.with_status(+f: Filter, +v: String) -> Filter:
  match f:
    case Filter{+b, _, +p, +q}:
      Filter{b, v, p, q}

def Mv.with_project(+f: Filter, +v: String) -> Filter:
  match f:
    case Filter{+b, +s, _, +q}:
      Filter{b, s, v, q}

def Mv.with_q(+f: Filter, +v: String) -> Filter:
  match f:
    case Filter{+b, +s, +p, _}:
      Filter{b, s, p, v}

# a status chip: pressed again it clears
def Mv.toggle(+f: Filter, +id: String) -> Filter:
  Mv.with_status(f, Bool.pick(String, String.eq(Filter.status(f), id), "", id))

type Chip is Data:
  Chip{id: String, label: String, on: Bool}

def Mv.chip(+f: Filter, +id: String) -> Chip:
  Chip{id, id, String.eq(Filter.status(f), id)}

def Mv.chips.project(+p: String) -> List<&2, Chip>:
  match p:
    case SNil{}:
      Nil{}
    case _:
      [Chip{"project", p, True{}}]

def Mv.chips(+f: Filter) -> List<&2, Chip>:
  List.append(&2, Chip, [Mv.chip(f, "unread"), Mv.chip(f, "open"), Mv.chip(f, "snoozed"), Mv.chip(f, "done")], Mv.chips.project(Filter.project(f)))

# Labels
# ------
# An address is `bot:<id>`, `thread:<id>`, `room:<id>`, `person` or a bare
# system name. `names` maps an address to what to show (a bot's name, a
# thread's title); anything missing shows its id, shortened.

def Mv.strip(+a: String) -> String:
  Bool.pick(String, String.starts_with(a, "bot:"), String.drop(a, 4n),
    Bool.pick(String, String.starts_with(a, "thread:"), String.drop(a, 7n),
      Bool.pick(String, String.starts_with(a, "room:"), String.drop(a, 5n), a)))

def Mv.short(+s: String, +n: Nat) -> String:
  Bool.pick(String, Nat.is_gt(String.length(s), n), String.take(s, n) ++ "…", s)

def Mv.name.k(+n: String, +a: String) -> String:
  Bool.pick(String, String.eq(a, "person"), "you", Bool.pick(String, String.is_empty(n), Mv.short(Mv.strip(a), 8n), n))

def Mv.name(+names: Map<&2, String>, +a: String) -> String:
  Mv.name.k(ML.Get.str(names, a), a)

def Mv.str(+j: J.Json) -> String:
  match j:
    case J.Str{+s}:
      s
    case _:
      ""

def Mv.strs(+xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> +t:
      Mv.str(x) <> Mv.strs(t)

def Mv.cons_if(+keep: Bool, +l: String, +rest: List<&2, String>) -> List<&2, String>:
  match keep:
    case True{}:
      l <> rest
    case False{}:
      rest

# labels of the participants other than me
def Mv.labels(+ps: List<&2, String>, +me: String, +names: Map<&2, String>) -> List<&2, String>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> +t:
      Mv.cons_if(Bool.not(String.eq(p, me)), Mv.name(names, p), Mv.labels(t, me, names))

def Mv.who.more(+rest: List<&2, String>) -> String:
  match rest:
    case Nil{}:
      ""
    case _:
      " +" ++ Nat.show(List.length(&2, String, rest))

def Mv.who.two(+a: String, +t: List<&2, String>) -> String:
  match t:
    case Nil{}:
      a
    case +b <> +rest:
      Mv.short(a, 14n) ++ ", " ++ Mv.short(b, 14n) ++ Mv.who.more(rest)

# "alice, bob +2"
def Mv.who(+ls: List<&2, String>) -> String:
  match ls:
    case Nil{}:
      ""
    case +a <> +t:
      Mv.who.two(a, t)

# the age of `at` at `now`: now, 5m, 3h, 2d
def Mv.ago.k(+d: Nat) -> String:
  Bool.pick(String, Nat.is_lt(d, 60n), "now",
    Bool.pick(String, Nat.is_lt(d, 3600n), Nat.show(Nat.div(d, 60n)) ++ "m",
      Bool.pick(String, Nat.is_lt(d, 86400n), Nat.show(Nat.div(d, 3600n)) ++ "h", Nat.show(Nat.div(d, 86400n)) ++ "d")))

def Mv.ago(+now: Nat, +at: Nat) -> String:
  Mv.ago.k(Nat.sub(now, at))

# A row from one `convs` entry of mail.list / mail.find.
def Mv.row(+j: J.Json, +me: String, +names: Map<&2, String>) -> Row:
  Mv.make(J.Json.get_str(j, "id", ""), J.Json.get_str(j, "subject", ""), Mv.who(Mv.labels(Mv.strs(J.Json.elems(J.Json.get_obj(j, "parts"))), me, names)), J.Json.get_str(j, "head", ""), J.Json.get_nat(j, "last", 0n), J.Json.get_bool(j, "unread", False{}), J.Json.get_str(j, "status", "open"), J.Json.get_str(j, "project", ""))

def Mv.rows_of(+xs: List<&2, J.Json>, +me: String, +names: Map<&2, String>) -> List<&2, Row>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> +t:
      Mv.row(x, me, names) <> Mv.rows_of(t, me, names)

def Mv.parse(+j: J.Json, +me: String, +names: Map<&2, String>) -> List<&2, Row>:
  Mv.rows_of(J.Json.elems(J.Json.get_obj(j, "convs")), me, names)

# Conversation view
# -----------------

type Msg is Data:
  Msg{id: String, from: String, to: String, kind: String, want: String, at: Nat, text: String}

# What the client holds of one conversation: its header and its mails, kept
# in a Map under zero-padded time ++ id, so a walk is oldest first and a mail
# that arrives twice is one.
type Read is Data:
  Read{id: String, subject: String, parts: List<&2, String>, project: String, status: String, more: Bool, msgs: Map<&2, Msg>}

def Read.id(+r: Read) -> String:
  match r:
    case Read{+v, _, _, _, _, _, _}:
      v

def Read.subject(+r: Read) -> String:
  match r:
    case Read{_, +v, _, _, _, _, _}:
      v

def Read.parts(+r: Read) -> List<&2, String>:
  match r:
    case Read{_, _, +v, _, _, _, _}:
      v

def Read.project(+r: Read) -> String:
  match r:
    case Read{_, _, _, +v, _, _, _}:
      v

def Read.status(+r: Read) -> String:
  match r:
    case Read{_, _, _, _, +v, _, _}:
      v

def Read.more(+r: Read) -> Bool:
  match r:
    case Read{_, _, _, _, _, +v, _}:
      v

def Read.msgs(+r: Read) -> Map<&2, Msg>:
  match r:
    case Read{_, _, _, _, _, _, +v}:
      v

def Mv.empty_read() -> Read:
  Read{"", "", Nil{}, "", "open", False{}, Map.new(&2, Msg)}

def Mv.pad(+s: String) -> String:
  String.take("000000000000", Nat.sub(12n, String.length(s))) ++ s

def Msg.key(+m: Msg) -> String:
  match m:
    case Msg{+id, _, _, _, _, +at, _}:
      Mv.pad(Nat.show(at)) ++ "/" ++ id

def Mv.msg(+j: J.Json) -> Msg:
  Msg{J.Json.get_str(j, "id", ""), J.Json.get_str(j, "from", ""), J.Json.get_str(j, "to", ""), J.Json.get_str(j, "kind", ""), J.Json.get_str(j, "want", ""), J.Json.get_nat(j, "at", 0n), J.Json.get_str(j, "text", "")}

def Mv.msg_put(+x: Msg, +m: Map<&2, Msg>) -> Map<&2, Msg>:
  Map.set(&2, Msg, m, Msg.key(x), x)

def Mv.msgs_in(+xs: List<&2, J.Json>, +m: Map<&2, Msg>) -> Map<&2, Msg>:
  match xs:
    case Nil{}:
      m
    case +x <> +t:
      Mv.msgs_in(t, Mv.msg_put(Mv.msg(x), m))

# One bubble. `lead` starts a group: the same sender within 5 minutes of the
# mail before it shares one group, so only the lead shows the label and time.
type Bub is Data:
  Bub{id: String, from: String, who: String, kind: String, want: String, at: Nat, text: String, lead: Bool}

type BS is Data:
  BS{acc: List<&2, Bub>, from: String, at: Nat}

def Mv.group() -> Nat:
  300n

def Mv.lead(+same: Bool, +gap: Nat) -> Bool:
  Bool.or(Bool.not(same), Nat.is_gt(gap, Mv.group()))

def Mv.bub.add(+m: Msg, +names: Map<&2, String>, +s: BS) -> BS:
  match m s:
    case Msg{+id, +from, _, +kind, +want, +at, +text} BS{+acc, +pf, +pat}:
      BS{Bub{id, from, Mv.name(names, from), kind, want, at, text, Mv.lead(String.eq(from, pf), Nat.sub(at, pat))} <> acc, from, at}

def Mv.bub.go(+ms: List<&2, Msg>, +names: Map<&2, String>, +s: BS) -> BS:
  match ms:
    case Nil{}:
      s
    case +m <> +t:
      Mv.bub.go(t, names, Mv.bub.add(m, names, s))

def Mv.bub.out(+s: BS) -> List<&2, Bub>:
  match s:
    case BS{+acc, _, _}:
      List.reverse(&2, Bub, acc)

# the conversation as bubbles, oldest first
def Mv.bubbles(+r: Read, +names: Map<&2, String>) -> List<&2, Bub>:
  Mv.bub.out(Mv.bub.go(Map.values(&2, Msg, Read.msgs(r)), names, BS{Nil{}, "\n", 0n}))

# The reply box's "to" picker: the conversation's parts and the live runs,
# once each; the person is never an option.
type Opt is Data:
  Opt{addr: String, label: String}

def Mv.has(+xs: List<&2, String>, +s: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +h <> +t:
      Bool.or(String.eq(h, s), Mv.has(t, s))

def Mv.skip(+x: String, +seen: List<&2, String>) -> Bool:
  Bool.or(String.is_empty(x), Bool.or(String.eq(x, "person"), Mv.has(seen, x)))

def Mv.addrs.add(+skip: Bool, +x: String, +seen: List<&2, String>) -> List<&2, String>:
  match skip:
    case True{}:
      seen
    case False{}:
      x <> seen

def Mv.addrs(+xs: List<&2, String>, +seen: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      seen
    case +x <> +t:
      Mv.addrs(t, Mv.addrs.add(Mv.skip(x, seen), x, seen))

def Mv.opts(+xs: List<&2, String>, +names: Map<&2, String>) -> List<&2, Opt>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> +t:
      Opt{x, Mv.name(names, x)} <> Mv.opts(t, names)

def Mv.targets(+parts: List<&2, String>, +live: List<&2, String>, +names: Map<&2, String>) -> List<&2, Opt>:
  Mv.opts(List.reverse(&2, String, Mv.addrs(live, Mv.addrs(parts, Nil{}))), names)

# Cache
# -----
# The last mail.list pages per (bot, filter) key, and the conversations
# opened. A page is 50 rows in a Map under `key#page`, so a row is found in
# O(log pages + 50) whatever the scroll.

type Cache is Data:
  Cache{pages: Map<&2, List<&2, Row>>, tot: Map<&2, Nat>, fresh: Map<&2, Bool>, keys: Map<&2, List<&2, String>>, reads: Map<&2, Read>}

def Cache.pages(+c: Cache) -> Map<&2, List<&2, Row>>:
  match c:
    case Cache{+v, _, _, _, _}:
      v

def Cache.tot(+c: Cache) -> Map<&2, Nat>:
  match c:
    case Cache{_, +v, _, _, _}:
      v

def Cache.fresh(+c: Cache) -> Map<&2, Bool>:
  match c:
    case Cache{_, _, +v, _, _}:
      v

def Cache.keys(+c: Cache) -> Map<&2, List<&2, String>>:
  match c:
    case Cache{_, _, _, +v, _}:
      v

def Cache.reads(+c: Cache) -> Map<&2, Read>:
  match c:
    case Cache{_, _, _, _, +v}:
      v

def Cache.with_pages(+c: Cache, +v: Map<&2, List<&2, Row>>) -> Cache:
  match c:
    case Cache{_, +a, +b, +d, +e}:
      Cache{v, a, b, d, e}

def Cache.with_tot(+c: Cache, +v: Map<&2, Nat>) -> Cache:
  match c:
    case Cache{+a, _, +b, +d, +e}:
      Cache{a, v, b, d, e}

def Cache.with_fresh(+c: Cache, +v: Map<&2, Bool>) -> Cache:
  match c:
    case Cache{+a, +b, _, +d, +e}:
      Cache{a, b, v, d, e}

def Cache.with_keys(+c: Cache, +v: Map<&2, List<&2, String>>) -> Cache:
  match c:
    case Cache{+a, +b, +d, _, +e}:
      Cache{a, b, d, v, e}

def Cache.with_reads(+c: Cache, +v: Map<&2, Read>) -> Cache:
  match c:
    case Cache{+a, +b, +d, +e, _}:
      Cache{a, b, d, e, v}

def Mv.cache() -> Cache:
  Cache{Map.new(&2, List<&2, Row>), Map.new(&2, Nat), Map.new(&2, Bool), Map.new(&2, List<&2, String>), Map.new(&2, Read)}

def Mv.pg.got(r: Map<&2, List<&2, Row>> & List<&2, Row>) -> List<&2, Row>:
  (_, v) = r
  v

def Mv.ks.got(r: Map<&2, List<&2, String>> & List<&2, String>) -> List<&2, String>:
  (_, v) = r
  v

def Mv.rd.got(r: Map<&2, Read> & Read) -> Read:
  (_, v) = r
  v

def Mv.slot(+key: String, +p: Nat) -> String:
  key ++ "#" ++ Nat.show(p)

def Mv.page_of(+c: Cache, +key: String, +p: Nat) -> List<&2, Row>:
  Mv.pg.got(Map.get(List<&2, Row>, Nil{}, Cache.pages(c), Mv.slot(key, p)))

def Mv.total(+c: Cache, +key: String) -> Nat:
  ML.Get.nat(Cache.tot(c), key)

# true once a page for the key arrived and no touch has come since
def Mv.fresh(+c: Cache, +key: String) -> Bool:
  ML.Get.has(Map.get(Bool, False{}, Cache.fresh(c), key))

def Mv.stale(+c: Cache, +key: String) -> Bool:
  Bool.not(Mv.fresh(c, key))

def Mv.nth(+xs: List<&2, Row>, +i: Nat) -> Row:
  match xs i:
    case Nil{} _:
      Mv.empty_row()
    case +h <> +r 0n:
      h
    case +x <> +t 1n+k:
      Mv.nth(t, k)

def Mv.row_at(+c: Cache, +key: String, +i: Nat) -> Row:
  Mv.nth(Mv.page_of(c, key, Nat.div(i, 50n)), Nat.mod(i, 50n))

# Rows skip..skip+n of the list under `key`, clamped to its total. A row not
# loaded yet is the empty row (id "").
def Mv.rows.go(+n: Nat, +c: Cache, +key: String, +i: Nat, +acc: List<&2, Row>) -> List<&2, Row>:
  match n:
    case 0n:
      acc
    case 1n+k:
      Mv.rows.go(k, c, key, Nat.add(i, 1n), Mv.row_at(c, key, i) <> acc)

def Mv.rows(+c: Cache, +key: String, +skip: Nat, +n: Nat) -> List<&2, Row>:
  List.reverse(&2, Row, Mv.rows.go(MN.Menu.min(n, Nat.sub(Mv.total(c, key), skip)), c, key, skip, Nil{}))

# every row of the window is loaded
def Mv.loaded.go(+n: Nat, +c: Cache, +key: String, +i: Nat, +ok: Bool) -> Bool:
  match n:
    case 0n:
      ok
    case 1n+k:
      Mv.loaded.go(k, c, key, Nat.add(i, 1n), Bool.and(ok, Bool.not(String.is_empty(Row.id(Mv.row_at(c, key, i))))))

def Mv.loaded(+c: Cache, +key: String, +skip: Nat, +n: Nat) -> Bool:
  Mv.loaded.go(MN.Menu.min(n, Nat.sub(Mv.total(c, key), skip)), c, key, skip, True{})

# the ids of rows, comma-ended (for tests and laws)
def Mv.ids(+rs: List<&2, Row>) -> String:
  match rs:
    case Nil{}:
      ""
    case +r <> +t:
      Row.id(r) ++ "," ++ Mv.ids(t)

def Mv.remember.k(+known: Bool, +ks: List<&2, String>, +key: String) -> List<&2, String>:
  match known:
    case True{}:
      ks
    case False{}:
      key <> ks

def Mv.remember(+ks: List<&2, String>, +key: String) -> List<&2, String>:
  Mv.remember.k(Mv.has(ks, key), ks, key)

# a page of up to 50 rows starting at row p*50; longer lists fill the next pages
def Mv.put.go(+fuel: Nat, +c: Cache, +key: String, +p: Nat, +rs: List<&2, Row>) -> Cache:
  match fuel:
    case 0n:
      c
    case 1n+k:
      Mv.put.go(k, Cache.with_pages(c, Map.set(&2, List<&2, Row>, Cache.pages(c), Mv.slot(key, p), List.take(&2, Row, rs, 50n))), key, Nat.add(p, 1n), List.drop(&2, Row, rs, 50n))

def Mv.put.fin(+c: Cache, +f: Filter, +total: Nat) -> Cache:
  Cache.with_fresh(Cache.with_keys(Cache.with_tot(c, Map.set(&2, Nat, Cache.tot(c), Mv.key(f), total)), Map.set(&2, List<&2, String>, Cache.keys(c), Filter.bot(f), Mv.remember(Mv.ks.got(Map.get(List<&2, String>, Nil{}, Cache.keys(c), Filter.bot(f))), Mv.key(f)))), Map.set(&2, Bool, Cache.fresh(c), Mv.key(f), True{}))

# Merge a page that arrived: its rows replace those at its slots, the rest of
# the list is untouched (so the ids of rows it did not change stay where they
# are), and the key is fresh. skip must be a multiple of 50.
def Mv.put_page(+c: Cache, +f: Filter, +skip: Nat, +total: Nat, +rs: List<&2, Row>) -> Cache:
  Mv.put.fin(Mv.put.go(Nat.div(Nat.add(List.length(&2, Row, rs), 49n), 50n), c, Mv.key(f), Nat.div(skip, 50n), rs), f, total)

# a mail.list / mail.find result for filter f at skip
def Mv.put_list(+c: Cache, +f: Filter, +skip: Nat, +j: J.Json, +me: String, +names: Map<&2, String>) -> Cache:
  Mv.put_page(c, f, skip, J.Json.get_nat(j, "total", 0n), Mv.parse(j, me, names))

# An "@mail" touch for these bots: only their keys, and All mail's, go stale.
# The rows stay and keep drawing until the new page replaces them.
def Mv.stale_keys(+ks: List<&2, String>, +fresh: Map<&2, Bool>) -> Map<&2, Bool>:
  match ks:
    case Nil{}:
      fresh
    case +k <> +t:
      Mv.stale_keys(t, Map.set(&2, Bool, fresh, k, False{}))

def Mv.touch_bot(+c: Cache, +bot: String) -> Cache:
  Cache.with_fresh(c, Mv.stale_keys(Mv.ks.got(Map.get(List<&2, String>, Nil{}, Cache.keys(c), bot)), Cache.fresh(c)))

def Mv.touch(+bots: List<&2, String>, +c: Cache) -> Cache:
  match bots:
    case Nil{}:
      Mv.touch_bot(c, "*")
    case +b <> +t:
      Mv.touch(t, Mv.touch_bot(c, b))

# Opened conversations
def Mv.read_of(+c: Cache, +id: String) -> Read:
  Mv.rd.got(Map.get(Read, Mv.empty_read(), Cache.reads(c), id))

def Mv.more(+older: Bool, +first: Bool, +was: Bool, +now: Bool) -> Bool:
  Bool.pick(Bool, Bool.or(older, first), now, was)

def Mv.put_read.old(+c: Cache, +id: String, +cv: J.Json, +j: J.Json, +older: Bool, +old: Read) -> Cache:
  Cache.with_reads(c, Map.set(&2, Read, Cache.reads(c), id,
    Read{id, J.Json.get_str(cv, "subject", ""), Mv.strs(J.Json.elems(J.Json.get_obj(cv, "parts"))), J.Json.get_str(cv, "project", ""), J.Json.get_str(cv, "status", "open"),
      Mv.more(older, String.is_empty(Read.id(old)), Read.more(old), J.Json.get_bool(j, "more", False{})),
      Mv.msgs_in(J.Json.elems(J.Json.get_obj(j, "mails")), Read.msgs(old))}))

def Mv.put_read.at(+c: Cache, +id: String, +cv: J.Json, +j: J.Json, +older: Bool) -> Cache:
  Mv.put_read.old(c, id, cv, j, older, Mv.read_of(c, id))

def Mv.put_read.k(+c: Cache, +cv: J.Json, +j: J.Json, +older: Bool) -> Cache:
  Mv.put_read.at(c, J.Json.get_str(cv, "id", ""), cv, j, older)

# A mail.read result: the header is replaced, the mails are merged by id.
# `older` is true for a page fetched with `before`.
def Mv.put_read(+c: Cache, +j: J.Json, +older: Bool) -> Cache:
  Mv.put_read.k(c, J.Json.get_obj(j, "conv"), j, older)

# Search
# ------
# A header search on every key press. Rows carry a lower-cased haystack, so a
# match is one `contains`. Typing narrows: a longer query that starts with the
# last one filters the last hits instead of scanning the cache again.

def Mv.hit(+r: Row, +ql: String) -> Bool:
  String.contains(Row.hay(r), ql)

def Mv.find_local.add(+keep: Bool, +r: Row, +acc: List<&2, Row>) -> List<&2, Row>:
  match keep:
    case True{}:
      r <> acc
    case False{}:
      acc

# rows whose header contains q, in order
def Mv.find_local.go(+rs: List<&2, Row>, +ql: String, +acc: List<&2, Row>) -> List<&2, Row>:
  match rs:
    case Nil{}:
      List.reverse(&2, Row, acc)
    case +r <> +t:
      Mv.find_local.go(t, ql, Mv.find_local.add(Mv.hit(r, ql), r, acc))

def Mv.find_local(+rs: List<&2, Row>, +q: String) -> List<&2, Row>:
  Mv.find_local.go(rs, String.to_lower(q), Nil{})

type Find is Data:
  Find{q: String, hits: List<&2, Row>, capped: Bool}

def Find.q(+f: Find) -> String:
  match f:
    case Find{+v, _, _}:
      v

def Find.hits(+f: Find) -> List<&2, Row>:
  match f:
    case Find{_, +v, _}:
      v

def Find.capped(+f: Find) -> Bool:
  match f:
    case Find{_, _, +v}:
      v

def Mv.found() -> Find:
  Find{"", Nil{}, True{}}

type G is Data:
  G{acc: List<&2, Row>, cnt: Nat, cap: Nat}

def G.full(+g: G) -> Bool:
  match g:
    case G{_, +cnt, +cap}:
      Nat.is_ge(cnt, cap)

def G.push(+r: Row, +g: G) -> G:
  match g:
    case G{+acc, +cnt, +cap}:
      G{r <> acc, Nat.add(cnt, 1n), cap}

def Mv.grep.add(+hit: Bool, +r: Row, +g: G) -> G:
  match hit:
    case True{}:
      G.push(r, g)
    case False{}:
      g

def Mv.grep.step(+full: Bool, +r: Row, +ql: String, +g: G) -> G:
  match full:
    case True{}:
      g
    case False{}:
      Mv.grep.add(Mv.hit(r, ql), r, g)

def Mv.grep.next(+r: Row, +ql: String, +g: G) -> G:
  Mv.grep.step(G.full(g), r, ql, g)

def Mv.grep(+rs: List<&2, Row>, +ql: String, +g: G) -> G:
  match rs:
    case Nil{}:
      g
    case +r <> +t:
      Mv.grep(t, ql, Mv.grep.next(r, ql, g))

def Mv.scan(+pages: Nat, +c: Cache, +key: String, +ql: String, +p: Nat, +g: G) -> G:
  match pages:
    case 0n:
      g
    case 1n+k:
      Mv.scan(k, c, key, ql, Nat.add(p, 1n), Mv.grep(Mv.page_of(c, key, p), ql, g))

def Mv.scan.out(+g: G) -> Find:
  match g:
    case G{+acc, +cnt, +cap}:
      Find{"", List.reverse(&2, Row, acc), Nat.is_ge(cnt, cap)}

def Mv.find.set(+f: Find, +q: String) -> Find:
  match f:
    case Find{_, +h, +cp}:
      Find{q, h, cp}

# the cached rows of `key` that match q, at most n
def Mv.find_cached(+c: Cache, +key: String, +q: String, +n: Nat) -> Find:
  Mv.find.set(Mv.scan.out(Mv.scan(Nat.div(Nat.add(Mv.total(c, key), 49n), 50n), c, key, String.to_lower(q), 0n, G{Nil{}, 0n, n})), q)

# The next search state after the query becomes q: narrow the last hits when
# q extends the last query and those hits were complete; otherwise scan.
def Mv.find_next.k(+narrow: Bool, +prev: Find, +c: Cache, +key: String, +q: String, +n: Nat) -> Find:
  match narrow:
    case True{}:
      Find{q, Mv.find_local(Find.hits(prev), q), False{}}
    case False{}:
      Mv.find_cached(c, key, q, n)

def Mv.find_next(+prev: Find, +c: Cache, +key: String, +q: String, +n: Nat) -> Find:
  Mv.find_next.k(Bool.and(Bool.not(String.is_empty(Find.q(prev))), Bool.and(Bool.not(Find.capped(prev)), String.starts_with(q, Find.q(prev)))), prev, c, key, q, n)

# Full-text hits arrive later. Merge them into the local ones, newest first
# (then by id), each conversation once; a conversation both lists hold keeps
# its local row.
type Z is Data:
  Z{a: List<&2, Row>, b: List<&2, Row>, acc: List<&2, Row>}

def Mv.before.id(+c: Cmp) -> Bool:
  match c:
    case GT{}:
      False{}
    case _:
      True{}

def Mv.before.k(+c: Cmp, +a: String, +b: String) -> Bool:
  match c:
    case GT{}:
      True{}
    case LT{}:
      False{}
    case EQ{}:
      Mv.before.id(String.order(a, b))

def Mv.before(+x: Row, +y: Row) -> Bool:
  Mv.before.k(Nat.cmp(Row.at(x), Row.at(y)), Row.id(x), Row.id(y))

def Mv.zip.take(+first: Bool, +x: Row, +xt: List<&2, Row>, +y: Row, +yt: List<&2, Row>, +acc: List<&2, Row>) -> Z:
  match first:
    case True{}:
      Z{xt, y <> yt, x <> acc}
    case False{}:
      Z{x <> xt, yt, y <> acc}

def Mv.zip.step(+z: Z) -> Z:
  match z:
    case Z{Nil{}, +y <> +yt, +acc}:
      Z{Nil{}, yt, y <> acc}
    case Z{+x <> +xt, Nil{}, +acc}:
      Z{xt, Nil{}, x <> acc}
    case Z{+x <> +xt, +y <> +yt, +acc}:
      Mv.zip.take(Mv.before(x, y), x, xt, y, yt, acc)
    case Z{Nil{}, Nil{}, +acc}:
      Z{Nil{}, Nil{}, acc}

def Mv.zip.go(+fuel: Nat, +z: Z) -> Z:
  match fuel:
    case 0n:
      z
    case 1n+f:
      Mv.zip.go(f, Mv.zip.step(z))

def Mv.zip.out(+z: Z) -> List<&2, Row>:
  match z:
    case Z{_, _, +acc}:
      List.reverse(&2, Row, acc)

# each id once, first kept, order kept
def Mv.once.add(+seen: Bool, +r: Row, +acc: List<&2, Row>) -> List<&2, Row>:
  match seen:
    case True{}:
      acc
    case False{}:
      r <> acc

def Mv.once(+rs: List<&2, Row>, +seen: Map<&2, Bool>, +acc: List<&2, Row>) -> List<&2, Row>:
  match rs:
    case Nil{}:
      List.reverse(&2, Row, acc)
    case +r <> +t:
      Mv.once(t, Map.set(&2, Bool, seen, Row.id(r), True{}), Mv.once.add(ML.Get.has(Map.get(Bool, False{}, seen, Row.id(r))), r, acc))

def Mv.seen(+rs: List<&2, Row>, +m: Map<&2, Bool>) -> Map<&2, Bool>:
  match rs:
    case Nil{}:
      m
    case +r <> +t:
      Mv.seen(t, Map.set(&2, Bool, m, Row.id(r), True{}))

def Mv.merge.k(+l: List<&2, Row>, +server: List<&2, Row>) -> List<&2, Row>:
  Mv.zip.out(Mv.zip.go(Nat.add(List.length(&2, Row, l), List.length(&2, Row, server)), Z{l, Mv.once(server, Mv.seen(l, Map.new(&2, Bool)), Nil{}), Nil{}}))

# a conversation both lists hold keeps its local row
def Mv.merge_hits(+local: List<&2, Row>, +server: List<&2, Row>) -> List<&2, Row>:
  Mv.merge.k(Mv.once(local, Map.new(&2, Bool), Nil{}), server)

def Mv.id_list(+rs: List<&2, Row>) -> List<&2, String>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> +t:
      Row.id(r) <> Mv.id_list(t)

# no id twice (the check, quadratic: for laws and tests)
def Mv.unique(+rs: List<&2, Row>) -> Bool:
  match rs:
    case Nil{}:
      True{}
    case +r <> +t:
      Bool.and(Bool.not(Mv.has(Mv.id_list(t), Row.id(r))), Mv.unique(t))

# Badges
# ------
# Unread per bot and the sum, read off the fold's counters.
type Badge is Data:
  Badge{bot: String, n: Nat, text: String}

def Badge.bot(+b: Badge) -> String:
  match b:
    case Badge{+v, _, _}:
      v

def Badge.n(+b: Badge) -> Nat:
  match b:
    case Badge{_, +v, _}:
      v

def Badge.text(+b: Badge) -> String:
  match b:
    case Badge{_, _, +v}:
      v

def Mv.badge_text(+n: Nat) -> String:
  Bool.pick(String, Nat.is_eq(n, 0n), "", Bool.pick(String, Nat.is_gt(n, 99n), "99+", Nat.show(n)))

def Mv.badge.n(+bot: String, +n: Nat) -> Badge:
  Badge{bot, n, Mv.badge_text(n)}

def Mv.badge(+b: ML.Box, +bot: String) -> Badge:
  Mv.badge.n(bot, ML.Box.unread(b, bot))

def Mv.badge_all(+b: ML.Box) -> Badge:
  Mv.badge.n("*", ML.Box.unread_all(b))

def Mv.badges(+b: ML.Box, +bots: List<&2, String>) -> List<&2, Badge>:
  match bots:
    case Nil{}:
      Nil{}
    case +x <> +t:
      Mv.badge(b, x) <> Mv.badges(b, t)
