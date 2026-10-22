import Base
import ./check.bend as C
import ../src/core/thin-live.bend as TL
import ../src/core/thin.bend as T
import ../src/core/live.bend as L
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/server/thin-view.bend as V
import ../src/mobile/thin.bend as MT
import ../src/server/thin-actor.bend as A

def obj( +s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def check( +name: String, +yes: Bool) -> IO(Unit):
  C.Check.str(name, Bool.show(yes), "True")

def base() -> TL.Stream:
  TL.Stream.snapshot(L.Buf{"t", ["prefix é🙂"], 9n, False{}, 1n}, 10n)

def main() -> IO(Unit):
  do IO<Unit>:
    +a : T.Blocks = T.Blocks.of("prefix é🙂")
    +b : T.Blocks = TL.Blocks.append(a, "尾🙂")
    C.Check.str("append preserves scalar order", T.Blocks.join(T.Blocks.n(b), b, 0n, Nil{}), "prefix é🙂尾🙂")
    +long : T.Blocks = TL.Blocks.append(T.Blocks.of(String.repeat("a", 511n)), "🙂b")
    C.Check.str("scalar crosses UTF8 block intact", T.Blocks.get(long, 1n), "🙂b")
    C.Check.str("preceding block not rewritten", Nat.show(String.length(T.Blocks.get(long, 0n))), "511")
    +s : TL.Stream = TL.Stream.append(base(), "x", 11n)
    C.Check.str("delta advances stream version", Nat.show(TL.Stream.version(s)), "11")
    C.Check.str("short live text retains complete prefix", TL.Stream.preview(s), "prefix é🙂x")
    +big : TL.Stream = TL.Stream.snapshot(L.Buf{"t", [String.repeat("x", 9000n)], 9000n, False{}, 1n}, 12n)
    C.Check.str("large live text is shown whole", TL.Stream.preview(big), String.repeat("x", 9000n))
    +capped : TL.Stream = TL.Stream.snapshot(L.Buf{"t", ["tail"], 4n, True{}, 1n}, 13n)
    C.Check.str("only a buffer the hub cut is marked", TL.Stream.preview(capped), "…\ntail")
    C.Check.str("synthetic entry is only reserved selected ID", TL.Stream.id("t"), "@live:t")
    +p : J.Json = obj("{\"target\":{\"kind\":\"thread\",\"id\":\"t\"},\"entryId\":\"@live:t\",\"cursor\":\"\"}")
    +chunk : J.Json = J.Json.get_obj(V.LiveMessage.result(big, 0n, "t", p, "epoch"), "message")
    C.Check.str("live reader exact current version", J.Json.get_str(chunk, "version", ""), "12")
    check("first live chunk marks further data", J.Json.get_bool(chunk, "hasMore", False{}))
    check("first live chunk never claims complete", Bool.not(J.Json.get_bool(chunk, "complete", True{})))
    check("live initial cursor starts at zero", Maybe.is_some(&2, Nat, V.LiveMessage.start(big, p, "epoch")))
    +next : J.Json = V.Obj.put(p, "cursor", J.Str{J.Json.get_str(chunk, "cursor", "")})
    check("live old cursor rejected after delta", Bool.not(Maybe.is_some(&2, Nat, V.LiveMessage.start(TL.Stream.append(big, "z", 13n), next, "epoch"))))
    check("live foreign epoch cursor rejected", Bool.not(Maybe.is_some(&2, Nat, V.LiveMessage.start(big, next, "other"))))
    +choice : J.Json = MT.Choice(obj("{\"sel\":\"old\",\"route\":{\"kind\":\"bot-tab\",\"bot\":\"b\",\"tab\":\"mail\"}}"), "select", "hub|new")
    C.Check.str("native explicit selection keeps raw thread", J.Json.get_str(choice, "sel", ""), "new")
    check("native select omits stale route", Bool.not(Maybe.is_some(&2, J.Json, J.Json.get(choice, "route"))))
    C.Check.str("native target derives new thread", J.Json.get_str(MT.Target.of(choice), "kind", ""), "thread")
    +safe : J.Json = A.Actor.safe_input(M.State.empty(), obj("{\"target\":{\"kind\":\"thread\",\"id\":\"t\"},\"session\":{\"drafts\":{\"@lang.tag\":\"ja\",\"@f.password\":\"secret\"}}}"))
    C.Check.str("explicit device language survives safe projection input", J.Json.get_str(J.Json.get_obj(J.Json.get_obj(safe, "session"), "drafts"), "@lang.tag", ""), "ja")
    C.Check.str("display-language allowlist cannot import form secrets", J.Json.get_str(J.Json.get_obj(J.Json.get_obj(safe, "session"), "drafts"), "@f.password", "absent"), "absent")
    +cmd : J.Json = J.Json.obj([J.KV{"type", J.Str{"send"}}, J.KV{"text", J.Str{"{\"id\":1,\"m\":\"mail.read\",\"p\":{}}"}}])
    +target : J.Json = obj("{\"kind\":\"bot\",\"id\":\"b\"}")
    check("actual queued read exact socket view accepted", A.Job.valid(1, 1, 2, 2, "v", "v", target, target, cmd))
    check("same RPC id foreign socket refused", Bool.not(A.Job.valid(1, 3, 2, 2, "v", "v", target, target, cmd)))
    check("old connection generation read refused", Bool.not(A.Job.valid(1, 1, 2, 1, "v", "v", target, target, cmd)))
    check("old view read refused", Bool.not(A.Job.valid(1, 1, 2, 2, "new", "old", target, target, cmd)))
    +floor : Nat = Nat.add(Nat.mul(65536n, 2147483648n), 1n)
    check("first upper-half control accepted", A.Control.fresh(floor, 0n))
    check("duplicate control cannot reissue action", Bool.not(A.Control.fresh(floor, floor)))
    check("ordinary RPC cannot enter control lane", Bool.not(A.Control.fresh(1n, 0n)))
