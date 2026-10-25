import Base
import ./json.bend as J
import ./model.bend as M
import ./client.bend as U
import ./ubersync.bend as US
import ./ubop.bend as UO
import ./uber.bend as UB
import ./bot.bend as BT
import ./plane.bend as PL
import ./cat.bend as C

# Presentation only: read the committed lease. A request, heartbeat, bot
# selection, or peer discovery is never a home move. No persisted UI state.
def Hop.snapshot(+u: U.Ui) -> J.Json:
  +st = U.Ui.state(u)
  +uid = US.Ub.uid(st)
  J.Json.obj([J.KV{"uid", J.Str{uid}}, J.KV{"holder", J.Str{US.Ub.holder(st)}},
    J.KV{"term", J.Str{Nat.show(US.Ub.term(st))}}, J.KV{"stamp", J.Str{M.Setting.get(st, "uber.rs." ++ uid ++ ".lease", "")}},
    J.KV{"name", J.Str{PL.Plane.home.of(US.Ub.holder.me(st), u)}},
    J.KV{"botName", J.Str{BT.Bot.name(BT.World.bot(M.State.bots(st), UB.Uber.bot(st)))}},
    J.KV{"look", J.Json.nat(BT.Bot.look(BT.World.bot(M.State.bots(st), UB.Uber.bot(st))))}])

# The register is ordered by term first, then stamp (concurrent takeovers
# may move the holder within one term). Both holders must exist.
def Hop.newer(+term: Nat, +stamp: String, +next_term: Nat, +next_stamp: String) -> Bool:
  Bool.or(Nat.is_gt(next_term, term), Bool.and(Nat.is_eq(next_term, term), UO.Ub.stamp.gt(next_stamp, stamp)))

def Hop.changed(+uid: String, +holder: String, +term: Nat, +stamp: String, +next_uid: String, +next_holder: String, +next_term: Nat, +next_stamp: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(uid)), Bool.and(String.eq(uid, next_uid),
    Bool.and(Bool.not(String.is_empty(holder)), Bool.and(Bool.not(String.is_empty(next_holder)),
      Bool.and(Bool.not(String.eq(holder, next_holder)), Hop.newer(term, stamp, next_term, next_stamp))))))

# A still rig: the two-second transfer animates the one composited cat,
# never its individual paths. Reduced motion therefore has no hidden SMIL.
def Hop.still(parts: List<&2, C.Part>) -> List<&2, C.Part>:
  match parts:
    case Nil{}: Nil{}
    case C.Part{n, d, f, s, w, a, _} <> rest:
      C.Part{n, d, f, s, w, a, Nil{}} <> Hop.still(rest)

def Hop.rig(look: Nat) -> List<&2, C.Part>:
  Hop.still(C.Cat.rig(look, "idle"))

def Hop.cat(look: Nat) -> String:
  "data:image/svg+xml;utf8," ++ C.Cat.uri("<svg xmlns='http://www.w3.org/2000/svg' width='48' height='48' viewBox='0 0 64 64'>" ++ String.concat(C.Cat.svg.parts(Hop.rig(look))) ++ "</svg>")

def Hop.title.k(blank: Bool, name: String) -> String:
  match blank:
    case True{}: "Bub’s moving"
    case False{}: name ++ "’s moving"

def Hop.title(+name: String) -> String:
  Hop.title.k(String.is_empty(String.trim(name)), String.trim(name))

def Hop.result(changed: Bool, +before: J.Json, +after: J.Json) -> J.Json:
  match changed:
    case False{}: J.Json.obj([J.KV{"advance", J.Flag{True{}}}, J.KV{"changed", J.Flag{False{}}}])
    case True{}:
      J.Json.obj([J.KV{"advance", J.Flag{True{}}}, J.KV{"changed", J.Flag{True{}}},
        J.KV{"id", J.Str{J.Json.get_str(after, "uid", "") ++ ":" ++ J.Json.get_str(after, "term", "") ++ ":" ++ J.Json.get_str(after, "stamp", "")}},
        J.KV{"from", J.Str{J.Json.get_str(before, "name", "")}}, J.KV{"to", J.Str{J.Json.get_str(after, "name", "")}},
        J.KV{"title", J.Str{Hop.title(J.Json.get_str(after, "botName", ""))}},
        J.KV{"cat", J.Str{Hop.cat(J.Json.get_nat(after, "look", 0n))}},
        J.KV{"rig", J.Json.arr(C.Cat.parts.json(Hop.rig(J.Json.get_nat(after, "look", 0n))))},
        J.KV{"duration", J.Json.nat(2000n)}])

def Hop.transition.k(advance: Bool, +before: J.Json, +after: J.Json) -> J.Json:
  match advance:
    case False{}: J.Null{}
    case True{}:
      Hop.result(Hop.changed(J.Json.get_str(before, "uid", ""), J.Json.get_str(before, "holder", ""), M.Switch.num(J.Json.get_str(before, "term", "0"), 0n), J.Json.get_str(before, "stamp", ""),
        J.Json.get_str(after, "uid", ""), J.Json.get_str(after, "holder", ""), M.Switch.num(J.Json.get_str(after, "term", "0"), 0n), J.Json.get_str(after, "stamp", "")), before, after)

def Hop.transition(+before: J.Json, +after: J.Json) -> J.Json:
  Hop.transition.k(Bool.and(String.eq(J.Json.get_str(before, "uid", ""), J.Json.get_str(after, "uid", "")),
    Hop.newer(M.Switch.num(J.Json.get_str(before, "term", "0"), 0n), J.Json.get_str(before, "stamp", ""), M.Switch.num(J.Json.get_str(after, "term", "0"), 0n), J.Json.get_str(after, "stamp", ""))), before, after)

def Hop.text(before: String, after: String) -> J.Json:
  Hop.transition(Maybe.default(&2, J.Json, J.Json.parse(before), J.Null{}), Maybe.default(&2, J.Json, J.Json.parse(after), J.Null{}))

law hop_initial_quiet:
  for +h: String
  for +t: Nat
  for +s: String
  for +nh: String
  for +nt: Nat
  for +ns: String
  {Hop.changed("", h, t, s, "", nh, nt, ns) == False{} : Bool}

def hop_initial_quiet(h, t, s, nh, nt, ns):
  {==}
