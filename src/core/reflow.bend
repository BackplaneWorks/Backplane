import Base
import ./json.bend as J

# An educational reflow preview, shared by clients. This is deliberately a
# first-order temperature lag (24 s time constant, 1 s Euler steps), followed
# by a linear fraction between published solidus and liquidus temperatures.
# No board geometry, heat capacity, latent heat, flux, wetting or oven model
# is inferred. It must never be presented as a process qualification result.

type ProfilePoint is Data:
  ProfilePoint{second: Nat, celsius: F32}

def Reflow.default() -> String:
  "[[0,25],[90,150],[170,180],[220,245],[250,245],[330,25]]"

# Source: Indium Corporation's solder alloy directory, accessed 2026-10-27.
# https://www.indium.com/products/alloys/solder-alloys/
def Reflow.alloys() -> String:
  "[{\"id\":\"SAC305\",\"composition\":\"Sn 96.5 · Ag 3 · Cu 0.5 wt%\",\"solidus\":217,\"liquidus\":220},{\"id\":\"Sn63Pb37\",\"composition\":\"Sn 63 · Pb 37 wt%\",\"solidus\":183,\"liquidus\":183},{\"id\":\"Sn42Bi58\",\"composition\":\"Sn 42 · Bi 58 wt%\",\"solidus\":138,\"liquidus\":138}]"

def Reflow.number(x: J.Json) -> F32:
  match x:
    case J.Num{s}: Maybe.default(&2, F32, F32.read(s), F32.neg(1.0))
    case _: F32.neg(1.0)

def Reflow.point(t: Maybe<&2, Nat>, +v: F32) -> Maybe<&2, ProfilePoint>:
  match t:
    case Some{+s}:
      Bool.pick(Maybe<&2, ProfilePoint>, Bool.and(Nat.is_le(s, 1200n), Bool.and(F32.is_ge(v, 0.0), F32.is_le(v, 350.0))), Some{ProfilePoint{s, v}}, None{})
    case None{}: None{}

def Reflow.readpoint(x: J.Json) -> Maybe<&2, ProfilePoint>:
  match x:
    case J.Arr{J.Item{J.Num{t}, J.Item{v, J.End{}}}}:
      Reflow.point(Nat.read(t), Reflow.number(v))
    case _: None{}

def Reflow.order(empty: Bool, +s: Nat, +prev: Nat) -> Bool:
  match empty:
    case True{}: Nat.is_eq(s, 0n)
    case False{}: Nat.is_gt(s, prev)

def Reflow.prepend(p: Maybe<&2, ProfilePoint>, rest: List<&2, ProfilePoint>) -> List<&2, ProfilePoint>:
  match p:
    case Some{v}: v <> rest
    case None{}: ProfilePoint{1201n, F32.neg(1.0)} <> rest

def Reflow.points(xs: J.Json, fuel: Nat) -> List<&2, ProfilePoint>:
  match xs fuel:
    case J.End{} _: Nil{}
    case J.Item{h, t} 1n+left:
      Reflow.prepend(Reflow.readpoint(h), Reflow.points(t, left))
    case _ _: [ProfilePoint{1201n, F32.neg(1.0)}]

def Reflow.read(x: J.Json) -> List<&2, ProfilePoint>:
  match x:
    case J.Arr{xs}: Reflow.points(xs, 32n)
    case _: Nil{}

def Reflow.valid(xs: List<&2, ProfilePoint>, +prev: Nat, empty: Bool) -> Bool:
  match xs:
    case Nil{}: True{}
    case ProfilePoint{+s, +v} <> rest:
      Bool.and(Reflow.order(empty, s, prev), Bool.and(Nat.is_le(s, 1200n), Bool.and(F32.is_ge(v, 0.0), Bool.and(F32.is_le(v, 350.0), Reflow.valid(rest, s, False{})))))

def Reflow.temp.go(xs: List<&2, ProfilePoint>, +t: Nat, +lastSecond: Nat, +last: F32) -> F32:
  match xs:
    case ProfilePoint{+s, +v} <> rest:
      Bool.pick(F32, Nat.is_le(t, s),
        F32.add(last, F32.mul(F32.sub(v, last), F32.div(U32.to_f32(U32.from_nat(Nat.sub(t, lastSecond))), U32.to_f32(U32.from_nat(Nat.max(1n, Nat.sub(s, lastSecond))))))),
        Reflow.temp.go(rest, t, s, v))
    case Nil{}: last

def Reflow.end(xs: List<&2, ProfilePoint>, last: Nat) -> Nat:
  match xs:
    case Nil{}: last
    case ProfilePoint{s, _} <> t: Reflow.end(t, s)

def Reflow.clamp(+v: F32) -> F32:
  F32.min(1.0, F32.max(0.0, v))

def Reflow.fraction.k(eutectic: Bool, +v: F32, +solidus: F32, liquidus: F32) -> F32:
  match eutectic:
    case True{}: Bool.pick(F32, F32.is_ge(v, solidus), 1.0, 0.0)
    case False{}: Reflow.clamp(F32.div(F32.sub(v, solidus), F32.sub(liquidus, solidus)))

def Reflow.fraction(v: F32, +solidus: F32, +liquidus: F32) -> F32:
  Reflow.fraction.k(F32.is_eq(solidus, liquidus), v, solidus, liquidus)

def Reflow.sample(+t: Nat, f: F32, b: F32, l: F32) -> J.Json:
  J.Json.arr([J.Json.nat(t), J.Num{F32.show(f)}, J.Num{F32.show(b)}, J.Num{F32.show(l)}])

def Reflow.steps(fuel: Nat, +t: Nat, +points: List<&2, ProfilePoint>, +board: F32, +solidus: F32, +liquidus: F32, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match fuel:
    case 0n: List.reverse(&2, J.Json, acc)
    case 1n+left:
      +furnace = Reflow.temp.go(points, t, 0n, 25.0)
      +next = F32.add(board, F32.div(F32.sub(furnace, board), 24.0))
      Reflow.steps(left, 1n+t, points, next, solidus, liquidus, Reflow.sample(t, furnace, next, Reflow.fraction(next, solidus, liquidus)) <> acc)

def Reflow.plan.points(points: List<&2, ProfilePoint>, +alloy: String) -> String:
  match points:
    case ProfilePoint{_, +start} <> +p <> +rest:
      +ps = {ProfilePoint{0n, start} <> p <> rest : List<&2, ProfilePoint>}
      +duration = Reflow.end(ps, 0n)
      +solidus = Bool.pick(F32, String.eq(alloy, "Sn63Pb37"), 183.0, Bool.pick(F32, String.eq(alloy, "Sn42Bi58"), 138.0, 217.0))
      +liquidus = Bool.pick(F32, String.eq(alloy, "SAC305"), 220.0, solidus)
      +samples = {Reflow.sample(0n, start, start, Reflow.fraction(start, solidus, liquidus)) <> Reflow.steps(duration, 1n, ps, start, solidus, liquidus, Nil{}) : List<&2, J.Json>}
      J.Json.show(J.Json.obj([J.KV{"duration", J.Json.nat(duration)}, J.KV{"samples", J.Json.arr(samples)}, J.KV{"solidus", J.Num{F32.show(solidus)}}, J.KV{"liquidus", J.Num{F32.show(liquidus)}}]))
    case _:
      "{\"error\":\"Use 2–32 points [whole seconds, °C], starting at 0, strictly increasing to at most 1200 s, with temperatures from 0 to 350 °C.\"}"

def Reflow.plan.valid(ok: Bool, points: List<&2, ProfilePoint>, alloy: String) -> String:
  match ok:
    case True{}: Reflow.plan.points(points, alloy)
    case False{}: Reflow.plan.points(Nil{}, alloy)

def Reflow.plan.read(+points: List<&2, ProfilePoint>, alloy: String) -> String:
  Reflow.plan.valid(Reflow.valid(points, 0n, True{}), points, alloy)

def Reflow.plan.alloy(ok: Bool, profile: String, alloy: String) -> String:
  match ok:
    case True{}: Reflow.plan.read(Reflow.read(Maybe.default(&2, J.Json, J.Json.parse(profile), J.Null{})), alloy)
    case False{}: "{\"error\":\"Unknown solder alloy.\"}"

def plan(profile: String, +alloy: String) -> String:
  Reflow.plan.alloy(Bool.or(String.eq(alloy, "SAC305"), Bool.or(String.eq(alloy, "Sn63Pb37"), String.eq(alloy, "Sn42Bi58"))), profile, alloy)

def defaults() -> String:
  Reflow.default()

def alloys() -> String:
  Reflow.alloys()
