import Base
import ./check.bend as C
import ../src/core/thin.bend as T
import ../src/core/model.bend as M
import ../src/core/hits.bend as H

# The top bar's search: every thread but the deleted ones, each result's
# snippet with the query's words lit, the open thread's results first and
# the rest grouped by project and thread.

def ids(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}: ""
    case x <> t: x ++ "," ++ ids(t)

def ids2(p: T.SearchPage) -> String:
  match p:
    case T.SearchPage{is0, _, _, _}: Nat.show(List.length(&2, String, is0))

def search.ids(p: T.SearchPage) -> String:
  match p:
    case T.SearchPage{is0, _, _, _}: ids(is0)

# pieces shown as plain|[match]|plain...
def show.go(ps: List<&2, String>, lit: Bool) -> String:
  match ps lit:
    case Nil{} _: ""
    case (p <> t) True{}: "[" ++ p ++ "]" ++ show.go(t, False{})
    case (p <> t) False{}: p ++ show.go(t, True{})

def show(ps: List<&2, String>) -> String:
  show.go(ps, False{})

def snip(+q: String, +text: String) -> String:
  show(H.Snip.of(T.Words.of(q), text))

# "narwhal0\nnarwhal1\n...": a reply of n numbered lines
def lines(n: Nat, +i: Nat, acc: String) -> String:
  match n:
    case 0n: acc
    case 1n+k: lines(k, Nat.add(i, 1n), acc ++ Bool.pick(String, Nat.is_eq(i, 0n), "", "\n") ++ "narwhal" ++ Nat.show(i))

def words(n: Nat, +w: String, acc: String) -> String:
  match n:
    case 0n: acc
    case 1n+k: words(k, w, acc ++ w ++ " ")

def hit(+id: String, +th: String, +p: String) -> H.HitAt:
  H.HitAt{id, th, p}

def hit.ids(hs: List<&2, H.HitAt>) -> String:
  match hs:
    case Nil{}: ""
    case h <> t: H.HitAt.id(h) ++ "," ++ hit.ids(t)

def busy(n: Nat, i: T.Index) -> T.Index:
  match n:
    case 0n: i
    case 1n+k: busy(k, T.Index.post(i, M.Msg{"x", "c", M.Assistant{}, "a needle again", 9n}))

def busy.in(n: Nat, +th: String, i: T.Index) -> T.Index:
  match n:
    case 0n: i
    case 1n+k: busy.in(k, th, T.Index.post(i, M.Msg{"y", th, M.Assistant{}, "the needle there", 9n}))

def log() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "board", "/tmp/board", 100n},
   M.ThreadCreated{"a", "p1", "Power", M.Local{}, "", "", "claude", "", 200n},
   M.ThreadCreated{"b", "p1", "Pins", M.Local{}, "", "", "claude", "", 300n},
   M.ThreadDeleted{"b", 400n}]

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("a reply of lines shows the match's line and one each side", snip("narwhal17", lines(60n, 0n, "")), "narwhal16\n[narwhal17]\nnarwhal18")
    C.Check.str("the first line has none above", snip("narwhal0", lines(60n, 0n, "")), "[narwhal0]\nnarwhal1")
    C.Check.str("the last line has none below", snip("narwhal59", lines(60n, 0n, "")), "narwhal58\n[narwhal59]")
    C.Check.str("ASCII case aside, shown as written", snip("narwhal", "The Narwhal swims"), "The [Narwhal] swims")
    C.Check.str("every word of the query is lit", snip("alpha beta", "alpha, then beta and alpha again"), "[alpha], then [beta] and [alpha] again")
    C.Check.str("a whole word only", snip("cat", "concatenate the cat"), "concatenate the [cat]")
    C.Check.str("a word inside another is not a match: the text's start", snip("cat", "concatenate this"), "concatenate this")
    C.Check.str("a long line is cut around the match", snip("needle", words(40n, "hay", "") ++ "needle " ++ words(60n, "straw", "")),
      "…hay hay hay hay hay hay hay hay hay hay hay hay hay hay hay hay hay hay hay hay [needle] straw straw straw straw straw straw straw straw straw straw straw straw straw straw straw straw straw straw straw straw straw straw straw straw straw straw str…")
    C.Check.str("a blank line above is not shown", snip("x", "a\n\nx y"), "[x] y")
    C.Check.str("Unicode words", snip("日本語", "これは 日本語 です"), "これは [日本語] です")
    C.Check.str("an empty text", snip("x", ""), "")
    # the order shown: the open thread's first, then project by project
    # and thread by thread, each where its newest result was
    +hs : List<&2, H.HitAt> = [hit("1", "t2", "p1"), hit("2", "t1", "p2"), hit("3", "t3", "p1"), hit("4", "t2", "p1"), hit("5", "here", "p2"), hit("6", "t1", "p2"), hit("7", "here", "p2")]
    C.Check.str("the open thread first, then by project and thread", hit.ids(H.Hits.order(hs, "here")), "5,7,1,4,3,2,6,")
    C.Check.str("with no results in the open thread", hit.ids(H.Hits.order(hs, "none")), "1,4,3,2,6,5,7,")
    C.Check.str("nothing to order", hit.ids(H.Hits.order(Nil{}, "here")), "")
    # every thread's rows, the deleted thread's left out
    +i : T.Index = T.Index.post(T.Index.post(T.Index.post(T.Index.empty(),
      M.Msg{"m1", "a", M.User{}, "the narwhal here", 1n}), M.Msg{"m2", "b", M.Assistant{}, "a narwhal there", 2n}), M.Msg{"m3", "c", M.Assistant{}, "narwhal elsewhere", 3n})
    C.Check.str("a search over every thread", search.ids(T.Index.search.all(i, "narwhal", T.Search.gone(M.State.empty()), 0n)), "3,2,1,")
    +st : M.State = M.State.replay(log(), M.State.empty())
    C.Check.str("a deleted thread is left out", search.ids(T.Index.search.all(i, "narwhal", T.Search.gone(st), 0n)), "3,1,")
    C.Check.str("a search within a thread is as before", search.ids(T.Index.search(i, "narwhal", "a", 0n)), "1,")
    # from thread a: its results, then the others' (b deleted)
    C.Check.str("the open thread's results first, then the others'",
      search.ids(T.Search.join(T.Index.search.in(i, "narwhal", "a"), T.Index.search.all(i, "narwhal", T.Search.gone.but(T.Search.gone(st), "a"), 0n))), "1,3,")
    +b : T.Index = busy(200n, T.Index.post(T.Index.empty(), M.Msg{"m1", "a", M.User{}, "the needle here", 1n}))
    C.Check.str("a busy hub's newest uses of a word are elsewhere", search.ids(T.Index.search.all(b, "needle", T.Search.gone.but(Map.new(&2, Bool), "a"), 0n)),
      "201,200,199,198,197,196,195,194,193,192,191,190,189,188,187,186,185,184,183,182,181,180,179,178,177,")
    C.Check.str("the open thread's own result is still found", search.ids(T.Index.search.in(b, "needle", "a")), "1,")
    C.Check.str("and comes first, a page still 25", search.ids(T.Search.join(T.Index.search.in(b, "needle", "a"), T.Index.search.rest(b, "needle", T.Search.gone.but(Map.new(&2, Bool), "a"), T.Search.left(T.Index.search.in(b, "needle", "a")), 0n))),
      "1,201,200,199,198,197,196,195,194,193,192,191,190,189,188,187,186,185,184,183,182,181,180,179,178,")
    # a subagent's step (logged under "@sub:<thread>", model.bend's
    # Bg.log) opens its thread, at the call that started the subagent;
    # the hub refused a view of "@sub:a" (the page stuck on "Conversation
    # unavailable", and again after every reload)
    C.Check.str("a subagent's step is its thread's", H.Hits.owner("@sub:a"), "a")
    C.Check.str("a thread is its own", H.Hits.owner("a"), "a")
    C.Check.str("a subagent's step shows its call", H.Hits.jump(M.Act{"call1/m:0", "@sub:a", "call1", "say", "the narwhal", 5n}), "call1")
    C.Check.str("an entry of a thread shows itself", H.Hits.jump(M.Act{"call1", "a", "tool", "Agent", "the narwhal", 5n}), "call1")
    C.Check.str("a message shows itself", H.Hits.jump(M.Msg{"m1", "a", M.User{}, "the narwhal", 1n}), "m1")
    +subs : T.Index = T.Index.post(T.Index.post(i, M.Act{"c1/m:0", "@sub:a", "c1", "say", "a narwhal in a", 4n}), M.Act{"c2/m:0", "@sub:b", "c2", "say", "a narwhal in b", 5n})
    C.Check.str("a deleted thread's subagents are left out too", search.ids(T.Index.search.all(subs, "narwhal", T.Search.gone(st), 0n)), "4,3,1,")
    +many : T.Index = busy.in(30n, "d", busy(30n, T.Index.empty()))
    C.Check.str("at most ten of the open thread's", ids2(T.Index.search.in(many, "needle", "c")), "10")
    C.Check.str("and the others' fill the page", search.ids(T.Search.join(T.Index.search.in(many, "needle", "c"), T.Index.search.rest(many, "needle", T.Search.gone.but(Map.new(&2, Bool), "c"), T.Search.left(T.Index.search.in(many, "needle", "c")), 0n))),
      "30,29,28,27,26,25,24,23,22,21,60,59,58,57,56,55,54,53,52,51,50,49,48,47,46,")
