import Base
import ./json.bend as J

# Paths are inspected locally only. Neither these paths, their stems nor an
# identifier derived from them is returned by the telemetry serializer.
def Pcb.family(+path: String) -> String:
  Bool.pick(String, Bool.or(String.ends_with(path, ".kicad_pcb"), String.ends_with(path, ".kicad_pro")), String.take(path, Nat.sub(String.length(path), 10n)), "")

def Pcb.has(xs: List<&2, String>, +family: String) -> Bool:
  match xs:
    case Nil{}: False{}
    case h <> tail: Bool.or(String.eq(Pcb.family(h), family), Pcb.has(tail, family))

# A project's .pro and .pcb together are one board, including when the .pcb
# is written later. Git supplies only added files after rename detection.
def Pcb.created(xs: List<&2, String>, +before: List<&2, String>) -> Nat:
  match xs:
    case Nil{}: 0n
    case +h <> +tail:
      Nat.add(Bool.pick(Nat, Bool.and(Bool.not(String.is_empty(Pcb.family(h))), Bool.not(Bool.or(Pcb.has(before, Pcb.family(h)), Pcb.has(tail, Pcb.family(h))))), 1n, 0n), Pcb.created(tail, before))

def Pcb.board_kind(+kind: String) -> Bool:
  Bool.or(String.eq(kind, "pcb"), Bool.or(String.eq(kind, "flex"), Bool.or(String.eq(kind, "hdi"), Bool.or(String.eq(kind, "aluminum"), Bool.or(String.eq(kind, "copper_core"), Bool.or(String.eq(kind, "rogers"), String.eq(kind, "ptfe")))))))

def Pcb.has_board(parts: List<&2, J.Json>) -> Bool:
  match parts:
    case Nil{}: False{}
    case h <> tail: Bool.or(Pcb.board_kind(J.Json.get_str(h, "kind", "")), Pcb.has_board(tail))

def Pcb.purchased(+state: String) -> Bool:
  Bool.or(String.eq(state, "paid"), Bool.or(String.eq(state, "placed"), Bool.or(String.eq(state, "shipped"), String.eq(state, "delivered"))))

# A completed checkout containing boards counts once, regardless of quantity
# or design count. Pilot/test payments and already-paid upgrade history do not.
def Pcb.ordered(+q: J.Json, +last: String, state: String) -> Bool:
  Bool.and(Bool.not(J.Json.get_bool(q, "pilot", False{})), Bool.and(Pcb.purchased(state), Bool.and(Bool.not(Pcb.purchased(last)), Pcb.has_board(J.Json.elems(J.Json.get_obj(J.Json.get_obj(q, "quote"), "parts"))))))

# This record is local, durable and numerical. Its setting key is a local
# deduplication marker, never an extra telemetry field.
def Pcb.record(at: Nat, created: Nat, ordered: Nat) -> String:
  J.Json.show(J.Json.obj([J.KV{"at", J.Json.nat(at)}, J.KV{"created", J.Json.nat(created)}, J.KV{"ordered", J.Json.nat(ordered)}]))

def Pcb.key(source: String) -> String:
  "telemetry.pcb." ++ source
