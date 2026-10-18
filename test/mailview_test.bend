import Base
import ./check.bend as C
import ../src/core/mailview.bend as MV
import ../src/core/mail.bend as ML
import ../src/core/model.bend as M
import ../src/core/json.bend as J

# The mailroom view model: the visible window, the page cache, filters, local
# search and its merge with server hits, conversation bubbles, the reply
# picker, badges. Then a 5k-row cache timed (the frame budget is 8 ms).

def parsed(+text: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})

def names() -> Map<&2, String>:
  Map.set(&2, String, Map.set(&2, String, Map.new(&2, String), "bot:b", "Beta"), "thread:t1", "Fix PSU")

def span(+s: MV.Span) -> String:
  match s:
    case MV.Span{f, n}: Nat.show(f) ++ "+" ++ Nat.show(n)

def vis(+total: Nat, +scroll: Nat, +view: Nat) -> String:
  span(MV.Mv.visible(total, scroll, view))

def whos(+rs: List<&2, MV.Row>) -> List<&2, String>:
  match rs:
    case Nil{}: Nil{}
    case r <> t: MV.Row.who(r) <> whos(t)

def lo(+a: Nat, +b: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_le(a, b), a, b)

def covers.k(+s: MV.Span, +total: Nat, +scroll: Nat, +view: Nat) -> Bool:
  match s:
    case MV.Span{f, n}:
      Bool.and(Nat.is_ge(scroll, MV.Mv.y(f)), Nat.is_ge(MV.Mv.y(Nat.add(f, n)), lo(Nat.add(scroll, view), MV.Mv.height(total))))

# the visible window covers every px of scroll..scroll+view (when it fits)
def covers(+total: Nat, +scroll: Nat, +view: Nat) -> Bool:
  covers.k(MV.Mv.visible(total, scroll, view), total, scroll, view)

def list_text() -> String:
  "{\"convs\":[{\"id\":\"c2\",\"subject\":\"Second\",\"kind\":\"pair\",\"parts\":[\"bot:a\",\"person\"],\"project\":\"q\",\"tags\":[],\"last\":9,\"n\":1,\"status\":\"unread\",\"unread\":true,\"head\":\"hello there\",\"from\":\"person\"},{\"id\":\"c1\",\"subject\":\"First\",\"kind\":\"pair\",\"parts\":[\"bot:a\",\"thread:t1\",\"bot:b\",\"thread:t9\"],\"project\":\"p\",\"tags\":[],\"last\":5,\"n\":2,\"status\":\"open\",\"unread\":false,\"head\":\"PSU is done\",\"from\":\"bot:b\"}],\"total\":2}"

def read_text(+more: String, +mails: String) -> String:
  "{\"conv\":{\"id\":\"c1\",\"subject\":\"First\",\"kind\":\"pair\",\"parts\":[\"bot:a\",\"thread:t1\",\"bot:b\",\"person\",\"thread:t1\"],\"project\":\"p\",\"tags\":[],\"last\":5,\"n\":4,\"status\":\"open\",\"unread\":false,\"head\":\"x\",\"from\":\"bot:b\"},\"mails\":[" ++ mails ++ "],\"more\":" ++ more ++ "}"

def mail_json(+id: String, +from: String, +at: Nat, +text: String) -> String:
  "{\"id\":\"" ++ id ++ "\",\"from\":\"" ++ from ++ "\",\"to\":\"bot:a\",\"kind\":\"thread\",\"want\":\"act\",\"at\":" ++ Nat.show(at) ++ ",\"text\":\"" ++ text ++ "\"}"

def bubble(+b: MV.Bub) -> String:
  match b:
    case MV.Bub{id, _, who, _, _, _, _, lead}: id ++ ":" ++ who ++ ":" ++ Bool.show(lead)

def bubbles(+bs: List<&2, MV.Bub>) -> String:
  match bs:
    case Nil{}: ""
    case b <> t: bubble(b) ++ " " ++ bubbles(t)

def opts(+os: List<&2, MV.Opt>) -> String:
  match os:
    case Nil{}: ""
    case MV.Opt{a, l} <> t: a ++ "=" ++ l ++ " " ++ opts(t)

def cp(+cs: List<&2, MV.Chip>) -> String:
  match cs:
    case Nil{}: ""
    case MV.Chip{id, _, on} <> t: id ++ Bool.pick(String, on, "*", "") ++ " " ++ cp(t)

def row(+id: String, +at: Nat) -> MV.Row:
  MV.Mv.stub(id, at)

def mail(+id: String, +conv: String, +from: String, +to: String, +at: Nat) -> M.Change:
  M.MailPosted{id, conv, from, to, "thread", "", "act", "Latest HEAD", 10n, at}

# a 5k-row cache: rows c0 (newest) .. c4999, 50 to a page
def mk(+i: Nat) -> MV.Row:
  MV.Mv.make("c" ++ Nat.show(i), "Subject " ++ Nat.show(i), "Beta", "head " ++ Nat.show(i), Nat.sub(1000000n, i), False{}, "open", "p")

def mk_page(+n: Nat, +i: Nat, +acc: List<&2, MV.Row>) -> List<&2, MV.Row>:
  match n:
    case 0n: List.reverse(&2, MV.Row, acc)
    case 1n+k: mk_page(k, Nat.add(i, 1n), mk(i) <> acc)

def fill(+pages: Nat, +p: Nat, +c: MV.Cache) -> MV.Cache:
  match pages:
    case 0n: c
    case 1n+k: fill(k, Nat.add(p, 1n), MV.Mv.put_page(c, MV.Mv.filter(), Nat.mul(p, 50n), 5000n, mk_page(50n, Nat.mul(p, 50n), Nil{})))

def scroll_one(+s: MV.Span, +c: MV.Cache, +key: String) -> Nat:
  match s:
    case MV.Span{f, n}: List.length(&2, MV.Row, MV.Mv.rows(c, key, f, n))

# 40 scroll positions, `reps` times over: visible + rows each call
def scroll_run(+n: Nat, +c: MV.Cache, +key: String, +acc: Nat) -> Nat:
  match n:
    case 0n: acc
    case 1n+k: scroll_run(k, c, key, Nat.add(acc, scroll_one(MV.Mv.visible(5000n, Nat.mul(Nat.mod(k, 40n), 6000n), 600n), c, key)))

# type a query one character at a time, keeping the search state
def typed(+qs: List<&2, String>, +f: MV.Find, +c: MV.Cache, +key: String) -> MV.Find:
  match qs:
    case Nil{}: f
    case q <> t: typed(t, MV.Mv.find_next(f, c, key, q, 50n), c, key)

def typed_run(+n: Nat, +qs: List<&2, String>, +c: MV.Cache, +key: String, +acc: Nat) -> Nat:
  match n:
    case 0n: acc
    case 1n+k: typed_run(k, qs, c, key, Nat.add(acc, List.length(&2, MV.Row, MV.Find.hits(typed(qs, MV.Mv.found(), c, key)))))

def main() -> IO(Unit):
  do IO<Unit>:
    # the visible window
    C.Check.str("visible top", vis(5000n, 0n, 520n), "0+12")
    C.Check.str("visible middle has overscan", vis(5000n, 5200n, 520n), "98+14")
    C.Check.str("visible bottom", vis(5000n, 259000n, 520n), "4978+15")
    C.Check.str("visible past the end clamps", vis(5000n, 9999999n, 520n), "4985+15")
    C.Check.str("visible short list", vis(3n, 0n, 520n), "0+3")
    C.Check.str("visible empty list", vis(0n, 0n, 520n), "0+0")
    C.Check.str("visible partial rows", vis(5000n, 53n, 100n), "0+5")
    C.Check.str("covers top", Bool.show(covers(5000n, 0n, 520n)), "True")
    C.Check.str("covers middle", Bool.show(covers(5000n, 5200n, 520n)), "True")
    C.Check.str("covers bottom", Bool.show(covers(5000n, 259000n, 520n)), "True")
    C.Check.str("height", Nat.show(MV.Mv.height(5000n)), "260000")
    # parsing a mail.list page
    +rs : List<&2, MV.Row> = MV.Mv.parse(parsed(list_text()), "bot:a", names())
    C.Check.str("parse ids", MV.Mv.ids(rs), "c2,c1,")
    C.Check.str("who drops me, shows person as you", String.join(whos(rs), "|"), "you|Fix PSU, Beta +1")
    C.Check.str("row fields", MV.Row.subject(MV.Mv.nth(rs, 1n)) ++ "/" ++ MV.Row.head(MV.Mv.nth(rs, 1n)) ++ "/" ++ MV.Row.project(MV.Mv.nth(rs, 1n)) ++ "/" ++ MV.Row.status(MV.Mv.nth(rs, 0n)) ++ "/" ++ Bool.show(MV.Row.unread(MV.Mv.nth(rs, 0n))) ++ "/" ++ Bool.show(MV.Row.live(MV.Mv.nth(rs, 0n))), "First/PSU is done/p/unread/True/False")
    C.Check.str("unknown address is its id, shortened", MV.Mv.name(names(), "thread:abcdefghijkl"), "abcdefgh…")
    C.Check.str("name of a bot", MV.Mv.name(names(), "bot:b"), "Beta")
    C.Check.str("age now", MV.Mv.ago(100n, 100n), "now")
    C.Check.str("age minutes", MV.Mv.ago(1000n, 400n), "10m")
    C.Check.str("age hours", MV.Mv.ago(20000n, 5000n), "4h")
    C.Check.str("age days", MV.Mv.ago(1000000n, 100n), "11d")
    # the cache
    +f : MV.Filter = MV.Mv.filter()
    +k : String = MV.Mv.key(f)
    +c1 : MV.Cache = MV.Mv.put_list(MV.Mv.cache(), f, 0n, parsed(list_text()), "bot:a", names())
    C.Check.str("a key not fetched is stale", Bool.show(MV.Mv.stale(MV.Mv.cache(), k)), "True")
    C.Check.str("a fetched key is fresh", Bool.show(MV.Mv.stale(c1, k)), "False")
    C.Check.str("rows by window", MV.Mv.ids(MV.Mv.rows(c1, k, 0n, 10n)), "c2,c1,")
    C.Check.str("rows skip", MV.Mv.ids(MV.Mv.rows(c1, k, 1n, 10n)), "c1,")
    C.Check.str("rows past the end", MV.Mv.ids(MV.Mv.rows(c1, k, 5n, 10n)), "")
    C.Check.str("window loaded", Bool.show(MV.Mv.loaded(c1, k, 0n, 10n)), "True")
    C.Check.str("another key is empty", MV.Mv.ids(MV.Mv.rows(c1, MV.Mv.key(MV.Mv.for_bot("b")), 0n, 10n)), "")
    +c2 : MV.Cache = MV.Mv.put_page(c1, f, 0n, 3n, [row("c3", 12n), row("c2", 9n), row("c1", 5n)])
    C.Check.str("a newer page replaces the slots", MV.Mv.ids(MV.Mv.rows(c2, k, 0n, 10n)), "c3,c2,c1,")
    +c3 : MV.Cache = MV.Mv.put_page(c2, f, 0n, 3n, [row("c3", 12n), row("c2", 9n), row("c1", 5n)])
    C.Check.str("the same page again changes nothing", MV.Mv.ids(MV.Mv.rows(c3, k, 0n, 10n)), "c3,c2,c1,")
    +fb : MV.Filter = MV.Mv.for_bot("b")
    +cb : MV.Cache = MV.Mv.put_page(c3, fb, 0n, 1n, [row("x1", 1n)])
    +ct : MV.Cache = MV.Mv.touch(["a"], cb)
    C.Check.str("a touch for a bot leaves other bots fresh", Bool.show(MV.Mv.stale(ct, MV.Mv.key(fb))), "False")
    +cp2 : MV.Cache = MV.Mv.put_page(c3, MV.Mv.for_bot("a"), 0n, 1n, [row("y1", 1n)])
    +ct2 : MV.Cache = MV.Mv.touch(["a"], cp2)
    C.Check.str("a touch for a bot stales its key", Bool.show(MV.Mv.stale(ct2, MV.Mv.key(MV.Mv.for_bot("a")))), "True")
    C.Check.str("and All mail", Bool.show(MV.Mv.stale(ct2, k)), "True")
    C.Check.str("the rows stay while stale", MV.Mv.ids(MV.Mv.rows(ct2, k, 0n, 10n)), "c3,c2,c1,")
    C.Check.str("a touch for another bot leaves a bot's key", Bool.show(MV.Mv.stale(MV.Mv.touch(["b"], cp2), MV.Mv.key(MV.Mv.for_bot("a")))), "False")
    # a page beyond one page fills the next pages
    +c4 : MV.Cache = MV.Mv.put_page(MV.Mv.cache(), f, 0n, 120n, mk_page(120n, 0n, Nil{}))
    C.Check.str("page 3 row", MV.Row.id(MV.Mv.row_at(c4, k, 119n)), "c119")
    C.Check.str("page 2 starts at 50", MV.Row.id(MV.Mv.row_at(c4, k, 50n)), "c50")
    # filters
    C.Check.str("default filter key", MV.Mv.key(MV.Mv.filter()), "1:*0:")
    C.Check.str("keys differ by bot", Bool.show(String.eq(MV.Mv.key(MV.Mv.for_bot("a")), MV.Mv.key(MV.Mv.for_bot("b")))), "False")
    C.Check.str("keys do not run fields together", Bool.show(String.eq(MV.Mv.key(MV.Filter{"a|b", "", "c", ""}), MV.Mv.key(MV.Filter{"a", "|b", "c", ""}))), "False")
    C.Check.str("status and project do not swap", Bool.show(String.eq(MV.Mv.key(MV.Filter{"a", "x", "y", ""}), MV.Mv.key(MV.Filter{"a", "y", "x", ""}))), "False")
    C.Check.str("the query is not in the key", Bool.show(String.eq(MV.Mv.key(MV.Mv.with_q(f, "zz")), k)), "True")
    C.Check.str("chips none on", cp(MV.Mv.chips(f)), "unread open snoozed done ")
    C.Check.str("a status chip", cp(MV.Mv.chips(MV.Mv.toggle(f, "open"))), "unread open* snoozed done ")
    C.Check.str("pressed again it clears", cp(MV.Mv.chips(MV.Mv.toggle(MV.Mv.toggle(f, "open"), "open"))), "unread open snoozed done ")
    C.Check.str("a project chip", cp(MV.Mv.chips(MV.Mv.with_project(f, "p"))), "unread open snoozed done project* ")
    # search
    C.Check.str("local match on subject", MV.Mv.ids(MV.Mv.find_local(rs, "SECOND")), "c2,")
    C.Check.str("local match on who", MV.Mv.ids(MV.Mv.find_local(rs, "fix psu")), "c1,")
    C.Check.str("local match on head and project", MV.Mv.ids(MV.Mv.find_local(rs, "is done")) ++ MV.Mv.ids(MV.Mv.find_local(rs, "q")), "c1,c2,")
    C.Check.str("empty query is everything", MV.Mv.ids(MV.Mv.find_local(rs, "")), "c2,c1,")
    C.Check.str("cached search", MV.Mv.ids(MV.Find.hits(MV.Mv.find_cached(c1, k, "first", 10n))), "c1,")
    +local : List<&2, MV.Row> = [row("a", 9n), row("b", 7n), row("c", 5n)]
    +server : List<&2, MV.Row> = [row("b", 8n), row("d", 6n), row("a", 9n), row("e", 1n)]
    C.Check.str("merge: newest first", MV.Mv.ids(MV.Mv.merge_hits(local, server)), "a,b,d,c,e,")
    C.Check.str("merge: no duplicates", Bool.show(MV.Mv.unique(MV.Mv.merge_hits(local, server))), "True")
    C.Check.str("merge: local row wins", Nat.show(MV.Row.at(MV.Mv.nth(MV.Mv.merge_hits(local, server), 1n))), "7")
    C.Check.str("merge: stable under repeat", MV.Mv.ids(MV.Mv.merge_hits(MV.Mv.merge_hits(local, server), server)), "a,b,d,c,e,")
    C.Check.str("merge: duplicates inside one list", MV.Mv.ids(MV.Mv.merge_hits([row("a", 9n), row("a", 9n)], Nil{})), "a,")
    C.Check.str("merge: ties by id", MV.Mv.ids(MV.Mv.merge_hits([row("b", 4n)], [row("a", 4n)])), "a,b,")
    C.Check.str("merge: empty server", MV.Mv.ids(MV.Mv.merge_hits(local, Nil{})), "a,b,c,")
    # a conversation
    +p1 : String = read_text("true", mail_json("m1", "bot:b", 1000n, "hi") ++ "," ++ mail_json("m2", "bot:b", 1100n, "again") ++ "," ++ mail_json("m3", "person", 1150n, "ok") ++ "," ++ mail_json("m4", "bot:b", 1700n, "later"))
    +r1 : MV.Cache = MV.Mv.put_read(MV.Mv.cache(), parsed(p1), False{})
    C.Check.str("read header", MV.Read.subject(MV.Mv.read_of(r1, "c1")) ++ "/" ++ MV.Read.project(MV.Mv.read_of(r1, "c1")), "First/p")
    C.Check.str("bubbles group by sender within 5 minutes", bubbles(MV.Mv.bubbles(MV.Mv.read_of(r1, "c1"), names())), "m1:Beta:True m2:Beta:False m3:you:True m4:Beta:True ")
    C.Check.str("more is kept", Bool.show(MV.Read.more(MV.Mv.read_of(r1, "c1"))), "True")
    +p0 : String = read_text("false", mail_json("m0", "bot:b", 900n, "first") ++ "," ++ mail_json("m1", "bot:b", 1000n, "hi"))
    +r2 : MV.Cache = MV.Mv.put_read(r1, parsed(p0), True{})
    C.Check.str("an older page merges once, oldest first", bubbles(MV.Mv.bubbles(MV.Mv.read_of(r2, "c1"), names())), "m0:Beta:True m1:Beta:False m2:Beta:False m3:you:True m4:Beta:True ")
    C.Check.str("older page clears more", Bool.show(MV.Read.more(MV.Mv.read_of(r2, "c1"))), "False")
    +r3 : MV.Cache = MV.Mv.put_read(r1, parsed(p1), False{})
    C.Check.str("the same page twice is one set of mails", bubbles(MV.Mv.bubbles(MV.Mv.read_of(r3, "c1"), names())), "m1:Beta:True m2:Beta:False m3:you:True m4:Beta:True ")
    C.Check.str("unopened conversation is empty", bubbles(MV.Mv.bubbles(MV.Mv.read_of(r1, "zz"), names())), "")
    C.Check.str("to picker: parts and live runs, no person, no repeats", opts(MV.Mv.targets(MV.Read.parts(MV.Mv.read_of(r1, "c1")), ["thread:w1", "bot:a"], names())), "bot:a=a thread:t1=Fix PSU bot:b=Beta thread:w1=w1 ")
    C.Check.str("to picker is empty for the person alone", opts(MV.Mv.targets(["person"], Nil{}, names())), "")
    # badges
    +base : M.State = M.State.replay([
      M.ConvSet{"c1", "First", "pair", "bot:a\nbot:b", "p", "red", 1n},
      M.ConvSet{"c2", "Second", "pair", "bot:a", "q", "green", 1n},
      mail("m1", "c1", "bot:a", "bot:b", 5n),
      mail("m2", "c2", "person", "bot:a", 9n),
      mail("m3", "c2", "person", "bot:a", 7n)], M.State.empty())
    +b : ML.Box = M.State.mail(base)
    C.Check.str("badge per bot", MV.Badge.text(MV.Mv.badge(b, "a")), Nat.show(ML.Box.unread(b, "a")))
    C.Check.str("badge sum", Nat.show(MV.Badge.n(MV.Mv.badge_all(b))), Nat.show(ML.Box.unread_all(b)))
    C.Check.str("badge of nothing is blank", MV.Badge.text(MV.Mv.badge(b, "zz")), "")
    C.Check.str("badge caps", MV.Mv.badge_text(150n), "99+")
    C.Check.str("badges list", Nat.show(List.length(&2, MV.Badge, MV.Mv.badges(b, ["a", "b", "c"]))), "3")
    # 5k rows
    +t0 : Nat <- IO.now()
    +big : MV.Cache = fill(100n, 0n, MV.Mv.cache())
    +t1 : Nat <- IO.now()
    IO.print("fill 5000 rows in 100 pages: " ++ Nat.show(Nat.sub(t1, t0)) ++ " ms")
    +kb : String = MV.Mv.key(MV.Mv.filter())
    C.Check.str("big total", Nat.show(MV.Mv.total(big, kb)), "5000")
    C.Check.str("big last row", MV.Row.id(MV.Mv.row_at(big, kb, 4999n)), "c4999")
    C.Check.str("big window", MV.Mv.ids(MV.Mv.rows(big, kb, 2498n, 3n)), "c2498,c2499,c2500,")
    +s0 : Nat <- IO.now()
    +ssum : Nat = scroll_run(1000n, big, kb, 0n)
    +s1 : Nat <- IO.now()
    C.Check.str("scroll rows built", Bool.show(Nat.is_gt(ssum, 12000n)), "True")
    IO.print("scroll: visible + rows, 1000 calls (40 positions x 25): " ++ Nat.show(Nat.sub(s1, s0)) ++ " ms, so that many us per call")
    C.Check.str("scroll under 1 ms a call", Bool.show(Nat.is_lt(Nat.sub(s1, s0), 1000n)), "True")
    +q0 : Nat <- IO.now()
    +hits : MV.Find = typed(["h", "he", "hea", "head", "head ", "head 4"], MV.Mv.found(), big, kb)
    +q1 : Nat <- IO.now()
    C.Check.str("typed search finds head 4", MV.Mv.ids(List.take(&2, MV.Row, MV.Find.hits(hits), 3n)), "c4,c40,c41,")
    IO.print("typed 6 chars, 1 run, cold start: " ++ Nat.show(Nat.sub(q1, q0)) ++ " ms")
    +q2 : Nat <- IO.now()
    +nsum : Nat = typed_run(20n, ["h", "he", "hea", "head", "head ", "head 4"], big, kb, 0n)
    +q3 : Nat <- IO.now()
    C.Check.str("typed run hits", Nat.show(nsum), "1000")
    IO.print("typed 6 chars x 20 runs (120 keys): " ++ Nat.show(Nat.sub(q3, q2)) ++ " ms")
    +m0 : Nat <- IO.now()
    +miss : MV.Find = typed(["q", "qx", "qxz", "qxzj", "qxzjk", "qxzjkw"], MV.Mv.found(), big, kb)
    +m1 : Nat <- IO.now()
    C.Check.str("a miss finds nothing", Nat.show(List.length(&2, MV.Row, MV.Find.hits(miss))), "0")
    IO.print("typed 6 chars with no match (first key scans all 5000): " ++ Nat.show(Nat.sub(m1, m0)) ++ " ms")
