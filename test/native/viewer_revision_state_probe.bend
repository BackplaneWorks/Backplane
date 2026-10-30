import Base
import ../../src/server/plot.bend as PV
import ../../src/server/proc.bend as PR
import ../../src/core/plot.bend as P
import ../../src/core/json.bend as J
import ../../src/core/board.bend as B

# Real plot lifecycle functions, without a hub, filesystem watcher or CAD job.
# Distinct boxes make a retained successful model observable on the wire.
def probe.key() -> String:
  "0|review|board"

def probe.model(a: Bool) -> P.Plot:
  P.Plot.of([B.Item{"segment", 0, B.Layer.id("F.Cu"), Bool.pick(B.Prim, a,
    B.PSeg{B.Pt{0.011, 0.022}, B.Pt{0.033, 0.044}, 0.01},
    B.PSeg{B.Pt{0.055, 0.066}, B.Pt{0.077, 0.088}, 0.01}), "segment"}], 0)

def probe.box(m: Maybe<&2, P.Plot>) -> J.Json:
  match m:
    case None{}: J.Null{}
    case Some{P.Plot{_, P.Box{x0, y0, x1, y1}, _, _, _}}:
      J.Json.arr([J.Json.u32(x0), J.Json.u32(y0), J.Json.u32(x1), J.Json.u32(y1)])

def probe.describe.src(ss: List<&2, PV.Src>, stage: String) -> IO(Unit):
  match ss:
    case Nil{}: IO.die(Unit, 2, "missing source")
    case PV.Src{_, _, _, sz, observed, _, busy, plot, good} <> _:
      IO.print(J.Json.show(J.Json.obj([J.KV{"stage", J.Str{stage}}, J.KV{"kind", J.Str{"state"}},
        J.KV{"size", J.Json.u32(sz)}, J.KV{"observed", J.Str{observed}}, J.KV{"good", J.Str{good}},
        J.KV{"busy", J.Flag{busy}}, J.KV{"box", probe.box(plot)}])))

def probe.describe(st: PV.PSt, stage: String) -> IO(Unit):
  match st:
    case PV.PSt{ss, _, _}: probe.describe.src(ss, stage)

def probe.hex.digit(+n: U32) -> Char:
  Chr{Bool.pick(U32, U32.is_lt(n, 10), U32.add(n, 48), U32.add(n, 87))}

def probe.hex.go(bs: List<&2, U32>, acc: String) -> String:
  match bs:
    case Nil{}: String.reverse(acc)
    case +b <> rest:
      probe.hex.go(rest, SCon{probe.hex.digit(U32.and(b, 15)), SCon{probe.hex.digit(U32.shrn(b, 4n)), acc}})

def probe.message(m: Maybe<&1, PV.POut>, stage: String, k: Unit -> IO(Unit)) -> IO(Unit):
  match m:
    case None{}: IO.pure(Unit, Unit{})
    case Some{PV.POut{client, text}}:
      do IO<Unit>:
        IO.print(J.Json.show(J.Json.obj([J.KV{"stage", J.Str{stage}}, J.KV{"kind", J.Str{"text"}}, J.KV{"client", J.Json.u32(client)}, J.KV{"text", J.Str{text}}])))
        k(Unit{})
    case Some{PV.PBin{client, bytes}}:
      do IO<Unit>:
        IO.print(J.Json.show(J.Json.obj([J.KV{"stage", J.Str{stage}}, J.KV{"kind", J.Str{"binary"}}, J.KV{"client", J.Json.u32(client)}, J.KV{"hex", J.Str{probe.hex.go(bytes, "")}}])))
        k(Unit{})

def probe.drain(fuel: Nat, +out: Chan(PV.POut), +stage: String) -> IO(Unit):
  match fuel:
    case 0n: IO.die(Unit, 2, "message count exceeded")
    case 1n+rest:
      IO.bind(Maybe<&1, PV.POut>, Unit, Chan.recv(PV.POut, out), m => probe.message(m, stage, u => probe.drain(rest, out, stage)))

def probe.handle(st: PV.PSt, message: PV.PMsg, +stage: String) -> IO(PV.PSt):
  do IO<PV.PSt>:
    +q : Chan(PV.PMsg) <- Chan.new(PV.PMsg, 8)
    +out : Chan(PV.POut) <- Chan.new(PV.POut, 16)
    +next : PV.PSt <- PV.Pv.handle(st, message, q, out, "/tmp", "")
    Chan.close(PV.PMsg, q)
    Chan.close(PV.POut, out)
    probe.drain(17n, out, stage)
    probe.describe(next, stage)
    return next

def probe.watch(st: PV.PSt, client: U32, +stage: String) -> IO(PV.PSt):
  do IO<PV.PSt>:
    +q : Chan(PV.PMsg) <- Chan.new(PV.PMsg, 8)
    +out : Chan(PV.POut) <- Chan.new(PV.POut, 16)
    +next : PV.PSt <- PV.Pv.watch.k(False{}, False{}, st, client, 0, probe.key(), "/unused", q, out, "/unused")
    Chan.close(PV.PMsg, q)
    Chan.close(PV.POut, out)
    probe.drain(17n, out, stage)
    probe.describe(next, stage)
    return next

def probe.checked.message(m: Maybe<&1, PV.PMsg>, stage: String) -> IO(Unit):
  match m:
    case Some{PV.PGot{_, sz, mt, plot, error}}:
      IO.print(J.Json.show(J.Json.obj([J.KV{"stage", J.Str{stage}}, J.KV{"kind", J.Str{"checked"}},
        J.KV{"size", J.Json.u32(sz)}, J.KV{"observed", J.Str{mt}}, J.KV{"error", J.Str{error}}, J.KV{"box", probe.box(plot)}])))
    case _: IO.die(Unit, 2, "read did not report PGot")

def probe.checked(+stage: String, before: U32 & String, after: U32 & String, r: PR.Ran) -> IO(Unit):
  do IO<Unit>:
    +q : Chan(PV.PMsg) <- Chan.new(PV.PMsg, 2)
    PV.Pv.read.checked(before, after, r, probe.key(), 0, q)
    Chan.close(PV.PMsg, q)
    m : Maybe<&1, PV.PMsg> <- Chan.recv(PV.PMsg, q)
    probe.checked.message(m, stage)

def main() -> IO(Unit):
  do IO<Unit>:
    a : PV.PSt <- probe.handle(PV.PSt{[PV.Src{probe.key(), 0, "/unused", 0, "", 0n, False{}, None{}, ""}], Nil{}, True{}},
      PV.PGot{probe.key(), 10, "revision-A", Some{probe.model(True{})}, ""}, "success-A")
    watched : PV.PSt <- probe.watch(a, 7, "first-subscriber")
    failed : PV.PSt <- probe.handle(watched, PV.PGot{probe.key(), 10, "revision-B", None{}, "export failed"}, "failure-B")
    cached : PV.PSt <- probe.watch(failed, 8, "cached-after-failure")
    half : PV.PSt <- probe.handle(cached, PV.PGot{probe.key(), 10, "revision-C", None{}, ""}, "half-written-C")
    cached2 : PV.PSt <- probe.watch(half, 9, "cached-after-half-written")
    good : PV.PSt <- probe.handle(cached2, PV.PGot{probe.key(), 11, "revision-D", Some{probe.model(False{})}, ""}, "success-D")
    last : PV.PSt <- probe.watch(good, 10, "cached-after-new-success")
    probe.checked("changed-revision", (10, "before"), (10, "after"), PR.Ran{0, P.Line.plot(probe.model(False{}))})
    probe.checked("changed-size", (10, "same"), (11, "same"), PR.Ran{0, P.Line.plot(probe.model(False{}))})
    probe.checked("stable-success", (10, "same"), (10, "same"), PR.Ran{0, P.Line.plot(probe.model(False{}))})
    probe.checked("stable-failure", (10, "same"), (10, "same"), PR.Ran{3, "export failed"})
    probe.checked("stable-half-written", (10, "same"), (10, "same"), PR.Ran{2, ""})
