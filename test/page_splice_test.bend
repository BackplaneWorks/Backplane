import Base
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/thin.bend as T
import ../src/core/thin-live.bend as TL
import ../src/core/hist.bend as H
import ../src/server/thin-view.bend as V
import ../src/server/thin-splice.bend as S

# A view.page answered from the last view with only its timeline drawn
# again (thin-splice.bend's Page.splice) is the full render of that page:
# from the newest page to an older one and back, with and without a live
# row, and the same page again, on a thread long enough for three pages.
# Design steps are spread over it, so the top bar's count of changes is the
# thread's on every page. The driver (test/tools/page_splice.mjs) compares
# each case's two pictures: Bend's String.eq on them overflows its stack.

def z(+s: String) -> String:
  s ++ SCon{Chr{0}, SNil{}}

# a design step after every tenth message, each on the tool call before it
def step(+i: Nat) -> M.Change:
  M.ActivityLogged{"t1", "step:c" ++ Nat.show(i), "step", "kicad",
    H.Hist.record("c" ++ Nat.show(i) ++ "\nc0\n/r/.git\n/r\n" ++ z("M") ++ z("b.kicad_pcb"), "m" ++ Nat.show(i)), Nat.add(100n, i)}

def msg(+i: Nat) -> M.Change:
  M.MessagePosted{"t1", "m" ++ Nat.show(i), Bool.pick(M.Role, Nat.is_eq(Nat.mod(i, 2n), 0n), M.User{}, M.Assistant{}),
    "message number " ++ Nat.show(i) ++ " about the buck converter", Nat.add(100n, i)}

def msgs.k(stepped: Bool, +i: Nat, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match stepped:
    case True{}: step(i) <> msg(i) <> acc
    case False{}: msg(i) <> acc

def msgs(n: Nat, +i: Nat, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match n:
    case 0n: acc
    case 1n+k:
      msgs(k, Nat.add(i, 1n), msgs.k(Nat.is_eq(Nat.mod(i, 10n), 5n), i, acc))

def st() -> M.State:
  M.State.replay(List.append(&2, M.Change, [M.ProjectCreated{"p1", "P", "/p", 1n}, M.ThreadCreated{"t1", "p1", "A", M.Local{}, "", "", "claude", "", 2n}],
    List.reverse(&2, M.Change, msgs(80n, 1n, Nil{}))), M.State.empty())

def now() -> Nat:
  1790000100n

def p(+anchor: String) -> J.Json:
  J.Json.obj([J.KV{"target", J.Json.obj([J.KV{"kind", J.Str{"thread"}}, J.KV{"id", J.Str{"t1"}}])}, J.KV{"hub", J.Str{"127.0.0.1:3787"}},
    J.KV{"cid", J.Str{"c1"}}, J.KV{"session", J.Json.obj(Nil{})}, J.KV{"action", J.Str{""}},
    J.KV{"workspaceLayout", J.Str{"rail-v1"}}, J.KV{"workspaceChrome", J.Str{"top-v1"}}, J.KV{"bodyPages", J.Num{"1"}},
    J.KV{"anchor", J.Json.obj([J.KV{"id", J.Str{anchor}}, J.KV{"offset", J.Num{"0"}}])}])

def index() -> T.Index:
  T.Index.of(st())

# a page: the rows before row `before` (0: the newest page)
def rows(+before: Nat) -> List<&2, String>:
  T.Index.page(index(), "t1", before, 32n)

def mid(+before: Nat) -> Nat:
  V.Page.number(V.Page.mid(rows(before), V.Page.last(rows(before), "")))

def stream(+text: String, +version: Nat) -> TL.Stream:
  TL.Stream{1n, String.length(text), T.Blocks.of(text), False{}, version}

def info() -> J.Json:
  J.Json.obj(Nil{})

def live() -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"live"}}, J.KV{"items", J.Json.arr(Nil{})}])

def acted(+anchor: String) -> V.ActResult:
  V.Action.run(True{}, V.Session.current(True{}, st(), info(), live(), now(), p(anchor)), p(anchor), now())

def full(+before: Nat, +anchor: String, +s: TL.Stream) -> J.Json:
  V.Render.acted_live(acted(anchor), True{}, st(), index(), p(anchor), "1", now(), rows(before), s)

def shown(m: Maybe<&2, J.Json>) -> String:
  match m:
    case Some{j}: J.Json.show(j)
    case None{}: "(none)"

def spliced.k(a: V.ActResult, +from: Nat, +to: Nat, +anchor: String, +s: TL.Stream) -> String:
  match a:
    case V.ActResult{+u, _}:
      shown(S.Page.splice(full(from, anchor, s), V.Render.ui(u, st(), index(), J.Json.get_obj(p(anchor), "target"), rows(to), s, now()), "t1",
        V.Render.window(s, index(), "t1", p(anchor), "1", rows(to))))

def spliced(+from: Nat, +to: Nat, +anchor: String, +s: TL.Stream) -> String:
  spliced.k(acted(anchor), from, to, anchor, s)

# each case: the page spliced from the last view, then drawn in full
# newest page to the older one
def c0.spliced(+z: Nat) -> String:
  spliced(Nat.add(z, 0n), mid(0n), "m40", TL.Stream.blank())

def c0.full(+z: Nat) -> String:
  J.Json.show(full(Nat.add(z, mid(0n)), "m40", TL.Stream.blank()))

# the older page back to the newest
def c1.spliced(+z: Nat) -> String:
  spliced(Nat.add(z, mid(0n)), 0n, "m70", TL.Stream.blank())

def c1.full(+z: Nat) -> String:
  J.Json.show(full(Nat.add(z, 0n), "m70", TL.Stream.blank()))

# one older page to the next older
def c2.spliced(+z: Nat) -> String:
  spliced(Nat.add(z, mid(0n)), mid(mid(0n)), "m20", TL.Stream.blank())

def c2.full(+z: Nat) -> String:
  J.Json.show(full(Nat.add(z, mid(mid(0n))), "m20", TL.Stream.blank()))

# with a live row, newest to older
def c3.spliced(+z: Nat) -> String:
  spliced(Nat.add(z, 0n), mid(0n), "m40", stream("Looking at the buck converter now.", 3n))

def c3.full(+z: Nat) -> String:
  J.Json.show(full(Nat.add(z, mid(0n)), "m40", stream("Looking at the buck converter now.", 3n)))

# the same page again
def c4.spliced(+z: Nat) -> String:
  spliced(Nat.add(z, 0n), 0n, "", TL.Stream.blank())

def c4.full(+z: Nat) -> String:
  J.Json.show(full(Nat.add(z, 0n), "", TL.Stream.blank()))
