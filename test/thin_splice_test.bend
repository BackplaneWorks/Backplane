import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/thin.bend as T
import ../src/core/thin-live.bend as TL
import ../src/core/i18n.bend as I
import ../src/server/thin-view.bend as V
import ../src/server/thin-splice.bend as S
import ../src/core/cbor.bend as CB

# A thin reply spliced with a new live row (thin-splice.bend) is the full
# render of the same inputs at the same clock: web and mobile, plain and
# Markdown text, the live row's chunk marker on, and no live row (None).

def st() -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "P", "/p", 1n}, M.ThreadCreated{"t1", "p1", "A", M.Local{}, "", "", "claude", "", 2n},
    M.MessagePosted{"t1", "m1", M.User{}, "Route the USB pair", 3n}, M.MessagePosted{"t1", "m2", M.Assistant{}, "On it.", 4n},
    M.ActivityLogged{"t1", "a1", "tool", "Bash", "ls -la", 5n}, M.MessagePosted{"t1", "m3", M.User{}, "and the buck?", 6n},
    M.TurnChanged{"t1", M.Running{}, 7n}], M.State.empty())

def now() -> Nat:
  1790000100n

def p(+web: Bool) -> J.Json:
  J.Json.obj([J.KV{"target", J.Json.obj([J.KV{"kind", J.Str{"thread"}}, J.KV{"id", J.Str{"t1"}}])}, J.KV{"hub", J.Str{"127.0.0.1:3787"}},
    J.KV{"cid", J.Str{"c1"}}, J.KV{"session", J.Json.obj(Nil{})}, J.KV{"action", J.Str{""}}])

def rows() -> List<&2, String>:
  T.Index.page(T.Index.of(st()), "t1", 0n, 32n)

def stream(+text: String, +cut: Bool, +version: Nat) -> TL.Stream:
  TL.Stream{1n, String.length(text), T.Blocks.of(text), cut, version}

def info() -> J.Json:
  J.Json.obj(Nil{})

def live() -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"live"}}, J.KV{"items", J.Json.arr(Nil{})}])

def full(+web: Bool, +s: TL.Stream) -> J.Json:
  V.Render.acted_live(V.Action.run(web, V.Session.current(web, st(), info(), live(), now(), p(web)), p(web), now()), web, st(), T.Index.of(st()), p(web), "1", now(), rows(), s)

def window(+web: Bool, +s: TL.Stream) -> J.Json:
  V.Render.window(s, T.Index.of(st()), "t1", p(web), "1", rows())

def shown(m: Maybe<&2, J.Json>) -> String:
  match m:
    case Some{j}:
      J.Json.show(j)
    case None{}:
      "(none)"

def spliced(+web: Bool, +a: TL.Stream, +b: TL.Stream) -> String:
  shown(S.Splice.reply(web, full(web, a), b, "t1", "1", J.Json.get_obj(p(web), "target"), I.En{}))

def bytes.show(bs: List<&2, U32>, acc: String) -> String:
  match bs:
    case Nil{}:
      acc
    case b <> t:
      bytes.show(t, acc ++ "," ++ U32.show(b))

def bytes.sp(sp: S.Spliced) -> String:
  match sp:
    case S.Spliced{+res, +holes, +json, +names}:
      Nat.show(Nat.sub(Nat.add(J.Json.bytes(res), json), names)) ++ "|" ++ bytes.show(CB.Cbor.encode.holes(res, holes), "")

def bytes.k(m: Maybe<&2, S.Cuts>, +web: Bool, +b: TL.Stream) -> String:
  match m:
    case Some{+cuts}:
      bytes.sp(S.Cuts.splice(cuts, web, b, "t1", "1", J.Json.get_obj(p(web), "target"), I.En{}))
    case None{}:
      "(no cuts)"

# the kept bytes spliced: the full render's CBOR and JSON size, exactly
def bytes(+web: Bool, +a: TL.Stream, +b: TL.Stream) -> String:
  bytes.k(S.Cuts.of(web, full(web, a), "t1"), web, b)

def bytes.full(+web: Bool, +b: TL.Stream) -> String:
  Nat.show(J.Json.bytes(full(web, b))) ++ "|" ++ bytes.show(CB.Cbor.encode(full(web, b)), "")

def same(+name: String, +web: Bool, +a: TL.Stream, +b: TL.Stream) -> IO(Unit):
  do IO<Unit>:
    C.Check.str(name, spliced(web, a, b), J.Json.show(full(web, b)))
    C.Check.str(name ++ ": bytes and JSON size", bytes(web, a, b), bytes.full(web, b))

def big(n: Nat, acc: String) -> String:
  match n:
    case 0n:
      acc
    case 1n+m:
      big(m, acc ++ "Line of a long streamed answer with some words, padding the block out to its size; ")

def main() -> IO(Unit):
  do IO<Unit>:
    same("web: a longer live row", True{}, stream("Looking at", False{}, 1n), stream("Looking at the buck converter now.", False{}, 2n))
    same("mobile: a longer live row", False{}, stream("Looking at", False{}, 1n), stream("Looking at the buck converter now.", False{}, 2n))
    same("mobile: Markdown in the live row", False{}, stream("Plan:", False{}, 3n), stream("Plan:\n\n- move C12\n- reroute **VBUS**\n\n```\nnet USB_D+\n```\n日本 😀", False{}, 4n))
    same("web: Markdown in the live row", True{}, stream("Plan:", False{}, 3n), stream("Plan:\n\n- move C12\n```\nx\n```", False{}, 4n))
    same("mobile: chunk marker on, new version", False{}, stream(big(140n, ""), True{}, 5n), stream(big(150n, "x"), True{}, 6n))
    same("web: chunk marker on, new version", True{}, stream(big(140n, ""), True{}, 5n), stream(big(150n, "x"), True{}, 6n))
    C.Check.str("no live row: none (a full render)", spliced(True{}, TL.Stream.blank(), stream("hi", False{}, 1n)), "(none)")
    C.Check.str("no live row, mobile: none", spliced(False{}, TL.Stream.blank(), stream("hi", False{}, 1n)), "(none)")
    C.Check.str("no live row: no cuts kept", bytes(True{}, TL.Stream.blank(), stream("hi", False{}, 1n)), "(no cuts)")
