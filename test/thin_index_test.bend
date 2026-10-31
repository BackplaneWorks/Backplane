import Base
import ./check.bend as C
import ../src/core/thin.bend as T
import ../src/core/model.bend as M
import ../src/core/mail.bend as ML

def yes(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def ids(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}: ""
    case x <> t: x ++ "," ++ ids(t)

def posts(n: Nat, i: T.Index) -> T.Index:
  match n:
    case 0n: i
    case 1n+k: posts(k, T.Index.post(i, M.Msg{"same-id", "thread-a", M.Assistant{}, "Common needle 日本語", 7n}))

def search.ids(p: T.SearchPage) -> String:
  match p:
    case T.SearchPage{is0, _, _, _}: ids(is0)

def search.visits(p: T.SearchPage) -> String:
  match p:
    case T.SearchPage{_, n, _, _}: Nat.show(n)

def search.more(p: T.SearchPage) -> String:
  match p:
    case T.SearchPage{_, _, _, b}: yes(b)

def main() -> IO(Unit):
  do IO<Unit>:
    +empty : T.Index = T.Index.empty()
    C.Check.str("empty page is bounded empty", ids(T.Index.page(empty, "thread-a", 0n, 32n)), "")
    +a : T.Index = T.Index.post(empty, M.Msg{"m1", "thread-a", M.User{}, "Alpha common", 999n})
    +b : T.Index = T.Index.post(a, M.Act{"a1", "thread-b", "tool", "Bash", "beta common", 1n})
    +c : T.Index = T.Index.post(b, M.Msg{"m1", "thread-a", M.Assistant{}, "ALPHA gamma 日本語", 999n})
    C.Check.str("insertion order retained despite timestamp order", ids(T.Index.page(c, "thread-a", 0n, 32n)), "3,1,")
    C.Check.str("duplicate original IDs remain distinct rows", Nat.show(ML.Order.count(T.Index.thread(c, "thread-a"))), "2")
    C.Check.str("cross thread entries excluded", ids(T.Index.page(c, "thread-b", 0n, 32n)), "2,")
    C.Check.str("stable exclusive anchor skips exact prior entry", ids(T.Index.page(c, "thread-a", 3n, 32n)), "1,")
    +d : T.Index = T.Index.post(c, M.Msg{"later", "thread-a", M.Assistant{}, "later alpha", 0n})
    C.Check.str("insert before old page anchor cannot duplicate new rows", ids(T.Index.page(d, "thread-a", 3n, 32n)), "1,")
    C.Check.str("recent roots preserve latest insertion order", ids(T.Index.recent(d)), "thread-a,thread-b,")
    C.Check.str("indexed search normalizes ASCII case", search.ids(T.Index.search(d, "ALPHA", "", 0n)), "4,3,1,")
    C.Check.str("multi term search uses indexed conjunction", search.ids(T.Index.search(d, "alpha gamma", "", 0n)), "3,")
    C.Check.str("indexed Unicode term retained", search.ids(T.Index.search(d, "日本語", "", 0n)), "3,")
    C.Check.str("search target stays scoped", search.ids(T.Index.search(d, "common", "thread-a", 0n)), "1,")
    C.Check.str("unknown term avoids global scan", search.visits(T.Index.search(d, "absent", "", 0n)), "0")
    C.Check.str("empty query cannot enumerate all entries", search.ids(T.Index.search(d, "", "", 0n)), "")
    C.Check.str("missing conjunct gives zero candidate visits", search.visits(T.Index.search(d, "common absent", "", 0n)), "0")
    C.Check.str("search continuation excludes already inspected anchor", search.ids(T.Index.search(d, "alpha", "", 3n)), "1,")
    C.Check.str("raw canonical entry text remains exact", T.Entry.text(T.Row.entry(T.Index.row(d, 3n))), "ALPHA gamma 日本語")
    +many : T.Index = posts(160n, T.Index.empty())
    C.Check.str("requested oversized page capped at32", Nat.show(List.length(&2, String, T.Index.page(many, "thread-a", 0n, 999n))), "32")
    C.Check.str("search results cap at25", Nat.show(List.length(&2, String, T.Page.ids(T.Index.word(many, "common"), 0n, 25n))), "25")
    C.Check.str("matched search consumes only25 candidates", search.visits(T.Index.search(many, "common", "", 0n)), "25")
    C.Check.str("filtered search visits at most128 candidates", search.visits(T.Index.search(many, "common", "thread-other", 0n)), "128")
    C.Check.str("candidate bound explicitly retains continuation", search.more(T.Index.search(many, "common", "thread-other", 0n)), "yes")
    +state : M.State = M.State.apply(M.State.apply(M.State.empty(), M.MessagePosted{"thread-a", "one", M.User{}, "first", 1n}), M.ActivityLogged{"thread-b", "two", "tool", "Read", "second", 1n})
    +loaded : T.Index = T.Index.of(state)
    C.Check.str("boot index preserves real canonical State posting order", ids(T.Index.page(loaded, "thread-a", 0n, 32n)), "1,")
    C.Check.str("incremental commit matches boot ordinal", ids(T.Index.page(T.Index.change(T.Index.empty(), M.MessagePosted{"thread-a", "one", M.User{}, "first", 1n}), "thread-a", 0n, 32n)), "1,")
    C.Check.str("unrelated event adds no phantom entry", Nat.show(T.Index.n(T.Index.change(loaded, M.SettingSet{"test", "value", 1n}))), "2")
    +history : T.Index = T.Index.changes([M.ActivityLogged{"thread-a", "internal", "step", "snapshot", "history.git café", 1n}, M.MessagePosted{"thread-a", "visible", M.User{}, "history.git café", 2n}], T.Index.empty())
    C.Check.str("search excludes snapshot payload", search.ids(T.Index.search(history, "café", "", 0n)), "2,")
