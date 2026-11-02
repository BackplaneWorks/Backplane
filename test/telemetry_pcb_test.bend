import Base
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/pcb-usage.bend as PC
import ../src/core/usage-counts.bend as UC
import ../src/core/telemetry-usage.bend as U
import ../src/server/pcb-telemetry.bend as PCB
import ../src/core/storedb.bend as DB
import ../src/core/snapshot.bend as SN
import ./telemetry_usage_cases.bend as F
import ./check.bend as C

def check(name: String, yes: Bool) -> IO(Unit):
  C.Check.str(name, Bool.pick(String, yes, "yes", "no"), "yes")

def state() -> M.State:
  M.State.replay([M.SettingSet{PC.Pcb.key("step:private-design"), PC.Pcb.record(86400n, 3n, 0n), 86400n},
    M.SettingSet{PC.Pcb.key("order:private-quote"), PC.Pcb.record(86400n, 0n, 2n), 86400n}], F.UsageCases.state())

def count(s: M.State) -> String:
  M.Metrics.get(s, "1970-01-02")

def quote(pilot: Bool, kind: String) -> J.Json:
  J.Json.obj([J.KV{"pilot", J.Flag{pilot}}, J.KV{"quote", J.Json.obj([J.KV{"parts", J.Json.arr([
    J.Json.obj([J.KV{"kind", J.Str{kind}}, J.KV{"qty", J.Num{"999"}}, J.KV{"name", J.Str{"private-design"}}, J.KV{"summary", J.Str{"private-dimensions"}}])])}])}])

def numeric(v: J.Json) -> Bool:
  match v:
    case J.Num{+s}: Bool.and(Bool.not(String.is_empty(s)), Bool.and(UC.Uc.digits(s), UC.Uc.within(s)))
    case _: False{}

def fields(xs: J.Json, +allowed: List<&2, String>) -> Bool:
  match xs:
    case J.End{}: True{}
    case J.Obj{chain}: fields(chain, allowed)
    case J.Field{key, value, tail}: Bool.and(M.Tok.has(allowed, key), Bool.and(numeric(value), fields(tail, allowed)))
    case _: False{}

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("PCB project and board count once", Nat.show(PC.Pcb.created(["boards/private.kicad_pro", "boards/private.kicad_pcb"], Nil{})), "1")
    C.Check.str("later board in existing project not doubled", Nat.show(PC.Pcb.created(["boards/private.kicad_pcb"], ["boards/private.kicad_pro"])), "0")
    C.Check.str("later project beside existing board not doubled", Nat.show(PC.Pcb.created(["boards/private.kicad_pro"], ["boards/private.kicad_pcb"])), "0")
    C.Check.str("two independent boards", Nat.show(PC.Pcb.created(["a.kicad_pcb", "b.kicad_pcb", "a.kicad_sch", "part.step"], Nil{})), "2")
    C.Check.str("schematic or mechanical file is not board", Nat.show(PC.Pcb.created(["x.kicad_sch", "x.step"], Nil{})), "0")
    check("real board checkout counts", PC.Pcb.ordered(quote(False{}, "pcb"), "quoted", "paid"))
    check("missed paid transition still counts completed checkout", PC.Pcb.ordered(quote(False{}, "flex"), "quoted", "delivered"))
    check("pilot never counts", Bool.not(PC.Pcb.ordered(quote(True{}, "pcb"), "quoted", "paid")))
    check("quote alone never counts", Bool.not(PC.Pcb.ordered(quote(False{}, "pcb"), "quoted", "quoted")))
    check("cancelled never counts", Bool.not(PC.Pcb.ordered(quote(False{}, "pcb"), "quoted", "cancelled")))
    check("refunded never counts", Bool.not(PC.Pcb.ordered(quote(False{}, "pcb"), "quoted", "refunded")))
    check("mechanical only order is not board", Bool.not(PC.Pcb.ordered(quote(False{}, "3dp"), "quoted", "paid")))
    check("paid history and later shipping never recount", Bool.not(PC.Pcb.ordered(quote(False{}, "pcb"), "paid", "shipped")))
    C.Check.str("projected cumulative board counters", count(state()), "1/1/1/2/1/100/20/30/40/1/3/2")
    C.Check.str("duplicate source record cannot recount", count(M.State.apply(state(), M.SettingSet{PC.Pcb.key("step:private-design"), PC.Pcb.record(86400n, 3n, 0n), 86400n})), count(state()))
    C.Check.str("duplicate observation is not appended", Nat.show(List.length(&2, M.Change, PCB.Pcb.changes(state(), "step:private-design", 3n, 0n, M.Setting.get(state(), "telemetry.usage.generation", ""), 86400n))), "0")
    C.Check.str("off observation is not appended", Nat.show(List.length(&2, M.Change, PCB.Pcb.changes(M.State.apply(state(), M.SettingSet{"telemetry.share", "off", 86400n}), "new", 3n, 1n, "", 86400n))), "0")
    C.Check.str("unanswered observation is not appended", Nat.show(List.length(&2, M.Change, PCB.Pcb.changes(M.State.empty(), "new", 3n, 1n, "", 86400n))), "0")
    C.Check.str("stale worker generation is not appended", Nat.show(List.length(&2, M.Change, PCB.Pcb.changes(state(), "new", 3n, 1n, "stale", 86400n))), "0")
    C.Check.str("mirrored observation cannot count", Nat.show(List.length(&2, M.Change, PCB.Pcb.changes(state(), "peer~source", 3n, 1n, "", 86400n))), "0")
    C.Check.str("opt out erases all counters", count(M.State.apply(state(), M.SettingSet{"telemetry.share", "off", 86400n})), "")
    C.Check.str("reset erases all counters", count(M.State.apply(state(), M.SettingSet{"telemetry.reset", "1", 86400n})), "")
    C.Check.str("old stored ten counters read with zero board fields", UC.Uc.show(UC.Uc.read("1/2/3/4/5/6/7/8/9/10")), "1/2/3/4/5/6/7/8/9/10/0/0")
    C.Check.str("exactly twelve wire enum keys", Nat.show(List.length(&2, String, UC.Uc.all_names())), "12")
    check("payload has only integer counters and allowed enum keys", fields(UC.Uc.json(UC.Uc.read(count(state()))), UC.Uc.all_names()))
    check("payload has no design or local source data", Bool.not(String.contains(J.Json.show(U.Usage.event("1970-01-02", count(state()), F.UsageCases.id(), F.UsageCases.view())), "private")))
    check("boolean rejected by integer payload test", Bool.not(fields(J.Json.obj([J.KV{"boards_created", J.Flag{True{}}}]), UC.Uc.all_names())))
    check("fingerprint key rejected by payload test", Bool.not(fields(J.Json.obj([J.KV{"design_hash", J.Num{"1"}}]), UC.Uc.all_names())))
    C.Check.str("hot boot restores board counters and dedup marker", count(SN.Snap.boot(DB.Mat.replay(DB.Demo.lines(List.append(&2, M.Change, F.UsageCases.log(), [M.SettingSet{PC.Pcb.key("step:private-design"), PC.Pcb.record(86400n, 3n, 0n), 86400n}, M.SettingSet{PC.Pcb.key("order:private-quote"), PC.Pcb.record(86400n, 0n, 2n), 86400n}]), 0n), DB.Mat.empty()), Nil{})), count(state()))
