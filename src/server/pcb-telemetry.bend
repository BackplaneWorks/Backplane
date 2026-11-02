import Base
import ../core/json.bend as J
import ../core/model.bend as M
import ../core/pcb-usage.bend as PC

# A job's privacy generation is captured before it starts. A discarded job
# cannot add counters to a new sharing session after an off/on or reset.
def Pcb.scope(step: Bool, +when: String, on: Bool, generation: String) -> String:
  Bool.pick(String, step, J.Json.show(J.Json.obj([J.KV{"when", J.Str{when}}, J.KV{"on", J.Flag{on}}, J.KV{"generation", J.Str{generation}}])), when)

def Pcb.epoch(generation: Maybe<&2, String>, s: M.State) -> String:
  Maybe.default(&2, String, generation, M.Setting.get(s, "telemetry.usage.generation", ""))

def Pcb.changes.k(go: Bool, source: String, created: Nat, ordered: Nat, +now: Nat) -> List<&2, M.Change>:
  match go:
    case False{}: Nil{}
    case True{}: [M.SettingSet{PC.Pcb.key(source), PC.Pcb.record(now, created, ordered), now}]

def Pcb.changes(+s: M.State, +source: String, +created: Nat, +ordered: Nat, generation: String, now: Nat) -> List<&2, M.Change>:
  Pcb.changes.k(Bool.and(M.Metrics.allowed(s), Bool.and(M.Metrics.local(source), Bool.and(Bool.not(String.is_empty(source)), Bool.and(String.is_empty(M.Setting.get(s, PC.Pcb.key(source), "")), Bool.and(Bool.or(Nat.is_gt(created, 0n), Nat.is_gt(ordered, 0n)), String.eq(generation, M.Setting.get(s, "telemetry.usage.generation", ""))))))), source, created, ordered, now)
