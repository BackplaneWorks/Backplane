import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/thin-live.bend as TL
import ../src/web/dom.bend as D
import ../src/server/thin-view.bend as V
import ../src/server/thin-actor.bend as A

# The "N new ↓" pill on the hub-drawn page (thin-view's Render.pill): the
# page says which entry was newest when the reader left the end (host.js,
# the request's "away", kept by thin-actor's Actor.safe_input), and the hub
# counts what came after it on the whole thread, a reply streaming counting.

def st() -> M.State:
  M.State.replay([M.ThreadCreated{"t", "p", "T", M.Local{}, "", "", "claude", "", 1n},
    M.MessagePosted{"t", "m1", M.User{}, "one", 2n}, M.MessagePosted{"t", "m2", M.Assistant{}, "two", 3n},
    M.MessagePosted{"t", "m3", M.User{}, "three", 4n}, M.MessagePosted{"t", "m4", M.Assistant{}, "four", 5n}], M.State.empty())

def streaming() -> TL.Stream:
  TL.Stream.append(TL.Stream.blank(), "partial reply", 1n)

def pill.text(ns: List<&2, D.Node>) -> String:
  match ns:
    case Nil{}: ""
    case D.El{"button", "hold-pill", _, [D.Txt{+t}]} <> _: t
    case _ <> +tail: pill.text(tail)

def away.of(+p: J.Json) -> String:
  J.Json.get_str(J.Json.get_obj(J.Json.get_obj(A.Actor.safe_input(st(), p), "session"), "drafts"), "@away.t", "-")

def request(+away: J.Json) -> J.Json:
  J.Json.obj([J.KV{"target", V.Target.json("thread", "t")}, J.KV{"away", away}, J.KV{"session", J.Json.obj([J.KV{"sel", J.Str{"t"}}, J.KV{"drafts", J.Json.obj(Nil{})}])}])

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("at the end: no pill", V.Render.pill(st(), "t", "", TL.Stream.blank()), "")
    C.Check.str("at the end while a reply streams: no pill", V.Render.pill(st(), "t", "", streaming()), "")
    C.Check.str("away, nothing new", V.Render.pill(st(), "t", "m4", TL.Stream.blank()), "")
    C.Check.str("away, a reply streaming in", V.Render.pill(st(), "t", "m4", streaming()), "New ↓")
    C.Check.str("away, two messages since", V.Render.pill(st(), "t", "m2", TL.Stream.blank()), "2 new ↓")
    C.Check.str("away, counted on the whole thread, not a page", V.Render.pill(st(), "t", "m1", streaming()), "3 new ↓")
    C.Check.str("the pill comes last in the timeline", pill.text(List.reverse(&2, D.Node, V.Render.after(J.Json.obj([J.KV{"partial", J.Json.arr(Nil{})}]), U.Ui.scratch_set(U.Ui.init(0n), "@pill", "2 new ↓")))), "2 new ↓")
    C.Check.str("no pill, no button", Nat.show(List.length(&2, D.Node, V.Render.after(J.Json.obj([J.KV{"partial", J.Json.arr(Nil{})}]), U.Ui.init(0n)))), "0")
    C.Check.str("the request's away mark is the shown thread's", away.of(request(J.Str{"m2"})), "m2")
    C.Check.str("back at the end it is cleared", away.of(request(J.Str{""})), "")
    C.Check.str("a request without one leaves it", away.of(J.Json.obj([J.KV{"target", V.Target.json("thread", "t")}, J.KV{"session", J.Json.obj([J.KV{"sel", J.Str{"t"}}])}])), "-")
    C.Check.str("a mark longer than any entry id is not taken", away.of(request(J.Str{String.repeat("x", 200n)})), "-")
